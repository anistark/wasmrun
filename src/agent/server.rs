//! Agent mode: REST API server for AI agent sandbox management.

use crate::agent::api::*;
use crate::agent::executor;
use crate::agent::limits::{dir_size, ResourceLimits};
use crate::agent::session::{SessionConfig, SessionError, SessionManager, SessionState};
use crate::agent::shell;
use crate::agent::tools;
use crate::error::{Result, WasmrunError};
use crate::runtime::core::native_executor::{execute_wasm_bytes_with_env, ExecLimits};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const API_PREFIX: &str = "/api/v1";
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 30;
// Language runtimes (e.g. QuickJS compiled to WASM) generate deep call chains that
// overflow the default 8 MB thread stack when run through the WASM interpreter.
const EXEC_THREAD_STACK_BYTES: usize = 64 * 1024 * 1024;

pub struct AgentConfig {
    pub port: u16,
    pub session_config: SessionConfig,
    pub allow_cors: bool,
    pub verbose: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            port: 8430,
            session_config: SessionConfig::default(),
            allow_cors: false,
            verbose: false,
        }
    }
}

pub struct AgentServer {
    session_manager: Arc<SessionManager>,
    config: AgentConfig,
}

impl AgentServer {
    pub fn new(config: AgentConfig) -> Self {
        let session_manager = Arc::new(SessionManager::with_config(config.session_config.clone()));
        Self {
            session_manager,
            config,
        }
    }

    pub fn start(self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.port);
        let server = Server::http(&addr)
            .map_err(|e| WasmrunError::from(format!("Failed to start agent server: {e}")))?;

        self.print_banner();

        let cleanup_handle = SessionManager::start_cleanup_thread(self.session_manager.clone());

        // Graceful shutdown on Ctrl+C
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_flag = shutdown.clone();
        let _ = ctrlc::set_handler(move || {
            shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        for request in server.incoming_requests() {
            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                let _ =
                    request.respond(Response::from_string("").with_status_code(StatusCode(503)));
                break;
            }
            if let Err(e) = self.handle_request(request) {
                eprintln!("Request error: {e}");
            }
        }

        eprintln!("\n🛑 Shutting down...");
        let destroyed = self.session_manager.destroy_all().unwrap_or(0);
        self.session_manager.stop_cleanup();
        let _ = cleanup_handle.join();
        if destroyed > 0 {
            eprintln!("   Cleaned up {destroyed} session(s)");
        }
        eprintln!("   Goodbye.");
        Ok(())
    }

    fn print_banner(&self) {
        let port = self.config.port;
        let max = self.config.session_config.max_sessions;
        let timeout = self.config.session_config.default_timeout.as_secs();
        let limits = &self.config.session_config.limits;
        let cors = if self.config.allow_cors {
            "open"
        } else {
            "restricted"
        };
        println!("\n🤖 Wasmrun Agent Server");
        println!("   Endpoint:        http://0.0.0.0:{port}{API_PREFIX}");
        println!("   Max sessions:    {max}");
        println!("   Session timeout: {timeout}s");
        println!(
            "   Memory limit:    {}",
            fmt_pages_mb(limits.max_memory_pages)
        );
        println!(
            "   Fuel limit:      {}",
            fmt_opt_u64(limits.max_fuel, "instructions")
        );
        println!(
            "   Output limit:    {}",
            fmt_bytes_mb(limits.max_output_bytes.map(|b| b as u64))
        );
        println!("   File size limit: {}", fmt_bytes_mb(limits.max_file_size));
        println!(
            "   Disk limit:      {}",
            fmt_bytes_mb(limits.max_disk_bytes)
        );
        println!("   CORS:            {cors}");
        println!();
        println!("   Endpoints:");
        println!("     POST   /sessions              create session");
        println!("     GET    /sessions/:id           session status");
        println!("     DELETE /sessions/:id           destroy session");
        println!("     POST   /sessions/:id/exec      execute WASM");
        println!("     POST   /sessions/:id/files     write file");
        println!("     GET    /sessions/:id/files     read / list files");
        println!("     DELETE /sessions/:id/files     delete file");
        println!("     POST   /sessions/:id/env       set env vars");
        println!("     GET    /sessions/:id/env       get env vars");
        println!("     GET    /tools                  LLM tool schemas");
        println!();
    }

