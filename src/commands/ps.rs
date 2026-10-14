//! [Process Manager] `wasmrun ps` subcommands: the client side of the
//! supervisor in `src/ps/`.

use crate::cli::PsSubcommands;
use crate::error::{Result, WasmrunError};
use crate::ps::config::DEFAULT_CONFIG_FILE;
use crate::ps::control::{self, SupervisorInfo};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub fn handle_ps_command(cmd: &PsSubcommands) -> Result<()> {
    let home = control::ps_home();
    match cmd {
        PsSubcommands::Start {
            config,
            foreground,
            name,
        } => start(&home, config.as_deref(), *foreground, name.as_deref()),
        PsSubcommands::Stop { name } => stop(&home, name.as_deref()),
        PsSubcommands::Restart { name } => restart(&home, name),
        PsSubcommands::List { json } => list(&home, *json),
        PsSubcommands::Status { name, json } => status(&home, name, *json),
        PsSubcommands::Logs {
            name,
            follow,
            lines,
        } => logs(&home, name, *follow, *lines),
        PsSubcommands::Supervise { config } => crate::ps::run_supervisor(Path::new(config)),
    }
}

fn err(message: impl Into<String>) -> WasmrunError {
    WasmrunError::from(message.into())
}

fn require(home: &Path) -> Result<SupervisorInfo> {
    control::running(home)
        .ok_or_else(|| err("no supervisor is running; start one with `wasmrun ps start`"))
}

/// A request whose non-2xx answer becomes the error, with the server's reason.
fn call(
    info: &SupervisorInfo,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let (code, text) = control::request(info, method, path, body.as_ref()).map_err(err)?;
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if (200..300).contains(&code) {
        Ok(value)
    } else {
        Err(err(value
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or(&text)
            .to_string()))
    }
}

fn get_status(info: &SupervisorInfo, name: &str) -> Result<ProcessStatusView> {
    let value = call(info, "GET", &format!("/processes/{name}"), None)?;
    serde_json::from_value(value).map_err(|e| err(e.to_string()))
}

fn start(home: &Path, config: Option<&str>, foreground: bool, name: Option<&str>) -> Result<()> {
    if let Some(name) = name {
        let info = require(home)?;
        call(&info, "POST", &format!("/processes/{name}/start"), None)?;
        let s = wait_for(&info, name, |s| {
            s.status != "stopped" && s.status != "starting"
        })?;
        println!("{name}: {}", describe(&s));
        return Ok(());
    }

    let path = PathBuf::from(config.unwrap_or(DEFAULT_CONFIG_FILE));
    let path = path
        .canonicalize()
        .map_err(|e| err(format!("{}: {e}", path.display())))?;
    // Read it here too, so a mistake is reported by the command that made it
    // rather than only in the supervisor's log.
    crate::ps::config::load(&path)?;

    if let Some(info) = control::running(home) {
        let added = call(
            &info,
            "POST",
            "/load",
            Some(serde_json::json!({"config": path.display().to_string()})),
        )?;
        println!("Added to the running supervisor: {}", added["added"]);
        std::thread::sleep(Duration::from_millis(500));
        return list(home, false);
    }

    if foreground {
        return crate::ps::run_supervisor(&path);
    }

    spawn_daemon(home, &path)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while control::running(home).is_none() {
        if Instant::now() > deadline {
            return Err(err(format!(
                "the supervisor did not come up; see {}",
                home.join("supervisor.log").display()
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(500));
    list(home, false)
}

/// Start `wasmrun ps supervise` detached from this terminal, logging to
/// `supervisor.log`.
fn spawn_daemon(home: &Path, config: &Path) -> Result<()> {
    std::fs::create_dir_all(home).map_err(|e| err(e.to_string()))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("supervisor.log"))
        .map_err(|e| err(e.to_string()))?;
    let exe = std::env::current_exe().map_err(|e| err(e.to_string()))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["ps", "supervise", "--config"])
        .arg(config)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().map_err(|e| err(e.to_string()))?)
        .stderr(log);
    // Its own process group, so Ctrl+C in this terminal does not reach it.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map_err(|e| err(format!("cannot start the supervisor: {e}")))?;
    Ok(())
}

fn stop(home: &Path, name: Option<&str>) -> Result<()> {
    let info = require(home)?;
    match name {
        Some(name) => {
            call(&info, "POST", &format!("/processes/{name}/stop"), None)?;
            let s = wait_for(&info, name, |s| {
                s.status != "running" && s.status != "starting"
            })?;
            println!("{name}: {}", describe(&s));
        }
        None => {
            call(&info, "POST", "/shutdown", None)?;
            let deadline = Instant::now() + Duration::from_secs(30);
            while control::state_file(home).exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
            println!("Stopped the supervisor and every program it ran");
        }
    }
    Ok(())
}

fn restart(home: &Path, name: &str) -> Result<()> {
    let info = require(home)?;
    let before = get_status(&info, name)?.pid;
    call(&info, "POST", &format!("/processes/{name}/restart"), None)?;
    let s = wait_for(&info, name, |s| {
        s.status == "errored" || (s.status == "running" && s.pid != before)
    })?;
    println!("{name}: {}", describe(&s));
    Ok(())
}

/// Poll a program's status until `done` holds, or 30s pass.
fn wait_for(
    info: &SupervisorInfo,
    name: &str,
    done: impl Fn(&ProcessStatusView) -> bool,
) -> Result<ProcessStatusView> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = get_status(info, name)?;
        if done(&s) || Instant::now() > deadline {
            return Ok(s);
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn list(home: &Path, json: bool) -> Result<()> {
    let Some(info) = control::running(home) else {
        if json {
            println!("[]");
        } else {
            println!("No supervisor is running. Start one with `wasmrun ps start`.");
        }
        return Ok(());
    };
    let value = call(&info, "GET", "/processes", None)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        );
        return Ok(());
    }
    let procs: Vec<ProcessStatusView> =
        serde_json::from_value(value).map_err(|e| err(e.to_string()))?;
    println!(
        "{:<20} {:<9} {:>7} {:>9} {:>8} {:>9}  PROGRAM",
        "NAME", "STATUS", "PID", "UPTIME", "RESTARTS", "MEMORY"
    );
    for p in procs {
        println!(
            "{:<20} {:<9} {:>7} {:>9} {:>8} {:>9}  {}",
            p.name,
            p.status,
            p.pid.map(|x| x.to_string()).unwrap_or_else(|| "-".into()),
            p.uptime_secs
                .map(human_duration)
                .unwrap_or_else(|| "-".into()),
            p.restarts,
            p.memory_bytes
                .map(human_bytes)
                .unwrap_or_else(|| "-".into()),
            p.program
        );
    }
    Ok(())
}

