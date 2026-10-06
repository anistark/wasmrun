//! [Server Mode] Which dev sessions are running, so `wasmrun stop` can find them.
//!
//! Several sessions can run at once, each on its own UI port, so there is one
//! file per session rather than one PID file: named by its PID, holding its UI
//! URL, in the system temp directory.

use crate::error::{Result, ServerError, WasmrunError};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn registry_dir() -> PathBuf {
    std::env::temp_dir().join("wasmrun").join("servers")
}

/// A running session's entry. Dropping it removes the entry.
pub struct Registration {
    path: PathBuf,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Record this process as a session serving `ui_url`. Best effort: a session
/// that cannot be recorded still serves, `wasmrun stop` just cannot see it.
/// Ctrl+C and the SIGTERM `stop` sends both remove the entry before exiting.
pub fn register(ui_url: &str) -> Option<Registration> {
    let dir = registry_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(std::process::id().to_string());
    std::fs::write(&path, ui_url).ok()?;

    let on_signal = path.clone();
    let _ = ctrlc::set_handler(move || {
        let _ = std::fs::remove_file(&on_signal);
        std::process::exit(0);
    });
    Some(Registration { path })
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunningServer {
    pub pid: u32,
    pub url: String,
}

/// Every session that is still running. An entry left by a session that was
/// killed outright is removed here, and so is one whose PID now belongs to
/// something else: the process has to be a wasmrun and its port has to answer.
pub fn running_servers() -> Vec<RunningServer> {
    running_in(&registry_dir())
}

fn running_in(dir: &Path) -> Vec<RunningServer> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut servers = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let pid = path
            .file_name()
            .and_then(|n| n.to_str()?.parse::<u32>().ok());
        let url = std::fs::read_to_string(&path).unwrap_or_default();
        match pid {
            Some(pid) if is_wasmrun(pid) && answers(&url) => servers.push(RunningServer {
                pid,
                url: url.trim().to_string(),
            }),
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    servers.sort_by_key(|s| s.pid);
    servers
}

pub fn is_server_running() -> bool {
    !running_servers().is_empty()
}

/// Stop every running session and return the ones that were stopped.
pub fn stop_running_servers() -> Result<Vec<RunningServer>> {
    let servers = running_servers();
    if servers.is_empty() {
        return Err(WasmrunError::Server(ServerError::NotRunning));
    }
    for server in &servers {
        terminate(server.pid)?;
    }

    // Each one removes its own entry on the way out; wait for that, then
    // remove whatever is left so a slow exit does not leave a stale entry
    let deadline = Instant::now() + Duration::from_secs(3);
    let dir = registry_dir();
    while Instant::now() < deadline && servers.iter().any(|s| dir.join(s.pid.to_string()).exists())
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    for server in &servers {
        let _ = std::fs::remove_file(dir.join(server.pid.to_string()));
    }
    Ok(servers)
}

fn answers(url: &str) -> bool {
    let addr = url.trim().trim_start_matches("http://");
    addr.parse::<SocketAddr>()
        .is_ok_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok())
}

#[cfg(unix)]
fn is_wasmrun(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("wasmrun"))
}

#[cfg(windows)]
fn is_wasmrun(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("wasmrun"))
}

#[cfg(unix)]
fn terminate(pid: u32) -> Result<()> {
    run_kill(pid, std::process::Command::new("kill").arg(pid.to_string()))
}

#[cfg(windows)]
fn terminate(pid: u32) -> Result<()> {
    run_kill(
        pid,
        std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]),
    )
}

fn run_kill(pid: u32, command: &mut std::process::Command) -> Result<()> {
    let output = command.output().map_err(|e| {
        WasmrunError::Server(ServerError::StopFailed {
            pid,
            reason: e.to_string(),
        })
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(WasmrunError::Server(ServerError::StopFailed {
            pid,
            reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_keeps_a_live_server() {
        let dir = tempfile::tempdir().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        // The test binary is not called wasmrun, so look it up the way the
        // check does and only expect a live entry when the name matches
        let pid = std::process::id();
        std::fs::write(dir.path().join(pid.to_string()), &url).unwrap();

        let running = running_in(dir.path());
        if is_wasmrun(pid) {
            assert_eq!(running, vec![RunningServer { pid, url }]);
        } else {
            assert!(running.is_empty());
        }
    }

    #[test]
    fn test_registry_drops_dead_and_foreign_entries() {
        let dir = tempfile::tempdir().unwrap();
        // A port nothing listens on, and a PID that is not a wasmrun
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        std::fs::write(dir.path().join("1"), format!("http://{closed}")).unwrap();
        std::fs::write(dir.path().join("not-a-pid"), "x").unwrap();

        assert!(running_in(dir.path()).is_empty());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn test_registration_removes_its_entry_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("123");
        std::fs::write(&path, "http://127.0.0.1:1").unwrap();
        drop(Registration { path: path.clone() });
        assert!(!path.exists());
    }
}