    fn cors_headers(&self) -> Vec<Header> {
        let origin = if self.config.allow_cors {
            "*"
        } else {
            "http://127.0.0.1"
        };
        vec![
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], origin.as_bytes()).unwrap(),
            Header::from_bytes(
                &b"Access-Control-Allow-Methods"[..],
                &b"GET, POST, DELETE, OPTIONS"[..],
            )
            .unwrap(),
            Header::from_bytes(
                &b"Access-Control-Allow-Headers"[..],
                &b"Content-Type, Authorization"[..],
            )
            .unwrap(),
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        ]
    }

    fn handle_request(&self, mut request: Request) -> Result<()> {
        let method = request.method().clone();
        let url = request.url().to_string();

        if self.config.verbose {
            eprintln!("{method} {url}");
        }

        if method == Method::Options {
            return self.respond_empty(request, 204);
        }

        let (path, query) = split_url(&url);
        let segments: Vec<&str> = path
            .trim_start_matches(API_PREFIX)
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let result = match (method, segments.as_slice()) {
            (Method::Get, ["tools"]) => {
                let params = parse_query(&query);
                let format = params.get("format").map(|s| s.as_str()).unwrap_or("openai");
                self.respond_json(request, self.handle_get_tools(format))
            }
            (Method::Post, ["sessions"]) => {
                let body = read_body(request.as_reader())?;
                self.respond_json(request, self.handle_create_session_with_body(&body))
            }
            (Method::Get, ["sessions", id]) => {
                self.respond_json(request, self.handle_get_session(id))
            }
            (Method::Delete, ["sessions", id]) => {
                self.respond_json(request, self.handle_delete_session(id))
            }
            (Method::Post, ["sessions", id, "exec"]) => {
                let body = read_body(request.as_reader())?;
                self.respond_json(request, self.handle_exec(id, &body))
            }
            (Method::Post, ["sessions", id, "files"]) => {
                let body = read_body(request.as_reader())?;
                self.respond_json(request, self.handle_write_file(id, &body))
            }
            (Method::Get, ["sessions", id, "files"]) => {
                let params = parse_query(&query);
                let path = params.get("path").map(|s| s.as_str()).unwrap_or("/");
                if params.get("list").map(|v| v == "true").unwrap_or(false) {
                    self.respond_json(request, self.handle_list_files(id, path))
                } else {
                    self.respond_json(request, self.handle_read_file(id, path))
                }
            }
            (Method::Delete, ["sessions", id, "files"]) => {
                let params = parse_query(&query);
                let path = params.get("path").map(|s| s.as_str()).unwrap_or("");
                self.respond_json(request, self.handle_delete_file(id, path))
            }
            (Method::Post, ["sessions", id, "env"]) => {
                let body = read_body(request.as_reader())?;
                self.respond_json(request, self.handle_set_env(id, &body))
            }
            (Method::Get, ["sessions", id, "env"]) => {
                self.respond_json(request, self.handle_get_env(id))
            }
            _ => {
                let err = ApiError::NotFound(format!("Unknown endpoint: {path}"));
                self.respond_json(request, Err::<serde_json::Value, _>(err))
            }
        };

        result
    }

    // ── Session endpoints ─────────────────────────────────────────

    #[allow(dead_code)] // Used by tests; the HTTP route uses the _with_body variant.
    pub fn handle_create_session(&self) -> std::result::Result<CreateSessionResponse, ApiError> {
        self.create_session_with_limits(self.config.session_config.limits.clone())
    }

    /// Create a session, applying any per-session limit overrides supplied in
    /// the (optional) request body on top of the server defaults.
    pub fn handle_create_session_with_body(
        &self,
        body: &str,
    ) -> std::result::Result<CreateSessionResponse, ApiError> {
        let limits = if body.trim().is_empty() {
            self.config.session_config.limits.clone()
        } else {
            let req: CreateSessionRequest =
                serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
            match req.limits {
                Some(ov) => self.config.session_config.limits.with_overrides(&ov),
                None => self.config.session_config.limits.clone(),
            }
        };
        self.create_session_with_limits(limits)
    }

    fn create_session_with_limits(
        &self,
        limits: ResourceLimits,
    ) -> std::result::Result<CreateSessionResponse, ApiError> {
        let id = self
            .session_manager
            .create_session_with_limits(self.config.session_config.default_timeout, limits)
            .map_err(map_session_err)?;
        Ok(CreateSessionResponse {
            session_id: id,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    pub fn handle_get_session(
        &self,
        id: &str,
    ) -> std::result::Result<SessionStatusResponse, ApiError> {
        self.session_manager
            .get_session(id, |s| SessionStatusResponse {
                session_id: s.id().to_string(),
                state: match s.state() {
                    SessionState::Active => "active".into(),
                    SessionState::Expired => "expired".into(),
                },
                created_at_elapsed_ms: s.created_at().elapsed().as_millis() as u64,
                last_accessed_elapsed_ms: s.last_accessed().elapsed().as_millis() as u64,
                timeout_secs: s.timeout().as_secs(),
            })
            .map_err(map_session_err)
    }

    pub fn handle_delete_session(
        &self,
        id: &str,
    ) -> std::result::Result<MessageResponse, ApiError> {
        self.session_manager
            .destroy_session(id)
            .map_err(map_session_err)?;
        Ok(MessageResponse {
            message: format!("Session {id} destroyed"),
        })
    }

    // ── Exec endpoint ─────────────────────────────────────────────

    pub fn handle_exec(&self, id: &str, body: &str) -> std::result::Result<ExecResponse, ApiError> {
        let req: ExecRequest =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let (wasi_env, work_dir, limits) = self
            .session_manager
            .get_session(id, |s| {
                (s.wasi_env(), s.work_dir().to_path_buf(), s.limits().clone())
            })
            .map_err(map_session_err)?;

        // Prepare environment
        {
            let mut env = wasi_env
                .lock()
                .map_err(|_| ApiError::Internal("Lock".into()))?;
            env.clear_stdout();
            env.clear_stderr();
            if let Some(ref vars) = req.env {
                for (k, v) in vars {
                    env.add_env(k.clone(), v.clone());
                }
            }
        }

        let timeout_secs = req.timeout.unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
        let timeout = Duration::from_secs(timeout_secs);
        let start = Instant::now();
        let exec_limits = ExecLimits {
            max_memory_pages: limits.max_memory_pages,
            max_fuel: limits.max_fuel,
        };
        let exec_env = wasi_env.clone();

        let (tx, rx) = std::sync::mpsc::channel::<std::result::Result<i32, ApiError>>();
        // Cooperative cancellation: the worker runs detached, so if the
        // wall-clock timeout fires we trip this flag to make the (possibly
        // unlimited-fuel) interpreter self-terminate instead of running on.
        let cancel = Arc::new(AtomicBool::new(false));

        if let Some(command) = req.command {
            // Built-in shell emulation: parse and run the command line
            // against the session's filesystem. No WASM module is loaded.
            let work_dir_clone = work_dir.clone();
            std::thread::Builder::new()
                .stack_size(EXEC_THREAD_STACK_BYTES)
                .spawn(move || {
                    let result = shell::run_command(&command, &work_dir_clone, exec_env)
                        .map_err(|e| ApiError::BadRequest(e.to_string()));
                    let _ = tx.send(result);
                })
                .map_err(|e| ApiError::Internal(format!("Failed to spawn exec thread: {e}")))?;
        } else if let Some(files) = req.files {
            // Multi-file source project: write all files and run entry through runtime
            let lang = req.language.unwrap_or_else(|| "javascript".into());
            executor::resolve_runtime(&lang)?;
            let entry = req
                .entry
                .clone()
                .ok_or_else(|| ApiError::BadRequest("'entry' is required with 'files'".into()))?;
            if !files.contains_key(&entry) {
                return Err(ApiError::BadRequest(format!(
                    "Entry '{entry}' not found in 'files' map"
                )));
            }
            let work_dir_clone = work_dir.clone();
            let limits_clone = limits.clone();
            let cancel_worker = cancel.clone();
            std::thread::Builder::new()
                .stack_size(EXEC_THREAD_STACK_BYTES)
                .spawn(move || {
                    let result = executor::execute_source_project(
                        &files,
                        &entry,
                        &lang,
                        exec_env,
                        &work_dir_clone,
                        &limits_clone,
                        Some(cancel_worker),
                    );
                    let _ = tx.send(result);
                })
                .map_err(|e| ApiError::Internal(format!("Failed to spawn exec thread: {e}")))?;
        } else if let Some(source) = req.source {
            // Source execution: write code to session FS and run via language runtime
            let lang = req.language.unwrap_or_else(|| "javascript".into());
            // Validate language before spawning so callers get a 400 immediately
            executor::resolve_runtime(&lang)?;
            let work_dir_clone = work_dir.clone();
            let limits_clone = limits.clone();
            let cancel_worker = cancel.clone();
            std::thread::Builder::new()
                .stack_size(EXEC_THREAD_STACK_BYTES)
                .spawn(move || {
                    let result = executor::execute_source(
                        &source,
                        &lang,
                        exec_env,
                        &work_dir_clone,
                        &limits_clone,
                        Some(cancel_worker),
                    );
                    let _ = tx.send(result);
                })
                .map_err(|e| ApiError::Internal(format!("Failed to spawn exec thread: {e}")))?;
        } else if let Some(wasm_path) = req.wasm_path.as_deref() {
            // WASM file execution: load from session filesystem and run directly
            let resolved = resolve_session_path(&work_dir, wasm_path)?;
            let wasm_bytes = std::fs::read(&resolved)
                .map_err(|e| ApiError::NotFound(format!("{}: {e}", resolved.display())))?;
            let function = req.function.clone();
            let args = req.args.clone();
            let cancel_worker = cancel.clone();
            std::thread::Builder::new()
                .stack_size(EXEC_THREAD_STACK_BYTES)
                .spawn(move || {
                    let result = execute_wasm_bytes_with_env(
                        &wasm_bytes,
                        exec_env,
                        function,
                        args,
                        exec_limits,
                        Some(cancel_worker),
                    )
                    .map_err(|e| ApiError::Internal(e.to_string()));
                    let _ = tx.send(result);
                })
                .map_err(|e| ApiError::Internal(format!("Failed to spawn exec thread: {e}")))?;
        } else {
            return Err(ApiError::BadRequest(
                "Missing command, wasm_path, source, or files".into(),
            ));
        }

        let duration_ms;
        let exec_result = match rx.recv_timeout(timeout) {
            Ok(result) => {
                duration_ms = start.elapsed().as_millis() as u64;
                result
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Trip the cancel flag so the detached worker stops executing
                // instructions instead of running on past the timeout. (No-op
                // for the shell path, which isn't a long-running interpreter.)
                cancel.store(true, Ordering::Relaxed);
                duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ExecResponse {
                    stdout: read_env_stdout(&wasi_env),
                    stderr: read_env_stderr(&wasi_env),
                    exit_code: -1,
                    duration_ms,
                    output_truncated: read_env_truncated(&wasi_env),
                    error: Some(format!("Execution timed out after {timeout_secs}s")),
                });
            }
            Err(_) => {
                duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ExecResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: -1,
                    duration_ms,
                    output_truncated: false,
                    error: Some("Execution thread panicked".into()),
                });
            }
        };

        match exec_result {
            Ok(exit_code) => Ok(ExecResponse {
                stdout: read_env_stdout(&wasi_env),
                stderr: read_env_stderr(&wasi_env),
                exit_code,
                duration_ms,
                output_truncated: read_env_truncated(&wasi_env),
                error: None,
            }),
            Err(e) => Ok(ExecResponse {
                stdout: read_env_stdout(&wasi_env),
                stderr: read_env_stderr(&wasi_env),
                exit_code: -1,
                duration_ms,
                output_truncated: read_env_truncated(&wasi_env),
                error: Some(e.to_string()),
            }),
        }
    }

    // ── File endpoints ────────────────────────────────────────────

    pub fn handle_write_file(
        &self,
        id: &str,
        body: &str,
    ) -> std::result::Result<MessageResponse, ApiError> {
        let req: WriteFileRequest =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let (work_dir, limits) = self
            .session_manager
            .get_session(id, |s| (s.work_dir().to_path_buf(), s.limits().clone()))
            .map_err(map_session_err)?;

        let resolved = resolve_session_path(&work_dir, &req.path)?;

        // Enforce per-file size and total disk caps before writing.
        let existing_len = std::fs::metadata(&resolved).map(|m| m.len()).unwrap_or(0);
        limits
            .check_write(req.content.len() as u64, existing_len, dir_size(&work_dir))
            .map_err(ApiError::BadRequest)?;

        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ApiError::Internal(format!("mkdir: {e}")))?;
        }

        std::fs::write(&resolved, &req.content)
            .map_err(|e| ApiError::Internal(format!("write: {e}")))?;

        Ok(MessageResponse {
            message: format!("Written: {}", req.path),
        })
    }

    pub fn handle_read_file(
        &self,
        id: &str,
        path: &str,
    ) -> std::result::Result<ReadFileResponse, ApiError> {
        let work_dir = self
            .session_manager
            .get_session(id, |s| s.work_dir().to_path_buf())
            .map_err(map_session_err)?;

        let resolved = resolve_session_path(&work_dir, path)?;
        let content = std::fs::read_to_string(&resolved)
            .map_err(|e| ApiError::NotFound(format!("{path}: {e}")))?;

        Ok(ReadFileResponse {
            path: path.to_string(),
            content,
        })
    }

    pub fn handle_list_files(
        &self,
        id: &str,
        path: &str,
    ) -> std::result::Result<ListFilesResponse, ApiError> {
        let work_dir = self
            .session_manager
            .get_session(id, |s| s.work_dir().to_path_buf())
            .map_err(map_session_err)?;

        let resolved = resolve_session_path(&work_dir, path)?;
        let entries = std::fs::read_dir(&resolved)
            .map_err(|e| ApiError::NotFound(format!("{path}: {e}")))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let meta = entry.metadata().ok()?;
                Some(FileEntry {
                    name: entry.file_name().to_string_lossy().into(),
                    is_dir: meta.is_dir(),
                    size: meta.len(),
                })
            })
            .collect();

        Ok(ListFilesResponse {
            path: path.to_string(),
            entries,
        })
    }

    pub fn handle_delete_file(
        &self,
        id: &str,
        path: &str,
    ) -> std::result::Result<MessageResponse, ApiError> {
        if path.is_empty() {
            return Err(ApiError::BadRequest("Missing path parameter".into()));
        }

        let work_dir = self
            .session_manager
            .get_session(id, |s| s.work_dir().to_path_buf())
            .map_err(map_session_err)?;

        let resolved = resolve_session_path(&work_dir, path)?;

        if resolved.is_dir() {
            std::fs::remove_dir_all(&resolved)
                .map_err(|e| ApiError::NotFound(format!("{path}: {e}")))?;
        } else {
            std::fs::remove_file(&resolved)
                .map_err(|e| ApiError::NotFound(format!("{path}: {e}")))?;
        }

        Ok(MessageResponse {
            message: format!("Deleted: {path}"),
        })
    }

    // ── Env endpoints ─────────────────────────────────────────────

    pub fn handle_set_env(
        &self,
        id: &str,
        body: &str,
    ) -> std::result::Result<MessageResponse, ApiError> {
        let vars: HashMap<String, String> =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        self.session_manager
            .get_session(id, |s| {
                for (k, v) in &vars {
                    s.set_env(k, v);
                }
            })
            .map_err(map_session_err)?;

        Ok(MessageResponse {
            message: format!("Set {} environment variable(s)", vars.len()),
        })
    }

    pub fn handle_get_env(&self, id: &str) -> std::result::Result<EnvVarsResponse, ApiError> {
        let env = self
            .session_manager
            .get_session(id, |s| {
                let wasi = s.wasi_env();
                let locked = wasi.lock().unwrap();
                locked
                    .env_vars()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<HashMap<_, _>>()
            })
            .map_err(map_session_err)?;

        Ok(EnvVarsResponse { env })
    }

    // ── Tools endpoint ──────────────────────────────────────────

    pub fn handle_get_tools(
        &self,
        format: &str,
    ) -> std::result::Result<serde_json::Value, ApiError> {
        match format {
            "anthropic" => serde_json::to_value(tools::anthropic_tools())
                .map_err(|e| ApiError::Internal(e.to_string())),
            _ => serde_json::to_value(tools::openai_tools())
                .map_err(|e| ApiError::Internal(e.to_string())),
        }
    }

    // ── Response helpers ──────────────────────────────────────────

    fn respond_json<T: Serialize>(
        &self,
        request: Request,
        result: std::result::Result<T, ApiError>,
    ) -> Result<()> {
        let (status, body) = match result {
            Ok(data) => (200, serde_json::to_string(&data).unwrap_or_default()),
            Err(e) => {
                let code = e.status_code();
                let body = serde_json::to_string(&e.to_error_response()).unwrap_or_default();
                (code, body)
            }
        };
        let mut response = Response::from_string(body).with_status_code(StatusCode(status));
        for h in self.cors_headers() {
            response = response.with_header(h);
        }
        request
            .respond(response)
            .map_err(|e| WasmrunError::from(format!("Response error: {e}")))
    }

    fn respond_empty(&self, request: Request, status: u16) -> Result<()> {
        let mut response = Response::from_string("").with_status_code(StatusCode(status));
        for h in self.cors_headers() {
            response = response.with_header(h);
        }
        request
            .respond(response)
            .map_err(|e| WasmrunError::from(format!("Response error: {e}")))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Format a memory page count as a human-readable MB string for the banner.
fn fmt_pages_mb(pages: Option<u32>) -> String {
    match pages {
        Some(p) => format!("{} MB / session", (p as u64 * 65536) / (1024 * 1024)),
        None => "unlimited".to_string(),
    }
}

/// Format an optional byte cap as a human-readable MB string for the banner.
fn fmt_bytes_mb(bytes: Option<u64>) -> String {
    match bytes {
        Some(b) => format!("{} MB / session", b / (1024 * 1024)),
        None => "unlimited".to_string(),
    }
}

/// Format an optional numeric limit with a unit label for the banner.
fn fmt_opt_u64(val: Option<u64>, unit: &str) -> String {
    match val {
        Some(v) => format!("{v} {unit}"),
        None => "unlimited".to_string(),
    }
}

fn map_session_err(e: SessionError) -> ApiError {
    match e {
        SessionError::NotFound { id } => ApiError::SessionNotFound(id),
        SessionError::Expired { id } => ApiError::SessionExpired(id),
        SessionError::MaxSessionsReached { max } => ApiError::MaxSessions(max),
        SessionError::IoError { message } => ApiError::Internal(message),
        SessionError::LockError => ApiError::Internal("Lock error".into()),
    }
}

fn resolve_session_path(
    work_dir: &Path,
    guest_path: &str,
) -> std::result::Result<PathBuf, ApiError> {
    let cleaned = guest_path.trim_start_matches('/');
    for component in Path::new(cleaned).components() {
        if let Component::ParentDir = component {
            return Err(ApiError::BadRequest("Path traversal not allowed".into()));
        }
    }
    Ok(work_dir.join(cleaned))
}

fn read_body(reader: &mut dyn Read) -> Result<String> {
    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .map_err(|e| WasmrunError::from(format!("Failed to read request body: {e}")))?;
    Ok(body)
}

fn split_url(url: &str) -> (String, String) {
    match url.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (url.to_string(), String::new()),
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((k.to_string(), url_decode(v)))
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(hex_val);
            let lo = chars.next().and_then(hex_val);
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h << 4 | l) as char);
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn read_env_stdout(
    env: &std::sync::Arc<std::sync::Mutex<crate::runtime::wasi::WasiEnv>>,
) -> String {
    env.lock()
        .map(|e| String::from_utf8_lossy(&e.get_stdout()).into_owned())
        .unwrap_or_default()
}

