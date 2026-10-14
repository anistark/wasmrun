//! [Process Manager] The supervisor: one monitor thread per program, which
//! starts it, watches it, records how it ended, and starts it again when the
//! restart policy says so, backing off on quick failures and giving up after
//! too many in a row.
//!
//! A program runs as its own `wasmrun exec -q` process rather than on a thread
//! here, so a program that crashes the interpreter (a host stack overflow is
//! not a catchable panic) takes down only itself, and so its memory can be
//! read and it can be stopped like any other process.

use crate::ps::config::{ProcessSpec, Program, RestartPolicy};
use crate::ps::logs::RotatingLog;
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

/// A run shorter than this counts as a quick failure for the backoff.
const STABLE_AFTER: Duration = Duration::from_secs(10);
/// How long a program gets to exit after SIGTERM before it is killed.
const STOP_GRACE: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(100);
/// Exit records kept per program.
const HISTORY: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Being started.
    Starting,
    Running,
    /// Ended, and waiting out the backoff before it is started again.
    Waiting,
    /// Stopped on request, or never started (`autostart = false`).
    Stopped,
    /// Ended on its own and not restarted, as its policy says.
    Exited,
    /// Restarted too often, or could not be started at all.
    Errored,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExitRecord {
    /// When it ended, RFC 3339.
    pub at: String,
    /// The exit code, absent when a signal ended it.
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub ran_secs: f64,
}

/// What `ps list` and `ps status` show for one program.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessStatus {
    pub name: String,
    pub status: Status,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub restarts: u32,
    pub memory_bytes: Option<u64>,
    pub program: String,
    pub listen: Option<String>,
    pub restart: RestartPolicy,
    pub last_exit: Option<ExitRecord>,
    pub history: Vec<ExitRecord>,
    pub error: Option<String>,
    pub log: PathBuf,
}

struct RunState {
    status: Status,
    /// Whether it should be running: start and stop requests set this, and
    /// the monitor thread moves the program towards it.
    wanted: bool,
    restart_now: bool,
    pid: Option<u32>,
    started: Option<(Instant, SystemTime)>,
    restarts: u32,
    recent_restarts: VecDeque<Instant>,
    quick_failures: u32,
    history: VecDeque<ExitRecord>,
    error: Option<String>,
}

struct Managed {
    spec: ProcessSpec,
    log: RotatingLog,
    log_path: PathBuf,
    state: Mutex<RunState>,
    wake: Condvar,
}

/// How the supervisor starts a program: the wasmrun binary to run `exec`
/// with, and where the nodejs runtime is for a script.
pub struct Launcher {
    pub exe: PathBuf,
    pub nodejs: Box<dyn Fn() -> Result<PathBuf, String> + Send + Sync>,
}

pub struct Supervisor {
    procs: Mutex<BTreeMap<String, Arc<Managed>>>,
    monitors: Mutex<Vec<JoinHandle<()>>>,
    logs_dir: PathBuf,
    launcher: Arc<Launcher>,
    shutting_down: Arc<AtomicBool>,
}

