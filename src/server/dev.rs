//! [Server Mode] A development session: the UI server, the app server beside it, and the rebuild loop
//!
//! A project whose build produces a page (it has its own `index.html`, or the build emitted JS
//! glue) is a web app. It runs natively in the browser from that page, served on the app port,
//! and the UI port becomes a control center around it. Anything else is a module, and the UI port
//! shows the console that loads it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tiny_http::Server;

use super::{app, handler};
use crate::compiler::builder::BuildResult;
use crate::error::{Result, ServerError, WasmrunError};
use crate::template::{TemplateManager, TemplateType};
use crate::watcher::{Change, ProjectWatcher};

const MAX_LOG_ENTRIES: usize = 1000;
/// The app gets a range of its own, so its URL does not move with the UI port
const APP_PORTS: std::ops::RangeInclusive<u16> = 8500..=8599;
/// How far above `--port` the UI may move when it is taken
const UI_PORT_SCAN: u16 = 10;

/// What a build produced
#[derive(Debug, Clone)]
pub struct Artifacts {
    pub wasm_path: PathBuf,
    pub js_path: Option<PathBuf>,
}

impl Artifacts {
    pub fn from_build(result: &BuildResult) -> Self {
        Self {
            wasm_path: PathBuf::from(&result.wasm_path),
            js_path: result.js_path.as_ref().map(PathBuf::from),
        }
    }

    pub fn from_wasm_file(wasm_path: &Path) -> Self {
        Self {
            wasm_path: wasm_path.to_path_buf(),
            js_path: sibling_bindgen_js(wasm_path),
        }
    }