fn read_env_stderr(
    env: &std::sync::Arc<std::sync::Mutex<crate::runtime::wasi::WasiEnv>>,
) -> String {
    env.lock()
        .map(|e| String::from_utf8_lossy(&e.get_stderr()).into_owned())
        .unwrap_or_default()
}

fn read_env_truncated(
    env: &std::sync::Arc<std::sync::Mutex<crate::runtime::wasi::WasiEnv>>,
) -> bool {
    env.lock().map(|e| e.output_truncated()).unwrap_or(false)
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_server() -> AgentServer {
        AgentServer::new(AgentConfig {
            port: 0,
            session_config: SessionConfig {
                default_timeout: Duration::from_secs(60),
                max_sessions: 10,
                cleanup_interval: Duration::from_secs(300),
                limits: crate::agent::limits::ResourceLimits::default(),
            },
            allow_cors: true,
            verbose: false,
        })
    }

    // Hand-built WASM that calls fd_write to print "Hello, World!\n"
    fn hello_wasm() -> Vec<u8> {
        #[rustfmt::skip]
        let wasm: Vec<u8> = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x0c, 0x02,
            0x60, 0x04, 0x7f, 0x7f, 0x7f, 0x7f, 0x01, 0x7f,
            0x60, 0x00, 0x00,
            0x02, 0x23, 0x01,
            0x16,
            0x77, 0x61, 0x73, 0x69, 0x5f, 0x73, 0x6e, 0x61,
            0x70, 0x73, 0x68, 0x6f, 0x74, 0x5f, 0x70, 0x72,
            0x65, 0x76, 0x69, 0x65, 0x77, 0x31,
            0x08,
            0x66, 0x64, 0x5f, 0x77, 0x72, 0x69, 0x74, 0x65,
            0x00, 0x00,
            0x03, 0x02, 0x01, 0x01,
            0x05, 0x03, 0x01, 0x00, 0x01,
            0x07, 0x13, 0x02,
            0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
            0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74, 0x00, 0x01,
            0x0a, 0x1d, 0x01, 0x1b, 0x00,
            0x41, 0x00, 0x41, 0x10, 0x36, 0x02, 0x00,
            0x41, 0x04, 0x41, 0x0e, 0x36, 0x02, 0x00,
            0x41, 0x01, 0x41, 0x00, 0x41, 0x01, 0x41, 0x08,
            0x10, 0x00, 0x1a, 0x0b,
            0x0b, 0x14, 0x01, 0x00,
            0x41, 0x10, 0x0b, 0x0e,
            0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20,
            0x57, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x0a,
        ];
        wasm
    }

    // ── Session lifecycle ─────────────────────────────────────────

    #[test]
    fn test_create_session() {
        let server = test_server();
        let resp = server.handle_create_session().unwrap();
        assert_eq!(resp.session_id.len(), 32);
        assert!(!resp.created_at.is_empty());
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_get_session() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let resp = server.handle_get_session(&id).unwrap();
        assert_eq!(resp.session_id, id);
        assert_eq!(resp.state, "active");
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_delete_session() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        server.handle_delete_session(&id).unwrap();
        assert!(server.handle_get_session(&id).is_err());
    }

    #[test]
    fn test_session_not_found() {
        let server = test_server();
        let err = server.handle_get_session("nonexistent").unwrap_err();
        assert_eq!(err.status_code(), 404);
    }

    // ── File CRUD ─────────────────────────────────────────────────

    #[test]
    fn test_write_and_read_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(&id, r#"{"path": "test.txt", "content": "hello agent"}"#)
            .unwrap();

        let resp = server.handle_read_file(&id, "test.txt").unwrap();
        assert_eq!(resp.content, "hello agent");

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_write_nested_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(&id, r#"{"path": "sub/dir/file.txt", "content": "nested"}"#)
            .unwrap();

        let resp = server.handle_read_file(&id, "sub/dir/file.txt").unwrap();
        assert_eq!(resp.content, "nested");

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_list_files() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "a"}"#)
            .unwrap();
        server
            .handle_write_file(&id, r#"{"path": "b.txt", "content": "bb"}"#)
            .unwrap();

        let resp = server.handle_list_files(&id, "/").unwrap();
        assert_eq!(resp.entries.len(), 2);

        let names: Vec<&str> = resp.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_delete_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(&id, r#"{"path": "del.txt", "content": "x"}"#)
            .unwrap();

        server.handle_delete_file(&id, "del.txt").unwrap();
        assert!(server.handle_read_file(&id, "del.txt").is_err());

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_read_nonexistent_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let err = server.handle_read_file(&id, "nope.txt").unwrap_err();
        assert_eq!(err.status_code(), 404);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_path_traversal_rejected() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let err = server
            .handle_read_file(&id, "../../../etc/passwd")
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        server.session_manager.destroy_all().unwrap();
    }

    // ── Env ───────────────────────────────────────────────────────

    #[test]
    fn test_set_and_get_env() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_set_env(&id, r#"{"FOO": "bar", "BAZ": "qux"}"#)
            .unwrap();

        let resp = server.handle_get_env(&id).unwrap();
        assert_eq!(resp.env.get("FOO").unwrap(), "bar");
        assert_eq!(resp.env.get("BAZ").unwrap(), "qux");

        server.session_manager.destroy_all().unwrap();
    }

    // ── Exec ──────────────────────────────────────────────────────

    #[test]
    fn test_exec_wasm() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Write the hello WASM to the session
        let wasm = hello_wasm();
        let work_dir = server
            .session_manager
            .get_session(&id, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#)
            .unwrap();

        assert_eq!(resp.stdout, "Hello, World!\n");
        assert_eq!(resp.exit_code, 0);
        assert!(resp.error.is_none());
        assert!(resp.duration_ms < 5000);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_nonexistent_wasm() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let err = server
            .handle_exec(&id, r#"{"wasm_path": "nope.wasm"}"#)
            .unwrap_err();
        assert_eq!(err.status_code(), 404);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_missing_wasm_path_and_source() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let err = server.handle_exec(&id, r#"{}"#).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("wasm_path") || err.to_string().contains("source"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_source_unsupported_language() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Python is not supported yet — should fail immediately without network I/O
        let err = server
            .handle_exec(&id, r#"{"source": "print('hello')", "language": "python"}"#)
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("python"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_files_without_entry_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{"files": {"main.js": "console.log(1)"}}"#;
        let err = server.handle_exec(&id, body).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("entry"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_files_with_unknown_entry_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{"files": {"main.js": "x"}, "entry": "missing.js"}"#;
        let err = server.handle_exec(&id, body).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("missing.js"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_files_with_unsupported_language_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{"files": {"a.py": "print(1)"}, "entry": "a.py", "language": "python"}"#;
        let err = server.handle_exec(&id, body).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("python"));

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: fetches the nodejs runtime from wasmhub and verifies
    /// that all files in a multi-file project are written to the session FS and
    /// the entry file executes. Sibling files are visible in the session
    /// directory; whether they can be loaded depends on the runtime's module
    /// system (the QuickJS-based nodejs runtime currently lacks `require`).
    ///
    /// Ignored by default so the test suite stays offline-friendly. Run with:
    ///   cargo test --release --bin wasmrun multi_file_js_project_integration -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_multi_file_js_project_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "files": {
                "main.js": "console.log('main-ran');",
                "extra.js": "// sibling file, just present"
            },
            "entry": "main.js",
            "timeout": 60
        }"#;
        let resp = server.handle_exec(&id, body).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("main-ran"),
            "stdout did not contain expected output: {:?}",
            resp.stdout
        );

        // Verify the sibling file was actually written to the session FS
        let extra = server.handle_read_file(&id, "extra.js").unwrap();
        assert!(extra.content.contains("sibling file"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_files_routes_to_project_execution() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // With valid files+entry, request should reach the execution stage and
        // return Ok (any runtime fetch failure surfaces as ExecResponse.error,
        // not an ApiError from handle_exec itself).
        let body = r#"{"files": {"main.js": "console.log('ok')"}, "entry": "main.js"}"#;
        let result = server.handle_exec(&id, body);
        assert!(
            result.is_ok(),
            "valid files+entry should not return ApiError, got: {result:?}"
        );

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_source_defaults_to_javascript() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Omitting "language" with "source" present should not produce a BadRequest
        // (it defaults to javascript). We can't verify full execution without the runtime,
        // but we verify the request parses and reaches the execution stage (not a 400).
        // The exec thread may return an Internal error if the runtime is unavailable, which
        // surfaces as ExecResponse.error — not an ApiError from handle_exec itself.
        let result = server.handle_exec(&id, r#"{"source": "1+1"}"#);
        assert!(
            result.is_ok(),
            "default language should not return ApiError"
        );

        server.session_manager.destroy_all().unwrap();
    }

    // ── Shell command exec ────────────────────────────────────────

    #[test]
    fn test_exec_command_echo() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let resp = server
            .handle_exec(&id, r#"{"command": "echo hello"}"#)
            .unwrap();
        assert_eq!(resp.exit_code, 0);
        assert_eq!(resp.stdout, "hello\n");
        assert!(resp.error.is_none());

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_command_redirect_then_cat() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let resp = server
            .handle_exec(
                &id,
                r#"{"command": "echo persisted > log.txt && cat log.txt"}"#,
            )
            .unwrap();
        assert_eq!(resp.exit_code, 0);
        assert_eq!(resp.stdout, "persisted\n");

        // Verify the file is actually in the session work_dir
        let content = server.handle_read_file(&id, "log.txt").unwrap();
        assert_eq!(content.content, "persisted\n");

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_command_takes_precedence_over_wasm_path() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // wasm_path points at a nonexistent file but command should win.
        let resp = server
            .handle_exec(
                &id,
                r#"{"command": "echo first", "wasm_path": "nope.wasm"}"#,
            )
            .unwrap();
        assert_eq!(resp.exit_code, 0);
        assert_eq!(resp.stdout, "first\n");

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_command_export_persists_in_session() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Export via shell, then verify it shows up through the env endpoint.
        server
            .handle_exec(&id, r#"{"command": "export GREETING=hi"}"#)
            .unwrap();

        let env = server.handle_get_env(&id).unwrap();
        assert_eq!(env.env.get("GREETING").map(|s| s.as_str()), Some("hi"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_command_parse_error_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Unclosed quote → parse error → BadRequest
        let resp = server
            .handle_exec(&id, r#"{"command": "echo \"oops"}"#)
            .unwrap();
        // Parse error is surfaced via ExecResponse.error from the exec thread.
        assert_eq!(resp.exit_code, -1);
        assert!(resp.error.is_some());

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_clears_output_between_calls() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let wasm = hello_wasm();
        let work_dir = server
            .session_manager
            .get_session(&id, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

        // First exec
        let resp1 = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#)
            .unwrap();
        assert_eq!(resp1.stdout, "Hello, World!\n");

        // Second exec should not accumulate
        let resp2 = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#)
            .unwrap();
        assert_eq!(resp2.stdout, "Hello, World!\n");

        server.session_manager.destroy_all().unwrap();
    }

    // ── Full lifecycle ────────────────────────────────────────────

    #[test]
    fn test_full_session_lifecycle() {
        let server = test_server();

        // 1. Create
        let id = server.handle_create_session().unwrap().session_id;

        // 2. Set env
        server.handle_set_env(&id, r#"{"APP": "test"}"#).unwrap();

        // 3. Write WASM file
        let wasm = hello_wasm();
        let work_dir = server
            .session_manager
            .get_session(&id, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

        // 4. Write a data file
        server
            .handle_write_file(&id, r#"{"path": "data.txt", "content": "test data"}"#)
            .unwrap();

        // 5. List files
        let files = server.handle_list_files(&id, "/").unwrap();
        assert!(files.entries.len() >= 2);

        // 6. Execute WASM
        let exec = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#)
            .unwrap();
        assert_eq!(exec.stdout, "Hello, World!\n");
        assert_eq!(exec.exit_code, 0);

        // 7. Read file back
        let content = server.handle_read_file(&id, "data.txt").unwrap();
        assert_eq!(content.content, "test data");

        // 8. Check env
        let env = server.handle_get_env(&id).unwrap();
        assert_eq!(env.env.get("APP").unwrap(), "test");

        // 9. Destroy
        server.handle_delete_session(&id).unwrap();
        assert!(server.handle_get_session(&id).is_err());
    }

    // ── Concurrent sessions ───────────────────────────────────────

    #[test]
    fn test_concurrent_sessions_isolation() {
        let server = Arc::new(test_server());
        let wasm = hello_wasm();

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let srv = server.clone();
                let wasm = wasm.clone();
                std::thread::spawn(move || {
                    let id = srv.handle_create_session().unwrap().session_id;

                    // Each session writes its own file
                    let body = format!(r#"{{"path": "id.txt", "content": "session-{i}"}}"#);
                    srv.handle_write_file(&id, &body).unwrap();

                    // Write and exec WASM
                    let work_dir = srv
                        .session_manager
                        .get_session(&id, |s| s.work_dir().to_path_buf())
                        .unwrap();
                    std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

                    let exec = srv
                        .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#)
                        .unwrap();
                    assert_eq!(exec.stdout, "Hello, World!\n");

                    // Verify isolation
                    let content = srv.handle_read_file(&id, "id.txt").unwrap();
                    assert_eq!(content.content, format!("session-{i}"));

                    id
                })
            })
            .collect();

        let ids: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(ids.len(), 5);

        // Unique session IDs
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), 5);

        server.session_manager.destroy_all().unwrap();
    }

    // ── URL parsing helpers ───────────────────────────────────────

    #[test]
    fn test_split_url() {
        assert_eq!(
            split_url("/api/v1/sessions?foo=bar"),
            ("/api/v1/sessions".into(), "foo=bar".into())
        );
        assert_eq!(
            split_url("/api/v1/sessions"),
            ("/api/v1/sessions".into(), String::new())
        );
    }

    #[test]
    fn test_parse_query() {
        let q = parse_query("path=test.txt&list=true");
        assert_eq!(q.get("path").unwrap(), "test.txt");
        assert_eq!(q.get("list").unwrap(), "true");
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello%20world"), "hello world");
        assert_eq!(url_decode("a+b"), "a b");
        assert_eq!(url_decode("test%2Fpath"), "test/path");
    }

    #[test]
    fn test_resolve_session_path_normal() {
        let work = PathBuf::from("/tmp/session");
        let p = resolve_session_path(&work, "test.txt").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/session/test.txt"));
    }

    #[test]
    fn test_resolve_session_path_strips_leading_slash() {
        let work = PathBuf::from("/tmp/session");
        let p = resolve_session_path(&work, "/test.txt").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/session/test.txt"));
    }

    #[test]
    fn test_resolve_session_path_rejects_traversal() {
        let work = PathBuf::from("/tmp/session");
        assert!(resolve_session_path(&work, "../etc/passwd").is_err());
        assert!(resolve_session_path(&work, "sub/../../etc/passwd").is_err());
    }

    // ── Tools endpoint ────────────────────────────────────────────

    #[test]
    fn test_get_tools_openai_format() {
        let server = test_server();
        let result = server.handle_get_tools("openai").unwrap();
        let tools = result.as_array().unwrap();
        assert_eq!(tools.len(), 6);
        assert_eq!(tools[0]["type"], "function");
        assert!(tools[0]["function"]["name"].is_string());
        assert!(tools[0]["function"]["parameters"].is_object());
    }

    #[test]
    fn test_get_tools_anthropic_format() {
        let server = test_server();
        let result = server.handle_get_tools("anthropic").unwrap();
        let tools = result.as_array().unwrap();
        assert_eq!(tools.len(), 6);
        assert!(tools[0]["input_schema"].is_object());
        // Anthropic format has no "function" wrapper
        assert!(tools[0].get("function").is_none());
    }

    #[test]
    fn test_get_tools_default_is_openai() {
        let server = test_server();
        let result = server.handle_get_tools("unknown").unwrap();
        let tools = result.as_array().unwrap();
        assert_eq!(tools[0]["type"], "function");
    }

    #[test]
    fn test_get_tools_has_all_operations() {
        let server = test_server();
        let result = server.handle_get_tools("openai").unwrap();
        let names: Vec<&str> = result
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"create_session"));
        assert!(names.contains(&"execute_code"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"destroy_session"));
    }

    // ── Resource limits ───────────────────────────────────────────

    /// Hand-built WASM whose `_start` is an infinite `loop { br 0 }`.
    fn infinite_loop_wasm() -> Vec<u8> {
        #[rustfmt::skip]
        let wasm: Vec<u8> = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            // Type section: 1 type ()->()
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            // Function section: 1 func, type 0
            0x03, 0x02, 0x01, 0x00,
            // Export section: "_start" -> func 0
            0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74, 0x00, 0x00,
            // Code section: loop; br 0; end; end
            0x0a, 0x09, 0x01, 0x07, 0x00, 0x03, 0x40, 0x0c, 0x00, 0x0b, 0x0b,
        ];
        wasm
    }

    fn make_session_with_limits(server: &AgentServer, limits: ResourceLimits) -> String {
        server
            .session_manager
            .create_session_with_limits(Duration::from_secs(60), limits)
            .unwrap()
    }

    #[test]
    fn test_create_session_with_limits_override() {
        let server = test_server();
        let body = r#"{"limits":{"max_fuel":500,"max_output_mb":0,"max_file_size_mb":1}}"#;
        let id = server
            .handle_create_session_with_body(body)
            .unwrap()
            .session_id;

        let limits = server
            .session_manager
            .get_session(&id, |s| s.limits().clone())
            .unwrap();
        assert_eq!(limits.max_fuel, Some(500));
        assert_eq!(limits.max_output_bytes, None); // 0 disables the cap
        assert_eq!(limits.max_file_size, Some(1024 * 1024));
        // Unspecified fields keep the server defaults.
        assert_eq!(
            limits.max_memory_pages,
            server.config.session_config.limits.max_memory_pages
        );

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_create_session_empty_body_uses_defaults() {
        let server = test_server();
        let id = server
            .handle_create_session_with_body("")
            .unwrap()
            .session_id;
        let limits = server
            .session_manager
            .get_session(&id, |s| s.limits().clone())
            .unwrap();
        assert_eq!(limits, server.config.session_config.limits);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_create_session_invalid_limits_body_returns_400() {
        let server = test_server();
        let err = server
            .handle_create_session_with_body(r#"{"limits": "not-an-object"}"#)
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
    }

    #[test]
    fn test_write_file_exceeds_file_size_limit() {
        let server = test_server();
        let limits = ResourceLimits {
            max_file_size: Some(10),
            max_disk_bytes: None,
            ..ResourceLimits::default()
        };
        let id = make_session_with_limits(&server, limits);

        let err = server
            .handle_write_file(
                &id,
                r#"{"path": "big.txt", "content": "this is more than ten bytes"}"#,
            )
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("File size limit"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_write_file_exceeds_disk_limit() {
        let server = test_server();
        let limits = ResourceLimits {
            max_file_size: None,
            max_disk_bytes: Some(10),
            ..ResourceLimits::default()
        };
        let id = make_session_with_limits(&server, limits);

        // First 5-byte file fits (5 <= 10).
        server
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "12345"}"#)
            .unwrap();
        // Second 6-byte file pushes total to 11 > 10 → rejected.
        let err = server
            .handle_write_file(&id, r#"{"path": "b.txt", "content": "678901"}"#)
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Disk usage limit"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_command_output_truncated() {
        let server = test_server();
        let limits = ResourceLimits {
            max_output_bytes: Some(3),
            ..ResourceLimits::default()
        };
        let id = make_session_with_limits(&server, limits);

        // "echo hello" emits "hello\n" (6 bytes); capped to 3.
        let resp = server
            .handle_exec(&id, r#"{"command": "echo hello"}"#)
            .unwrap();
        assert_eq!(resp.stdout, "hel");
        assert!(resp.output_truncated);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_fuel_limit_aborts_runaway_wasm() {
        let server = test_server();
        let limits = ResourceLimits {
            max_fuel: Some(50_000),
            ..ResourceLimits::default()
        };
        let id = make_session_with_limits(&server, limits);

        let work_dir = server
            .session_manager
            .get_session(&id, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("loop.wasm"), infinite_loop_wasm()).unwrap();

        // With a fuel cap the runaway loop aborts well before the exec timeout.
        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "loop.wasm", "timeout": 30}"#)
            .unwrap();
        assert_eq!(resp.exit_code, -1);
        let err = resp.error.unwrap_or_default();
        assert!(
            err.contains("instruction limit") || err.contains("fuel"),
            "expected fuel-limit error, got: {err}"
        );
        assert!(resp.duration_ms < 30_000);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_timeout_cancels_runaway_wasm_without_fuel() {
        // No fuel cap → only the wall-clock timeout can stop the loop. The
        // worker must self-terminate via the cancel flag, freeing the session
        // so a follow-up exec still completes promptly.
        let server = test_server();
        let limits = ResourceLimits {
            max_fuel: None,
            ..ResourceLimits::default()
        };
        let id = make_session_with_limits(&server, limits);

        let work_dir = server
            .session_manager
            .get_session(&id, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("loop.wasm"), infinite_loop_wasm()).unwrap();
        std::fs::write(work_dir.join("hello.wasm"), hello_wasm()).unwrap();

        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "loop.wasm", "timeout": 1}"#)
            .unwrap();
        assert_eq!(resp.exit_code, -1);
        assert!(
            resp.error.unwrap_or_default().contains("timed out"),
            "expected a timeout error"
        );
        // ~1s timeout, well under any runaway ceiling.
        assert!(resp.duration_ms < 5_000);

        // The session is still usable: a normal exec runs and returns promptly,
        // which it could not if the runaway worker were still pinning the core.
        let ok = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm", "timeout": 10}"#)
            .unwrap();
        assert_eq!(ok.stdout, "Hello, World!\n");
        assert_eq!(ok.exit_code, 0);

        server.session_manager.destroy_all().unwrap();
    }
}