impl Supervisor {
    pub fn new(logs_dir: PathBuf, launcher: Launcher) -> Self {
        Self {
            procs: Mutex::new(BTreeMap::new()),
            monitors: Mutex::new(Vec::new()),
            logs_dir,
            launcher: Arc::new(launcher),
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Take on more programs. A name already supervised is refused, and so is
    /// the whole batch with it, so a file is loaded all or nothing.
    pub fn add(&self, specs: Vec<ProcessSpec>) -> Result<Vec<String>, String> {
        let mut procs = self.procs.lock().map_err(|_| "supervisor state poisoned")?;
        if let Some(dup) = specs.iter().find(|s| procs.contains_key(&s.name)) {
            return Err(format!(
                "a process named '{}' is already supervised",
                dup.name
            ));
        }
        let mut names = Vec::new();
        for spec in specs {
            let log_path = self.logs_dir.join(format!("{}.log", spec.name));
            let log = RotatingLog::open(
                log_path.clone(),
                spec.log_max_mb.saturating_mul(1024 * 1024),
                spec.log_files,
            )
            .map_err(|e| format!("cannot open {}: {e}", log_path.display()))?;
            let managed = Arc::new(Managed {
                state: Mutex::new(RunState {
                    status: Status::Stopped,
                    wanted: spec.autostart,
                    restart_now: false,
                    pid: None,
                    started: None,
                    restarts: 0,
                    recent_restarts: VecDeque::new(),
                    quick_failures: 0,
                    history: VecDeque::new(),
                    error: None,
                }),
                wake: Condvar::new(),
                log,
                log_path,
                spec,
            });
            names.push(managed.spec.name.clone());
            procs.insert(managed.spec.name.clone(), Arc::clone(&managed));
            let launcher = Arc::clone(&self.launcher);
            let shutting_down = Arc::clone(&self.shutting_down);
            let handle = std::thread::Builder::new()
                .name(format!("ps:{}", managed.spec.name))
                .spawn(move || monitor(managed, launcher, shutting_down))
                .map_err(|e| e.to_string())?;
            if let Ok(mut monitors) = self.monitors.lock() {
                monitors.push(handle);
            }
        }
        Ok(names)
    }

    fn get(&self, name: &str) -> Result<Arc<Managed>, String> {
        self.procs
            .lock()
            .ok()
            .and_then(|p| p.get(name).cloned())
            .ok_or_else(|| format!("no process named '{name}'"))
    }

    pub fn start(&self, name: &str) -> Result<(), String> {
        let m = self.get(name)?;
        let mut st = m.state.lock().map_err(|_| "poisoned")?;
        st.wanted = true;
        st.error = None;
        // A fresh start forgives earlier failures.
        st.recent_restarts.clear();
        st.quick_failures = 0;
        m.wake.notify_all();
        Ok(())
    }

    pub fn stop(&self, name: &str) -> Result<(), String> {
        let m = self.get(name)?;
        let mut st = m.state.lock().map_err(|_| "poisoned")?;
        st.wanted = false;
        m.wake.notify_all();
        Ok(())
    }

    pub fn restart(&self, name: &str) -> Result<(), String> {
        let m = self.get(name)?;
        let mut st = m.state.lock().map_err(|_| "poisoned")?;
        st.wanted = true;
        st.restart_now = true;
        st.error = None;
        m.wake.notify_all();
        Ok(())
    }

    pub fn status(&self, name: &str) -> Result<ProcessStatus, String> {
        Ok(snapshot(&*self.get(name)?))
    }

    pub fn list(&self) -> Vec<ProcessStatus> {
        let procs: Vec<Arc<Managed>> = self
            .procs
            .lock()
            .map(|p| p.values().cloned().collect())
            .unwrap_or_default();
        procs.iter().map(|m| snapshot(m)).collect()
    }

    /// Stop every program and wait for the monitor threads to finish.
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        if let Ok(procs) = self.procs.lock() {
            for m in procs.values() {
                if let Ok(mut st) = m.state.lock() {
                    st.wanted = false;
                }
                m.wake.notify_all();
            }
        }
        let handles: Vec<JoinHandle<()>> = self
            .monitors
            .lock()
            .map(|mut m| m.drain(..).collect())
            .unwrap_or_default();
        for handle in handles {
            let _ = handle.join();
        }
    }
}

fn snapshot(m: &Managed) -> ProcessStatus {
    let st = match m.state.lock() {
        Ok(st) => st,
        Err(poisoned) => poisoned.into_inner(),
    };
    let running = st.status == Status::Running;
    ProcessStatus {
        name: m.spec.name.clone(),
        status: st.status,
        pid: if running { st.pid } else { None },
        uptime_secs: if running {
            st.started.map(|(at, _)| at.elapsed().as_secs())
        } else {
            None
        },
        restarts: st.restarts,
        memory_bytes: if running {
            st.pid.and_then(crate::utils::process::rss_bytes)
        } else {
            None
        },
        program: match &m.spec.program {
            Program::Wasm(path) => path.display().to_string(),
            Program::Script(script) => format!("{script} (nodejs)"),
        },
        listen: m.spec.listen.clone(),
        restart: m.spec.restart,
        last_exit: st.history.back().cloned(),
        history: st.history.iter().cloned().collect(),
        error: st.error.clone(),
        log: m.log_path.clone(),
    }
}

/// The `wasmrun exec` command line for a program.
pub fn exec_command(spec: &ProcessSpec, exe: &Path, nodejs: Option<&Path>) -> Command {
    let mut cmd = Command::new(exe);
    cmd.arg("exec").arg("-q");
    cmd.arg("--max-memory-mb")
        .arg(spec.max_memory_mb.to_string());
    if spec.max_fuel > 0 {
        cmd.arg("--max-fuel").arg(spec.max_fuel.to_string());
    }
    if let Some(cwd) = &spec.cwd {
        cmd.arg("--dir").arg(cwd);
    }
    for dir in &spec.dirs {
        cmd.arg("--dir").arg(dir);
    }
    for (key, value) in &spec.env {
        cmd.arg("--env").arg(format!("{key}={value}"));
    }
    if let Some(listen) = &spec.listen {
        cmd.arg("--tcplisten").arg(listen);
    }
    for rule in &spec.allow_net {
        cmd.arg("--allow-net").arg(rule);
    }
    let mut args = Vec::new();
    match &spec.program {
        Program::Wasm(path) => {
            cmd.arg(path);
        }
        Program::Script(script) => {
            cmd.arg(nodejs.expect("a script needs the nodejs runtime"));
            args.push("run".to_string());
            args.push(format!("/{script}"));
        }
    }
    args.extend(spec.args.iter().cloned());
    if !args.is_empty() {
        cmd.arg("--").args(args);
    }
    cmd
}

/// One program's whole life under the supervisor.
fn monitor(m: Arc<Managed>, launcher: Arc<Launcher>, shutting_down: Arc<AtomicBool>) {
    loop {
        // Wait until it is wanted, or the supervisor is going away.
        {
            let mut st = match m.state.lock() {
                Ok(st) => st,
                Err(_) => return,
            };
            while !st.wanted && !shutting_down.load(Ordering::SeqCst) {
                st = match m.wake.wait(st) {
                    Ok(st) => st,
                    Err(_) => return,
                };
            }
            if shutting_down.load(Ordering::SeqCst) {
                return;
            }
            st.status = Status::Starting;
            st.restart_now = false;
        }

        let nodejs = match &m.spec.program {
            Program::Script(_) => match (launcher.nodejs)() {
                Ok(path) => Some(path),
                Err(e) => {
                    give_up(&m, format!("could not get the nodejs runtime: {e}"));
                    continue;
                }
            },
            Program::Wasm(_) => None,
        };
        let mut cmd = exec_command(&m.spec, &launcher.exe, nodejs.as_deref());
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                give_up(&m, format!("could not start: {e}"));
                continue;
            }
        };
        let pid = child.id();
        let started = Instant::now();
        if let Ok(mut st) = m.state.lock() {
            st.status = Status::Running;
            st.pid = Some(pid);
            st.started = Some((started, SystemTime::now()));
        }
        m.log.note(&format!("started (pid {pid})"));
        let pumps: Vec<JoinHandle<()>> = [
            child
                .stdout
                .take()
                .map(|o| Box::new(o) as Box<dyn std::io::Read + Send>),
            child
                .stderr
                .take()
                .map(|e| Box::new(e) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        .map(|stream| {
            let log = m.log.clone();
            std::thread::spawn(move || log.pump(stream))
        })
        .collect();

        let status = watch(&m, &mut child, &shutting_down);
        for pump in pumps {
            let _ = pump.join();
        }

        let ran = started.elapsed();
        let record = ExitRecord {
            at: chrono::Utc::now().to_rfc3339(),
            code: status.as_ref().and_then(|s| s.code()),
            signal: status.as_ref().and_then(signal_of),
            ran_secs: (ran.as_millis() as f64) / 1000.0,
        };
        let how = match (record.code, record.signal) {
            (Some(code), _) => format!("exited with code {code}"),
            (None, Some(signal)) => format!("ended by signal {signal}"),
            _ => "ended".to_string(),
        };
        m.log.note(&format!("{how} after {:.1}s", record.ran_secs));
        let failed = record.code != Some(0);

        let mut st = match m.state.lock() {
            Ok(st) => st,
            Err(_) => return,
        };
        st.pid = None;
        st.started = None;
        st.history.push_back(record);
        while st.history.len() > HISTORY {
            st.history.pop_front();
        }

        if shutting_down.load(Ordering::SeqCst) || !st.wanted {
            st.status = Status::Stopped;
            continue;
        }
        if st.restart_now {
            st.restarts += 1;
            continue; // an explicit restart: no backoff, no failure count
        }
        let restart = match m.spec.restart {
            RestartPolicy::Always => true,
            RestartPolicy::OnFailure => failed,
            RestartPolicy::Never => false,
        };
        if !restart {
            st.status = Status::Exited;
            st.wanted = false;
            continue;
        }

        // Too many restarts within the window: something is wrong that
        // restarting will not fix.
        let now = Instant::now();
        let window = Duration::from_secs(m.spec.restart_window_secs);
        while st
            .recent_restarts
            .front()
            .is_some_and(|t| now.duration_since(*t) > window)
        {
            st.recent_restarts.pop_front();
        }
        if st.recent_restarts.len() as u32 >= m.spec.max_restarts {
            let reason = format!(
                "restarted {} times within {}s; not restarting again (ps start {} to try once more)",
                m.spec.max_restarts, m.spec.restart_window_secs, m.spec.name
            );
            st.status = Status::Errored;
            st.wanted = false;
            st.error = Some(reason.clone());
            drop(st);
            m.log.note(&reason);
            continue;
        }

        st.quick_failures = if ran < STABLE_AFTER {
            st.quick_failures + 1
        } else {
            0
        };
        let delay = backoff(m.spec.backoff_ms, m.spec.max_backoff_ms, st.quick_failures);
        st.status = Status::Waiting;
        st.recent_restarts.push_back(now);
        st.restarts += 1;
        m.log
            .note(&format!("restarting in {:.1}s", delay.as_secs_f64()));
        // Sleep out the backoff, unless asked to stop or restart sooner.
        let deadline = Instant::now() + delay;
        while st.wanted && !st.restart_now && !shutting_down.load(Ordering::SeqCst) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            st = match m.wake.wait_timeout(st, left) {
                Ok((st, _)) => st,
                Err(_) => return,
            };
        }
        if !st.wanted {
            st.status = Status::Stopped;
        }
    }
}