    /// The directory the build wrote into, served under `/pkg/` on the app port
    pub fn dir(&self) -> PathBuf {
        self.wasm_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn wasm_filename(&self) -> String {
        file_name(&self.wasm_path)
    }

    pub fn js_filename(&self) -> Option<String> {
        self.js_path.as_deref().map(file_name)
    }
}

/// `foo_bg.wasm` next to `foo.js` is wasm-bindgen output, which only runs through its glue
fn sibling_bindgen_js(wasm_path: &Path) -> Option<PathBuf> {
    let name = wasm_path.file_name()?.to_string_lossy();
    let stem = name.strip_suffix("_bg.wasm")?;
    let js = wasm_path.with_file_name(format!("{stem}.js"));
    js.is_file().then_some(js)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Rebuilds the project, for watch mode
pub type Rebuild = Box<dyn FnMut() -> std::result::Result<Artifacts, String> + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    Building,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildInfo {
    /// Bumped by every successful build and every static-file change. A page reloads when it moves
    pub generation: u64,
    pub status: BuildStatus,
    pub builds: u64,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
    pub finished_at: Option<u64>,
    pub wasm_file: String,
    pub wasm_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    Build,
    Http,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub seq: u64,
    pub time: u64,
    pub source: LogSource,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Metrics {
    pub requests: u64,
    pub bytes_sent: u64,
    pub not_found: u64,
    pub errors: u64,
}

struct Inner {
    artifacts: Artifacts,
    build: BuildInfo,
    logs: VecDeque<LogEntry>,
    next_seq: u64,
    metrics: Metrics,
}

/// Everything the two servers and the rebuild loop share
pub struct DevState {
    pub project_path: Option<PathBuf>,
    pub project_name: String,
    pub watch: bool,
    pub ui_url: String,
    pub app_url: Option<String>,
    inner: Mutex<Inner>,
}

impl DevState {
    pub fn new(
        project_path: Option<PathBuf>,
        artifacts: Artifacts,
        watch: bool,
        ui_url: String,
        app_url: Option<String>,
    ) -> Self {
        let project_name = project_path
            .as_deref()
            .and_then(|p| p.canonicalize().ok())
            .map(|p| file_name(&p))
            .unwrap_or_else(|| artifacts.wasm_filename());
        let build = BuildInfo {
            generation: 1,
            status: BuildStatus::Ready,
            builds: 1,
            error: None,
            duration_ms: None,
            finished_at: Some(now_ms()),
            wasm_file: artifacts.wasm_filename(),
            wasm_size: file_size(&artifacts.wasm_path),
        };
        Self {
            project_path,
            project_name,
            watch,
            ui_url,
            app_url,
            inner: Mutex::new(Inner {
                artifacts,
                build,
                logs: VecDeque::new(),
                next_seq: 1,
                metrics: Metrics::default(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn artifacts(&self) -> Artifacts {
        self.lock().artifacts.clone()
    }

    pub fn build_info(&self) -> BuildInfo {
        self.lock().build.clone()
    }

    pub fn log(&self, source: LogSource, level: LogLevel, message: impl Into<String>) {
        let mut inner = self.lock();
        let seq = inner.next_seq;
        inner.next_seq += 1;
        inner.logs.push_back(LogEntry {
            seq,
            time: now_ms(),
            source,
            level,
            message: message.into(),
        });
        while inner.logs.len() > MAX_LOG_ENTRIES {
            inner.logs.pop_front();
        }
    }

    /// Entries after `since`, and the value to pass next time
    pub fn logs_since(&self, since: u64) -> (Vec<LogEntry>, u64) {
        let inner = self.lock();
        let entries = inner
            .logs
            .iter()
            .filter(|e| e.seq > since)
            .cloned()
            .collect();
        (entries, inner.next_seq - 1)
    }

    pub fn begin_build(&self) {
        self.lock().build.status = BuildStatus::Building;
        self.log(LogSource::Build, LogLevel::Info, "Building...");
    }

    /// A failed build keeps the last good artifacts, so the page keeps working
    pub fn finish_build(&self, result: std::result::Result<Artifacts, String>, elapsed: Duration) {
        let duration_ms = elapsed.as_millis() as u64;
        let message = {
            let mut inner = self.lock();
            inner.build.builds += 1;
            inner.build.duration_ms = Some(duration_ms);
            inner.build.finished_at = Some(now_ms());
            match result {
                Ok(artifacts) => {
                    inner.build.status = BuildStatus::Ready;
                    inner.build.error = None;
                    inner.build.generation += 1;
                    inner.build.wasm_file = artifacts.wasm_filename();
                    inner.build.wasm_size = file_size(&artifacts.wasm_path);
                    let msg = format!(
                        "Built {} ({}) in {duration_ms} ms",
                        inner.build.wasm_file,
                        format_bytes(inner.build.wasm_size)
                    );
                    inner.artifacts = artifacts;
                    Ok(msg)
                }
                Err(e) => {
                    inner.build.status = BuildStatus::Failed;
                    inner.build.error = Some(e.clone());
                    Err(e)
                }
            }
        };
        match message {
            Ok(msg) => self.log(LogSource::Build, LogLevel::Success, msg),
            Err(e) => self.log(
                LogSource::Build,
                LogLevel::Error,
                format!("Build failed: {e}"),
            ),
        }
    }

    /// A static file changed: nothing to build, but the page should reload
    pub fn reload(&self) {
        self.lock().build.generation += 1;
    }

    pub fn record_request(&self, status: u16, bytes: u64) {
        let mut inner = self.lock();
        inner.metrics.requests += 1;
        inner.metrics.bytes_sent += bytes;
        if status == 404 {
            inner.metrics.not_found += 1;
        } else if status >= 400 {
            inner.metrics.errors += 1;
        }
    }

    /// `GET /api/dev` on the UI port
    pub fn snapshot(&self) -> serde_json::Value {
        let inner = self.lock();
        serde_json::json!({
            "project": self.project_name,
            "project_path": self.project_path.as_ref().map(|p| p.to_string_lossy()),
            "watch": self.watch,
            "ui_url": self.ui_url,
            "app_url": self.app_url,
            "js_file": inner.artifacts.js_filename(),
            "build": inner.build,
            "metrics": inner.metrics,
        })
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn format_bytes(bytes: u64) -> String {
    match bytes {
        b if b >= 1024 * 1024 => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
        b if b >= 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

/// A project runs as a web app when it has a page of its own, or its build emitted JS glue
pub fn is_web_app(project_path: Option<&Path>, artifacts: &Artifacts) -> bool {
    artifacts.js_path.is_some() || project_path.is_some_and(|p| p.join("index.html").is_file())
}

pub struct DevConfig {
    pub project_path: Option<String>,
    pub ui_port: u16,
    pub app_port: Option<u16>,
    pub watch: bool,
    pub open_browser: bool,
}

/// Serve a built project until the process is stopped
pub fn run(config: DevConfig, initial: Artifacts, rebuild: Option<Rebuild>) -> Result<()> {
    let project_path = config.project_path.as_deref().map(PathBuf::from);
    let web = is_web_app(project_path.as_deref(), &initial);

    let (ui_server, ui_port) = bind_ui(config.ui_port)?;
    let ui_url = format!("http://127.0.0.1:{ui_port}");

    let app_server = if web {
        let (server, port) = bind_app(ui_port, config.app_port)?;
        Some((server, format!("http://127.0.0.1:{port}")))
    } else {
        None
    };

    let state = Arc::new(DevState::new(
        project_path.clone(),
        initial,
        config.watch && rebuild.is_some(),
        ui_url.clone(),
        app_server.as_ref().map(|(_, url)| url.clone()),
    ));

    if let Some((server, url)) = app_server {
        let state = Arc::clone(&state);
        state.log(
            LogSource::Server,
            LogLevel::Info,
            format!("App server listening on {url}"),
        );
        thread::spawn(move || {
            for request in server.incoming_requests() {
                app::handle_request(request, &state);
            }
        });
    }

    print_startup(&state);
    if config.open_browser {
        super::utils::open_browser_when_ready(ui_port);
    }

    let template_type = if web {
        TemplateType::App
    } else {
        TemplateType::Console
    };

    match (state.watch, rebuild, project_path) {
        (true, Some(rebuild), Some(project_path)) => {
            let ui_state = Arc::clone(&state);
            thread::spawn(move || serve_ui(ui_server, &ui_state, &template_type));
            watch_loop(&state, &project_path, rebuild)
        }
        _ => {
            serve_ui(ui_server, &state, &template_type);
            Ok(())
        }
    }
}

/// Loopback only: the app port serves the project directory, which is nobody else's business
fn bind(port: u16) -> Result<Server> {
    Server::http(("127.0.0.1", port)).map_err(|e| {
        WasmrunError::Server(ServerError::startup_failed(
            port,
            format!("Failed to bind 127.0.0.1:{port}: {e}"),
        ))
    })
}

/// The first port that binds. Binding is the check: probing first would race, and a probe on
/// another address can pass while `127.0.0.1` is taken
fn bind_first(ports: impl IntoIterator<Item = u16>) -> Option<(Server, u16)> {
    ports
        .into_iter()
        .find_map(|port| Server::http(("127.0.0.1", port)).ok().map(|s| (s, port)))
}

fn bind_ui(port: u16) -> Result<(Server, u16)> {
    if let Some(bound) = bind_first([port]) {
        return Ok(bound);
    }
    println!("⚠️  Port {port} is already in use");
    let above = (1..=UI_PORT_SCAN).filter_map(|offset| port.checked_add(offset));
    let (server, bound) = bind_first(above).ok_or_else(|| {
        WasmrunError::Server(ServerError::startup_failed(
            port,
            format!("Port {port} and the {UI_PORT_SCAN} above it are all in use; pass --port"),
        ))
    })?;
    println!("🔄 Using port {bound} for the UI");
    Ok((server, bound))
}

fn bind_app(ui_port: u16, requested: Option<u16>) -> Result<(Server, u16)> {
    if let Some(port) = requested {
        if port == ui_port {
            return Err(WasmrunError::from(format!(
                "--app-port {port} is the same as the UI port"
            )));
        }
        return Ok((bind(port)?, port));
    }
    bind_first(APP_PORTS).ok_or_else(|| {
        WasmrunError::Server(ServerError::startup_failed(
            *APP_PORTS.start(),
            format!(
                "No free port for the app in {}-{}; pass --app-port",
                APP_PORTS.start(),
                APP_PORTS.end()
            ),
        ))
    })
}

fn serve_ui(server: Server, state: &DevState, template_type: &TemplateType) {
    let templates = TemplateManager::new();
    for request in server.incoming_requests() {
        handler::handle_request(request, state, &templates, template_type);
    }
}

fn watch_loop(state: &DevState, project_path: &Path, mut rebuild: Rebuild) -> Result<()> {
    let watcher = ProjectWatcher::new(&project_path.to_string_lossy())
        .map_err(|e| WasmrunError::from(format!("Failed to create file watcher: {e}")))?;

    while let Some(events) = watcher.wait_for_change() {
        let events = match events {
            Ok(events) => events,
            Err(e) => {
                eprintln!("⚠️  File watcher error: {e}");
                continue;
            }
        };
        match watcher.classify(&events) {
            Change::Source(path) => {
                let name = relative_name(project_path, &path);
                println!("📂 {name} changed, rebuilding...");
                state.log(LogSource::Server, LogLevel::Info, format!("{name} changed"));
                state.begin_build();
                let started = Instant::now();
                let result = rebuild();
                match &result {
                    Ok(_) => println!("✅ Rebuilt in {} ms", started.elapsed().as_millis()),
                    Err(e) => eprintln!("❌ Build failed, still serving the last good build:\n{e}"),
                }
                state.finish_build(result, started.elapsed());
            }
            Change::Static(path) => {
                let name = relative_name(project_path, &path);
                println!("📂 {name} changed, reloading");
                state.log(
                    LogSource::Server,
                    LogLevel::Info,
                    format!("{name} changed, reloading"),
                );
                state.reload();
            }
            Change::None => {}
        }
    }
    Ok(())
}

fn relative_name(root: &Path, path: &Path) -> String {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    path.strip_prefix(&root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn print_startup(state: &DevState) {
    let build = state.build_info();
    let size = format_bytes(build.wasm_size);
    let summary = if state.project_name == build.wasm_file {
        format!("{} ({size})", build.wasm_file)
    } else {
        format!("{} ({}, {size})", state.project_name, build.wasm_file)
    };
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🅦 \x1b[1;36mWasmrun\x1b[0m  \x1b[0;37m{summary}\x1b[0m");
    println!();
    println!(
        "  🎛️  \x1b[1;34mUI:\x1b[0m  \x1b[4;36m{}\x1b[0m",
        state.ui_url
    );
    if let Some(app_url) = &state.app_url {
        println!("  🌐 \x1b[1;34mApp:\x1b[0m \x1b[4;36m{app_url}\x1b[0m");
    }
    if state.watch {
        println!("  👀 \x1b[1;34mWatching for changes\x1b[0m");
    }
    println!("\x1b[1;34m╰\x1b[0m\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn artifacts(dir: &Path, js: bool) -> Artifacts {
        let wasm = dir.join("demo_bg.wasm");
        fs::write(&wasm, b"\0asm\x01\0\0\0").unwrap();
        let js_path = js.then(|| {
            let js = dir.join("demo.js");
            fs::write(&js, "export default function __wbg_init() {}").unwrap();
            js
        });
        Artifacts {
            wasm_path: wasm,
            js_path,
        }
    }

    fn state(dir: &Path) -> DevState {
        DevState::new(None, artifacts(dir, true), true, "ui".into(), None)
    }

    #[test]
    fn test_web_app_detection() {
        let dir = tempdir().unwrap();
        let plain = artifacts(dir.path(), false);
        assert!(!is_web_app(Some(dir.path()), &plain));
        assert!(is_web_app(Some(dir.path()), &artifacts(dir.path(), true)));

        fs::write(dir.path().join("index.html"), "<html></html>").unwrap();
        assert!(is_web_app(Some(dir.path()), &plain));
        assert!(!is_web_app(None, &plain));
    }

    #[test]
    fn test_bindgen_sibling_js_is_found() {
        let dir = tempdir().unwrap();
        let a = artifacts(dir.path(), true);
        let found = Artifacts::from_wasm_file(&a.wasm_path);
        assert_eq!(found.js_filename().as_deref(), Some("demo.js"));

        let plain = dir.path().join("plain.wasm");
        fs::write(&plain, b"\0asm").unwrap();
        assert!(Artifacts::from_wasm_file(&plain).js_path.is_none());
    }

    #[test]
    fn test_failed_build_keeps_last_artifacts_and_generation() {
        let dir = tempdir().unwrap();
        let state = state(dir.path());
        let before = state.build_info();

        state.begin_build();
        assert_eq!(state.build_info().status, BuildStatus::Building);
        state.finish_build(Err("syntax error".into()), Duration::from_millis(5));

        let after = state.build_info();
        assert_eq!(after.status, BuildStatus::Failed);
        assert_eq!(after.error.as_deref(), Some("syntax error"));
        assert_eq!(after.generation, before.generation);
        assert_eq!(state.artifacts().wasm_filename(), "demo_bg.wasm");
    }

    #[test]
    fn test_successful_build_bumps_generation() {
        let dir = tempdir().unwrap();
        let state = state(dir.path());
        let next = dir.path().join("next.wasm");
        fs::write(&next, b"\0asm\x01\0\0\0extra").unwrap();

        state.finish_build(
            Ok(Artifacts {
                wasm_path: next,
                js_path: None,
            }),
            Duration::from_millis(7),
        );

        let info = state.build_info();
        assert_eq!(info.generation, 2);
        assert_eq!(info.status, BuildStatus::Ready);
        assert_eq!(info.wasm_file, "next.wasm");
        assert_eq!(info.wasm_size, 13);
        assert_eq!(info.duration_ms, Some(7));

        state.reload();
        assert_eq!(state.build_info().generation, 3);
    }

    #[test]
    fn test_logs_since_and_cap() {
        let dir = tempdir().unwrap();
        let state = state(dir.path());
        for i in 0..(MAX_LOG_ENTRIES + 5) {
            state.log(LogSource::Http, LogLevel::Info, format!("req {i}"));
        }
        let (all, next) = state.logs_since(0);
        assert_eq!(all.len(), MAX_LOG_ENTRIES);
        assert_eq!(next, (MAX_LOG_ENTRIES + 5) as u64);

        state.log(LogSource::Build, LogLevel::Error, "boom");
        let (fresh, _) = state.logs_since(next);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].message, "boom");
    }

    #[test]
    fn test_metrics() {
        let dir = tempdir().unwrap();
        let state = state(dir.path());
        state.record_request(200, 100);
        state.record_request(404, 9);
        state.record_request(500, 0);
        let m = &state.snapshot()["metrics"];
        assert_eq!(m["requests"], 3);
        assert_eq!(m["bytes_sent"], 109);
        assert_eq!(m["not_found"], 1);
        assert_eq!(m["errors"], 1);
    }

    #[test]
    fn test_bind_first_skips_a_taken_port() {
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let taken = held.local_addr().unwrap().port();
        let free = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (_server, port) = bind_first([taken, free]).unwrap();
        assert_eq!(port, free);
        assert!(bind_first([taken]).is_none());
    }

    #[test]
    fn test_bind_app() {
        assert!(bind_app(9000, Some(9000)).is_err());
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let taken = held.local_addr().unwrap().port();
        assert!(bind_app(9000, Some(taken)).is_err());
    }
}