fn status(home: &Path, name: &str, json: bool) -> Result<()> {
    let info = require(home)?;
    let value = call(&info, "GET", &format!("/processes/{name}"), None)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        );
        return Ok(());
    }
    let s: ProcessStatusView = serde_json::from_value(value).map_err(|e| err(e.to_string()))?;
    println!("{}", s.name);
    println!("  status    {}", describe(&s));
    println!("  program   {}", s.program);
    if let Some(listen) = &s.listen {
        println!("  listen    {listen}");
    }
    println!("  restart   {} ({} so far)", s.restart, s.restarts);
    if let Some(mem) = s.memory_bytes {
        println!("  memory    {}", human_bytes(mem));
    }
    if let (Some(error), false) = (&s.error, s.status == "errored") {
        println!("  error     {error}");
    }
    println!("  log       {}", s.log);
    if !s.history.is_empty() {
        println!("  exits (oldest first)");
        for e in &s.history {
            let how = match (e.code, e.signal) {
                (Some(c), _) => format!("code {c}"),
                (None, Some(sig)) => format!("signal {sig}"),
                _ => "ended".into(),
            };
            println!("    {}  {how} after {:.1}s", e.at, e.ran_secs);
        }
    }
    Ok(())
}

fn logs(home: &Path, name: &str, follow: bool, lines: usize) -> Result<()> {
    let path = home.join("logs").join(format!("{name}.log"));
    if !path.exists() {
        return Err(err(format!("no log for '{name}' at {}", path.display())));
    }
    for line in crate::ps::logs::tail(&path, lines) {
        println!("{line}");
    }
    if follow {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let stop = Arc::clone(&stop);
            let _ = ctrlc::set_handler(move || stop.store(true, Ordering::SeqCst));
        }
        crate::ps::logs::follow(&path, &mut std::io::stdout(), &|| {
            !stop.load(Ordering::SeqCst)
        });
    }
    Ok(())
}

/// The status as the client reads it back: strings rather than the server's
/// enums, so an older client still reads a newer server.
#[derive(Debug, serde::Deserialize)]
struct ProcessStatusView {
    name: String,
    status: String,
    pid: Option<u32>,
    uptime_secs: Option<u64>,
    restarts: u32,
    memory_bytes: Option<u64>,
    program: String,
    listen: Option<String>,
    restart: String,
    #[serde(default)]
    history: Vec<ExitView>,
    error: Option<String>,
    log: String,
}

#[derive(Debug, serde::Deserialize)]
struct ExitView {
    at: String,
    code: Option<i32>,
    signal: Option<i32>,
    ran_secs: f64,
}

fn describe(s: &ProcessStatusView) -> String {
    match (s.status.as_str(), s.pid, s.uptime_secs) {
        ("running", Some(pid), Some(up)) => {
            format!("running (pid {pid}, up {})", human_duration(up))
        }
        ("errored", _, _) => format!("errored: {}", s.error.as_deref().unwrap_or("see the log")),
        (other, _, _) => other.to_string(),
    }
}

fn human_duration(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m{}s", s / 60, s % 60),
        s if s < 86_400 => format!("{}h{}m", s / 3600, (s % 3600) / 60),
        s => format!("{}d{}h", s / 86_400, (s % 86_400) / 3600),
    }
}

fn human_bytes(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MB {
        format!("{:.1} MB", bytes as f64 / MB)
    } else {
        format!("{} KB", bytes / 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_sizes_read_naturally() {
        assert_eq!(human_duration(5), "5s");
        assert_eq!(human_duration(125), "2m5s");
        assert_eq!(human_duration(7300), "2h1m");
        assert_eq!(human_duration(90_000), "1d1h");
        assert_eq!(human_bytes(2048), "2 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}
