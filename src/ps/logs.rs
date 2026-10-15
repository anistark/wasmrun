//! [Process Manager] One log file per process: the program's stdout and
//! stderr, line by line, plus the supervisor's own notes on starts and exits.
//! Rotated by size, `<name>.log` then `<name>.log.1` and so on.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A log file that moves itself aside once it passes `max_bytes`, keeping
/// `keep` old files. Shared by the stdout and stderr readers of one process,
/// so a line from one is never split by a line from the other.
#[derive(Clone)]
pub struct RotatingLog {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    path: PathBuf,
    file: Option<File>,
    size: u64,
    max_bytes: u64,
    keep: u32,
}

impl RotatingLog {
    pub fn open(path: PathBuf, max_bytes: u64, keep: u32) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                path,
                file: Some(file),
                size,
                max_bytes: max_bytes.max(1024),
                keep,
            })),
        })
    }

    /// Append one line (a newline is added when it has none).
    pub fn line(&self, text: &str) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let mut bytes = text.as_bytes().to_vec();
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        if inner.size > 0 && inner.size + bytes.len() as u64 > inner.max_bytes {
            inner.rotate();
        }
        if let Some(file) = inner.file.as_mut() {
            if file.write_all(&bytes).is_ok() {
                inner.size += bytes.len() as u64;
            }
        }
    }

    /// A note from the supervisor, marked so it reads apart from the program.
    pub fn note(&self, text: &str) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        self.line(&format!("[wasmrun ps {now}] {text}"));
    }

    /// Copy `reader` into the log a line at a time, until it ends.
    pub fn pump(&self, reader: impl Read) {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => self.line(&String::from_utf8_lossy(&buf)),
            }
        }
    }
}

impl Inner {
    fn rotate(&mut self) {
        self.file = None;
        let numbered = |n: u32| PathBuf::from(format!("{}.{n}", self.path.display()));
        if self.keep == 0 {
            let _ = fs::remove_file(&self.path);
        } else {
            let _ = fs::remove_file(numbered(self.keep));
            for n in (1..self.keep).rev() {
                let _ = fs::rename(numbered(n), numbered(n + 1));
            }
            let _ = fs::rename(&self.path, numbered(1));
        }
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok();
        self.size = 0;
    }
}

/// The last `n` lines of a log.
pub fn tail(path: &Path, n: usize) -> Vec<String> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let lines: Vec<String> = BufReader::new(file).lines().map_while(|l| l.ok()).collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

/// Print what is appended to a log until `keep_going` says stop, following it
/// across a rotation (the file shrinking under us).
pub fn follow(path: &Path, out: &mut dyn Write, keep_going: &dyn Fn() -> bool) {
    let mut pos = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    while keep_going() {
        let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if len < pos {
            pos = 0; // rotated: read the new file from its start
        }
        if len > pos {
            if let Ok(mut file) = File::open(path) {
                if file.seek(SeekFrom::Start(pos)).is_ok() {
                    let mut chunk = Vec::new();
                    if file.take(len - pos).read_to_end(&mut chunk).is_ok() {
                        let _ = out.write_all(&chunk);
                        let _ = out.flush();
                        pos += chunk.len() as u64;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_log_rotates_and_keeps_only_so_many_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        let log = RotatingLog::open(path.clone(), 1024, 2).unwrap();
        for i in 0..100 {
            log.line(&format!("{i:03} {}", "x".repeat(60)));
        }
        assert!(fs::metadata(&path).unwrap().len() <= 1024);
        assert!(dir.path().join("app.log.1").exists());
        assert!(dir.path().join("app.log.2").exists());
        assert!(!dir.path().join("app.log.3").exists());
        // The newest line is in the live file, and nothing is split.
        let last = tail(&path, 1);
        assert!(last[0].starts_with("099 "), "{last:?}");
        assert!(tail(&path, 100).iter().all(|l| l.len() == 64));
    }

    #[test]
    fn pump_copies_lines_and_notes_are_marked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.log");
        let log = RotatingLog::open(path.clone(), 1 << 20, 1).unwrap();
        log.note("started (pid 1)");
        log.pump(&b"one\ntwo\nthree without newline"[..]);
        let lines = tail(&path, 10);
        assert!(lines[0].starts_with("[wasmrun ps ") && lines[0].ends_with("started (pid 1)"));
        assert_eq!(&lines[1..], ["one", "two", "three without newline"]);
    }
}