/// Watch a running program until it ends, stopping it when that is asked for.
fn watch(
    m: &Managed,
    child: &mut Child,
    shutting_down: &AtomicBool,
) -> Option<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        let stop = m
            .state
            .lock()
            .map(|st| !st.wanted || st.restart_now)
            .unwrap_or(true)
            || shutting_down.load(Ordering::SeqCst);
        if stop {
            m.log.note("stopping");
            return Some(stop_child(child));
        }
        // Wait a poll interval, or less if woken by a request.
        if let Ok(st) = m.state.lock() {
            let _ = m.wake.wait_timeout(st, POLL);
        }
    }
}

/// SIGTERM, then SIGKILL if it outstays the grace period.
fn stop_child(child: &mut Child) -> std::process::ExitStatus {
    let _ = crate::utils::process::terminate(child.id());
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            return status;
        }
        std::thread::sleep(POLL);
    }
    let _ = child.kill();
    child.wait().unwrap_or_else(|_| default_status())
}

#[cfg(unix)]
fn default_status() -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(9)
}

#[cfg(windows)]
fn default_status() -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1)
}

#[cfg(unix)]
fn signal_of(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(windows)]
fn signal_of(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

/// The delay before restart number `quick_failures` in a row: the base
/// doubled each time, capped. A program that ran long enough starts again
/// after the base delay.
pub fn backoff(base_ms: u64, max_ms: u64, quick_failures: u32) -> Duration {
    let factor = 1u64 << quick_failures.saturating_sub(1).min(20);
    Duration::from_millis(base_ms.saturating_mul(factor).min(max_ms))
}

/// Mark a program that cannot be started at all.
fn give_up(m: &Managed, reason: String) {
    m.log.note(&reason);
    if let Ok(mut st) = m.state.lock() {
        st.status = Status::Errored;
        st.wanted = false;
        st.error = Some(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_from_the_base_and_stops_at_the_cap() {
        let ms = |n| backoff(500, 30_000, n).as_millis();
        assert_eq!(
            [ms(0), ms(1), ms(2), ms(3), ms(4)],
            [500, 500, 1000, 2000, 4000]
        );
        assert_eq!(ms(10), 30_000);
        assert_eq!(ms(200), 30_000);
    }

    #[test]
    fn the_exec_command_carries_every_setting() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("server.js"), "").unwrap();
        let text = r#"
[[process]]
name = "api"
script = "server.js"
args = ["--verbose"]
env = { PORT = "8080" }
listen = "127.0.0.1:8080"
allow_net = ["127.0.0.1/32"]
max_memory_mb = 64
max_fuel = 1000
"#;
        let spec = crate::ps::config::parse(text, dir.path())
            .unwrap()
            .remove(0);
        let cmd = exec_command(
            &spec,
            Path::new("/bin/wasmrun"),
            Some(Path::new("/rt/nodejs.wasm")),
        );
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let joined = args.join(" ");
        for want in [
            "exec -q --max-memory-mb 64 --max-fuel 1000",
            "--env PORT=8080",
            "--tcplisten 127.0.0.1:8080",
            "--allow-net 127.0.0.1/32",
            "/rt/nodejs.wasm -- run /server.js --verbose",
        ] {
            assert!(joined.contains(want), "{want} not in {joined}");
        }
        assert!(
            joined.contains(&format!("--dir {}", dir.path().display())),
            "{joined}"
        );
    }
}
