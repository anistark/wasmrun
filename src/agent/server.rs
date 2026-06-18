//! Agent mode: REST API server for AI agent sandbox management.

use crate::agent::api::*;
use crate::agent::auth::{AuthConfig, TenantRate};
use crate::agent::executor;
use crate::agent::limits::{dir_size, LimitsOverride, ResourceLimits};
use crate::agent::metrics::{Gauges, Metrics, SessionResourceRow};
use crate::agent::session::{SessionConfig, SessionError, SessionManager, SessionState};
use crate::agent::shell;
use crate::agent::tools;
use crate::error::{Result, WasmrunError};
use crate::runtime::core::native_executor::{execute_wasm_bytes_with_env, ExecLimits};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const API_PREFIX: &str = "/api/v1";
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 30;
// Language runtimes (e.g. QuickJS compiled to WASM) generate deep call chains that
// overflow the default 8 MB thread stack when run through the WASM interpreter.
const EXEC_THREAD_STACK_BYTES: usize = 64 * 1024 * 1024;
/// Default request body cap (32 MB) when none is configured.
const DEFAULT_MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
/// Default ceiling on concurrent exec workers when none is configured.
const DEFAULT_MAX_CONCURRENT_EXEC: usize = 100;

pub struct AgentConfig {
    pub port: u16,
    pub session_config: SessionConfig,
    pub allow_cors: bool,
    pub verbose: bool,
    /// Maximum accepted request body size in bytes. `None` = unlimited.
    pub max_body_bytes: Option<usize>,
    /// Maximum number of exec workers allowed to run concurrently across all
    /// sessions. `0` = unlimited. Bounds thread / stack / memory footprint
    /// independently of `max_sessions` (which only bounds session count).
    pub max_concurrent_exec: usize,
    /// API-key authentication. `None` = open mode (no auth; back-compat). When
    /// `Some`, every `/api/v1/*` request must present a valid `Bearer` key and
    /// sessions are isolated per tenant.
    pub auth: Option<Arc<AuthConfig>>,
    /// Path to the auth config file, retained so the server can watch it for
    /// live reloads. `None` when `--auth` was not given (open mode).
    pub auth_path: Option<PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            port: 8430,
            session_config: SessionConfig::default(),
            allow_cors: false,
            verbose: false,
            max_body_bytes: Some(DEFAULT_MAX_BODY_BYTES),
            max_concurrent_exec: DEFAULT_MAX_CONCURRENT_EXEC,
            auth: None,
            auth_path: None,
        }
    }
}

/// Non-blocking counting semaphore bounding concurrent exec workers.
///
/// `max == 0` means unlimited. [`try_acquire`](ExecSlots::try_acquire) never
/// blocks: it either returns a permit or `None` (caller responds 429). A permit
/// is released when its guard is dropped — and because the guard is moved into
/// the exec worker thread, release happens on *worker completion*, not when the
/// HTTP response returns. This keeps a slot held by a timed-out-but-still-running
/// worker until cooperative cancellation actually stops it.
struct ExecSlots {
    in_flight: AtomicUsize,
    max: usize,
}

impl ExecSlots {
    fn new(max: usize) -> Arc<Self> {
        Arc::new(Self {
            in_flight: AtomicUsize::new(0),
            max,
        })
    }

    /// Current number of exec workers holding a slot. Read live for the
    /// `exec_in_flight` metrics gauge.
    fn in_flight(&self) -> u64 {
        self.in_flight.load(Ordering::Acquire) as u64
    }

    /// Try to take a slot. Returns `None` when saturated (caller → 429).
    fn try_acquire(self: &Arc<Self>) -> Option<ExecPermit> {
        if self.max == 0 {
            return Some(ExecPermit { slots: None });
        }
        let mut cur = self.in_flight.load(Ordering::Acquire);
        loop {
            if cur >= self.max {
                return None;
            }
            match self.in_flight.compare_exchange_weak(
                cur,
                cur + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(ExecPermit {
                        slots: Some(self.clone()),
                    })
                }
                Err(actual) => cur = actual,
            }
        }
    }
}

/// RAII permit for a slot in [`ExecSlots`]. Releases the slot on drop.
struct ExecPermit {
    slots: Option<Arc<ExecSlots>>,
}

impl Drop for ExecPermit {
    fn drop(&mut self) {
        if let Some(slots) = &self.slots {
            slots.in_flight.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Bundles the global and per-tenant exec permits so both slots release together
/// when the worker completes. The worker closures move this in and drop it on
/// completion — the same RAII discipline the single global permit used before.
struct HeldPermits {
    _global: ExecPermit,
    _tenant: ExecPermit,
}

/// Fixed-window per-tenant request counter for the requests/min cap.
///
/// `max_per_min == 0` means unlimited. The window is a simple fixed interval
/// reset when a minute elapses — cheap and dependency-free. A burst can straddle
/// a window boundary (up to ~2× the cap across two adjacent windows); a smoothing
/// token-bucket is left as a later refinement.
struct RateWindow {
    max_per_min: u64,
    state: Mutex<(Instant, u64)>,
}

impl RateWindow {
    fn new(max_per_min: u64) -> Self {
        Self {
            max_per_min,
            state: Mutex::new((Instant::now(), 0)),
        }
    }

    /// Record one request. Returns `false` when it exceeds the window cap.
    fn allow(&self) -> bool {
        if self.max_per_min == 0 {
            return true;
        }
        // Recover the guard even if poisoned: never fail-closed on a panic
        // elsewhere (throttling is best-effort, not a correctness invariant).
        let mut g = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (start, count) = &mut *g;
        if start.elapsed() >= Duration::from_secs(60) {
            *start = Instant::now();
            *count = 0;
        }
        if *count >= self.max_per_min {
            return false;
        }
        *count += 1;
        true
    }
}

/// Per-tenant concurrency + request-rate limiter.
///
/// Lazily creates a sized [`ExecSlots`] and [`RateWindow`] per tenant on first
/// use; only consulted in auth mode (open mode has no tenant). Ceilings come from
/// the tenant's `[tenants.rate]` table. Note: an entry is sized at first use, so
/// a live config reload (0.20.6c) does not resize an already-created entry — a
/// known, acceptable limitation revisited there.
struct TenantLimiter {
    exec: RwLock<HashMap<String, Arc<ExecSlots>>>,
    windows: RwLock<HashMap<String, Arc<RateWindow>>>,
}

impl TenantLimiter {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            exec: RwLock::new(HashMap::new()),
            windows: RwLock::new(HashMap::new()),
        })
    }

    /// Get-or-create the tenant's exec slots, sized to `max` (`0` = unlimited).
    fn exec_slots(&self, tenant: &str, max: usize) -> Arc<ExecSlots> {
        if let Some(s) = self
            .exec
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(tenant)
        {
            return s.clone();
        }
        self.exec
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(tenant.to_string())
            .or_insert_with(|| ExecSlots::new(max))
            .clone()
    }

    /// Get-or-create the tenant's request-rate window (`max` req/min; `0` = off).
    fn window(&self, tenant: &str, max: u64) -> Arc<RateWindow> {
        if let Some(w) = self
            .windows
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(tenant)
        {
            return w.clone();
        }
        self.windows
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(tenant.to_string())
            .or_insert_with(|| Arc::new(RateWindow::new(max)))
            .clone()
    }
}

pub struct AgentServer {
    session_manager: Arc<SessionManager>,
    config: AgentConfig,
    exec_slots: Arc<ExecSlots>,
    tenant_limiter: Arc<TenantLimiter>,
    metrics: Arc<Metrics>,
    /// Live, swappable auth config (`None` in open mode). Read on every request
    /// via a brief read lock; replaced wholesale by the reload watcher when the
    /// auth file changes. The inner `Arc` makes each request's snapshot cheap.
    live_auth: Option<Arc<RwLock<Arc<AuthConfig>>>>,
    /// The auth file to watch for live reloads (`None` if `--auth` was not set).
    auth_path: Option<PathBuf>,
}

impl AgentServer {
    pub fn new(config: AgentConfig) -> Self {
        let session_manager = Arc::new(SessionManager::with_config(config.session_config.clone()));
        let exec_slots = ExecSlots::new(config.max_concurrent_exec);
        let live_auth = config.auth.clone().map(|a| Arc::new(RwLock::new(a)));
        let auth_path = config.auth_path.clone();
        Self {
            session_manager,
            config,
            exec_slots,
            tenant_limiter: TenantLimiter::new(),
            metrics: Arc::new(Metrics::new()),
            live_auth,
            auth_path,
        }
    }

    /// A cheap snapshot (`Arc` clone) of the current auth config, or `None` in
    /// open mode. Taken under a brief read lock so a concurrent reload swap is
    /// invisible mid-request.
    fn auth_snapshot(&self) -> Option<Arc<AuthConfig>> {
        self.live_auth
            .as_ref()
            .map(|cell| cell.read().unwrap_or_else(|e| e.into_inner()).clone())
    }

    /// The calling tenant's configured rate ceilings, or `None` in open mode or
    /// for an unknown tenant.
    fn tenant_rate(&self, caller: Option<&str>) -> Option<TenantRate> {
        let id = caller?;
        self.auth_snapshot()?.rate(id).cloned()
    }

    /// The calling tenant's operator-assigned limit override, or `None` in open
    /// mode or when the tenant declared no `[tenants.limits]` table.
    fn tenant_limits(&self, caller: Option<&str>) -> Option<LimitsOverride> {
        let id = caller?;
        self.auth_snapshot()?.limits(id).cloned()
    }

    /// Enforce the tenant's requests/min window. `true` = allowed. Always `true`
    /// in open mode or when the tenant set no requests/min cap.
    fn allow_request_rate(&self, caller: Option<&str>) -> bool {
        let Some(tenant) = caller else {
            return true;
        };
        let max = match self.tenant_rate(caller).map(|r| r.max_requests_per_min) {
            Some(m) if m != 0 => m as u64,
            _ => return true,
        };
        self.tenant_limiter.window(tenant, max).allow()
    }

    /// Acquire a per-tenant exec slot. Returns a no-op permit in open mode or
    /// when the tenant has no concurrent-exec cap; `None` when the tenant is
    /// saturated (caller → 429).
    fn try_tenant_exec_permit(&self, caller: Option<&str>) -> Option<ExecPermit> {
        let Some(tenant) = caller else {
            return Some(ExecPermit { slots: None });
        };
        let max = match self.tenant_rate(caller).map(|r| r.max_concurrent_exec) {
            Some(m) if m != 0 => m as usize,
            _ => return Some(ExecPermit { slots: None }),
        };
        self.tenant_limiter.exec_slots(tenant, max).try_acquire()
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

        // Live auth-config reload: watch the auth file for mtime changes and
        // hot-swap the live config (auth mode only). A bad edit is logged and
        // the previous config is kept.
        let auth_watcher = match (&self.auth_path, &self.live_auth) {
            (Some(path), Some(cell)) => Some(spawn_auth_watcher(
                path.clone(),
                cell.clone(),
                self.config.session_config.cleanup_interval,
                shutdown.clone(),
            )),
            _ => None,
        };

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
        if let Some(handle) = auth_watcher {
            let _ = handle.join();
        }
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
        println!(
            "   Max body size:   {}",
            fmt_bytes_mb(self.config.max_body_bytes.map(|b| b as u64))
        );
        println!(
            "   Max concurrent:  {}",
            fmt_count(self.config.max_concurrent_exec, "exec(s)")
        );
        match &self.config.auth {
            Some(auth) => {
                println!(
                    "   Auth:            enabled ({} tenants)",
                    auth.tenant_count()
                );
                if let Some(path) = &self.auth_path {
                    println!("   Auth reload:     watching {}", path.display());
                }
            }
            None => println!("   Auth:            disabled (open)"),
        }
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
        println!("     GET    /metrics                metrics (Prometheus | ?format=json)");
        println!();
    }

    /// CORS headers shared by every response. The `Content-Type` is added
    /// per-response by [`send`](Self::send) so the metrics endpoint can return
    /// `text/plain` while everything else returns `application/json`.
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
        ]
    }

    fn handle_request(&self, mut request: Request) -> Result<()> {
        let method = request.method().clone();
        let url = request.url().to_string();
        let (path, query) = split_url(&url);

        // Per-request context for the structured access log + `X-Request-Id`.
        // Built up front so even early returns (OPTIONS, 401, 413) are logged
        // and carry the id header. `tenant` is filled in after auth resolves.
        let mut log = ReqLog {
            id: generate_request_id(),
            method: method.as_str().to_string(),
            path: path.clone(),
            tenant: "-".to_string(),
            start: Instant::now(),
        };

        if self.config.verbose {
            eprintln!("→ {method} {url} (id={})", log.id);
        }

        if method == Method::Options {
            return self.respond_empty(request, 204, &log);
        }

        // Authentication gate. Resolved once here — after the OPTIONS
        // short-circuit, before routing — so every handler receives an
        // already-validated caller. `None` means open mode (no auth config);
        // `Some(tenant)` is the authenticated tenant id. Auth applies to all
        // `/api/v1/*` routes including `/tools` and `/metrics` (simplest and
        // most secure; the metrics scrape is capped at global aggregates so a
        // tenant key cannot read another tenant's per-session data).
        // Snapshot the live auth config (cheap Arc clone) so a concurrent reload
        // can't change it mid-request. Resolved to an owned tenant id; `None`
        // means open mode.
        let tenant: Option<String> = match self.auth_snapshot() {
            None => None,
            Some(auth) => {
                match bearer_token(&request).and_then(|key| auth.resolve(key).map(String::from)) {
                    Some(t) => {
                        log.tenant = t.clone();
                        Some(t)
                    }
                    None => {
                        self.metrics.record_rejected_unauthorized();
                        let err = ApiError::Unauthorized(
                            "missing or invalid API key (expected 'Authorization: Bearer <key>')"
                                .into(),
                        );
                        return self.respond_json(request, Err::<serde_json::Value, _>(err), &log);
                    }
                }
            }
        };
        // Reborrow as `&str` for the handlers (the owned `String` lives to the
        // end of this function, so the borrow is valid throughout routing).
        let tenant: Option<&str> = tenant.as_deref();

        // Per-tenant requests/min throttle (auth mode only). Checked here — after
        // the tenant resolves, before the body is read — so a flood is rejected
        // cheaply and the cap covers every `/api/v1/*` route uniformly.
        if !self.allow_request_rate(tenant) {
            self.metrics.record_rejected_rate();
            let err = ApiError::RateLimited("requests-per-minute exceeded".into());
            return self.respond_json(request, Err::<serde_json::Value, _>(err), &log);
        }

        // Per-tenant requests/min throttle (auth mode only). Checked here — after
        // the tenant resolves, before the body is read — so a flood is rejected
        // cheaply and the cap covers every `/api/v1/*` route uniformly.
        if !self.allow_request_rate(tenant) {
            self.metrics.record_rejected_rate();
            let err = ApiError::RateLimited("requests-per-minute exceeded".into());
            return self.respond_json(request, Err::<serde_json::Value, _>(err), &log);
        }

        let segments: Vec<&str> = path
            .trim_start_matches(API_PREFIX)
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        // Read the request body once, up front, for methods that carry one.
        // Oversize bodies are rejected (413) before they are fully buffered, so
        // a large POST cannot OOM the process before a handler-level limit runs.
        let body = if method == Method::Post {
            match read_body(request.as_reader(), self.config.max_body_bytes) {
                Ok(b) => b,
                Err(e) => {
                    if matches!(e, ApiError::PayloadTooLarge(_)) {
                        self.metrics.record_rejected_payload();
                    }
                    return self.respond_json(request, Err::<serde_json::Value, _>(e), &log);
                }
            }
        } else {
            String::new()
        };

        match (method, segments.as_slice()) {
            (Method::Get, ["tools"]) => {
                let params = parse_query(&query);
                let format = params.get("format").map(|s| s.as_str()).unwrap_or("openai");
                self.respond_json(request, self.handle_get_tools(format), &log)
            }
            (Method::Get, ["metrics"]) => {
                let params = parse_query(&query);
                // Prometheus text exposition is the scrape default; `?format=json`
                // returns the same data as a flat JSON object.
                match params.get("format").map(|s| s.as_str()) {
                    Some("json") => {
                        self.respond_json(request, Ok::<_, ApiError>(self.metrics_json()), &log)
                    }
                    _ => self.send(
                        request,
                        200,
                        self.metrics_prometheus(),
                        "text/plain; version=0.0.4; charset=utf-8",
                        &log,
                    ),
                }
            }
            (Method::Post, ["sessions"]) => self.respond_json(
                request,
                self.handle_create_session_with_body(&body, tenant),
                &log,
            ),
            (Method::Get, ["sessions", id]) => {
                self.respond_json(request, self.handle_get_session(id, tenant), &log)
            }
            (Method::Delete, ["sessions", id]) => {
                self.respond_json(request, self.handle_delete_session(id, tenant), &log)
            }
            (Method::Post, ["sessions", id, "exec"]) => {
                self.respond_json(request, self.handle_exec(id, &body, tenant), &log)
            }
            (Method::Post, ["sessions", id, "files"]) => {
                self.respond_json(request, self.handle_write_file(id, &body, tenant), &log)
            }
            (Method::Get, ["sessions", id, "files"]) => {
                let params = parse_query(&query);
                let path = params.get("path").map(|s| s.as_str()).unwrap_or("/");
                if params.get("list").map(|v| v == "true").unwrap_or(false) {
                    self.respond_json(request, self.handle_list_files(id, path, tenant), &log)
                } else {
                    self.respond_json(request, self.handle_read_file(id, path, tenant), &log)
                }
            }
            (Method::Delete, ["sessions", id, "files"]) => {
                let params = parse_query(&query);
                let path = params.get("path").map(|s| s.as_str()).unwrap_or("");
                self.respond_json(request, self.handle_delete_file(id, path, tenant), &log)
            }
            (Method::Post, ["sessions", id, "env"]) => {
                self.respond_json(request, self.handle_set_env(id, &body, tenant), &log)
            }
            (Method::Get, ["sessions", id, "env"]) => {
                self.respond_json(request, self.handle_get_env(id, tenant), &log)
            }
            _ => {
                let err = ApiError::NotFound(format!("Unknown endpoint: {path}"));
                self.respond_json(request, Err::<serde_json::Value, _>(err), &log)
            }
        }
    }

    /// Sample the live gauge values at scrape time.
    fn current_gauges(&self) -> Gauges {
        Gauges {
            sessions_active: self.session_manager.active_count() as u64,
            sessions_total: self.session_manager.total_count() as u64,
            exec_in_flight: self.exec_slots.in_flight(),
            sessions_disk_bytes: self.session_manager.total_disk_bytes(),
        }
    }

    fn metrics_prometheus(&self) -> String {
        self.metrics.render_prometheus(&self.current_gauges())
    }

    fn metrics_json(&self) -> serde_json::Value {
        // Compute per-session reports once and derive the disk gauge from them.
        let reports = self.session_manager.session_reports();
        let disk: u64 = reports.iter().map(|r| r.disk_bytes).sum();
        let gauges = Gauges {
            sessions_active: self.session_manager.active_count() as u64,
            sessions_total: self.session_manager.total_count() as u64,
            exec_in_flight: self.exec_slots.in_flight(),
            sessions_disk_bytes: disk,
        };
        // Per-session rows are exposed only in open mode. In auth mode they
        // would leak one tenant's footprint to another, so the scrape stays at
        // global aggregates (0.20.5 Q2/Q3).
        let per_session = if self.config.auth.is_none() {
            Some(
                reports
                    .into_iter()
                    .map(|r| SessionResourceRow {
                        id: r.id,
                        disk_bytes: r.disk_bytes,
                        memory_cap_pages: r.memory_cap_pages,
                    })
                    .collect(),
            )
        } else {
            None
        };
        self.metrics.render_json(&gauges, per_session)
    }

    // ── Session endpoints ─────────────────────────────────────────

    #[allow(dead_code)] // Used by tests; the HTTP route uses the _with_body variant.
    pub fn handle_create_session(&self) -> std::result::Result<CreateSessionResponse, ApiError> {
        self.create_session_with_limits(self.config.session_config.limits.clone(), None)
    }

    /// Create a session, applying any per-session limit overrides supplied in
    /// the (optional) request body on top of the server defaults. `caller` is the
    /// authenticated tenant that will own the session (`None` in open mode).
    pub fn handle_create_session_with_body(
        &self,
        body: &str,
        caller: Option<&str>,
    ) -> std::result::Result<CreateSessionResponse, ApiError> {
        let limits = self.resolve_session_limits(body, caller)?;
        self.create_session_with_limits(limits, caller)
    }

    /// Compose the effective limits for a new session, in three layers:
    ///   1. server defaults (`--max-*` flags),
    ///   2. the tenant's `[tenants.limits]` override → the **tenant baseline**,
    ///   3. the per-session `{"limits":{}}` override, **clamped to the baseline**.
    ///
    /// Clamping makes the tenant limit a hard ceiling: a per-session override may
    /// only tighten a dimension, never raise it above the tenant's cap (a
    /// per-session "unlimited" is pulled down to the tenant's finite ceiling). In
    /// open mode there is no tenant baseline, so this reduces to defaults +
    /// per-session override exactly as before.
    fn resolve_session_limits(
        &self,
        body: &str,
        caller: Option<&str>,
    ) -> std::result::Result<ResourceLimits, ApiError> {
        let defaults = self.config.session_config.limits.clone();
        // The tenant baseline becomes the clamp ceiling, but *only* when the
        // tenant actually declared `[tenants.limits]`. With no tenant override
        // (open mode, or an auth tenant without limits) there is no ceiling, so
        // a per-session override applies un-clamped — preserving the existing
        // behavior where an open-mode override may raise a limit above defaults.
        let tenant_ov = self.tenant_limits(caller);
        let baseline = match &tenant_ov {
            Some(ov) => defaults.with_overrides(ov),
            None => defaults,
        };
        if body.trim().is_empty() {
            return Ok(baseline);
        }
        let req: CreateSessionRequest =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;
        let Some(ov) = req.limits else {
            return Ok(baseline);
        };
        let merged = baseline.with_overrides(&ov);
        Ok(if tenant_ov.is_some() {
            merged.clamp_to(&baseline)
        } else {
            merged
        })
    }

    fn create_session_with_limits(
        &self,
        limits: ResourceLimits,
        owner: Option<&str>,
    ) -> std::result::Result<CreateSessionResponse, ApiError> {
        // Resolve the calling tenant's per-tenant session ceiling (auth mode
        // only; `0`/absent = inherit, i.e. no per-tenant cap beyond the global).
        let owner_session_cap = self
            .tenant_rate(owner)
            .and_then(|r| (r.max_sessions != 0).then_some(r.max_sessions as usize));
        let id = self
            .session_manager
            .create_session_with_limits(
                self.config.session_config.default_timeout,
                limits,
                owner.map(String::from),
                owner_session_cap,
            )
            .map_err(map_session_err)?;
        self.metrics.record_session_created();
        Ok(CreateSessionResponse {
            session_id: id,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    pub fn handle_get_session(
        &self,
        id: &str,
        caller: Option<&str>,
    ) -> std::result::Result<SessionStatusResponse, ApiError> {
        self.session_manager
            .get_session(id, caller, |s| SessionStatusResponse {
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
        caller: Option<&str>,
    ) -> std::result::Result<MessageResponse, ApiError> {
        self.session_manager
            .destroy_session(id, caller)
            .map_err(map_session_err)?;
        Ok(MessageResponse {
            message: format!("Session {id} destroyed"),
        })
    }

    // ── Exec endpoint ─────────────────────────────────────────────

    pub fn handle_exec(
        &self,
        id: &str,
        body: &str,
        caller: Option<&str>,
    ) -> std::result::Result<ExecResponse, ApiError> {
        let req: ExecRequest =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let (wasi_env, work_dir, limits) = self
            .session_manager
            .get_session(id, caller, |s| {
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

        // Bound concurrent exec workers globally. The permit is moved into the
        // spawned worker so its slot is released on *worker completion* (not when
        // this HTTP response returns) — a timed-out-but-still-running worker keeps
        // its slot until cooperative cancellation actually stops it. On saturation
        // reject with 429 before spawning a fresh 64 MB-stack thread.
        let permit = match self.exec_slots.try_acquire() {
            Some(p) => p,
            None => {
                self.metrics.record_rejected_concurrency();
                return Err(ApiError::TooManyRequests(self.config.max_concurrent_exec));
            }
        };
        // Per-tenant concurrent-exec cap (auth mode only). Acquired after the
        // global slot; both are bundled so they release together on worker
        // completion (a timed-out-but-running worker keeps both until cancelled).
        let tenant_permit = match self.try_tenant_exec_permit(caller) {
            Some(p) => p,
            None => {
                self.metrics.record_rejected_rate();
                return Err(ApiError::RateLimited(
                    "per-tenant concurrent execution limit reached".into(),
                ));
            }
        };
        let permit = HeldPermits {
            _global: permit,
            _tenant: tenant_permit,
        };

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
                    let permit = permit; // held for the duration of execution
                    let result = shell::run_command(&command, &work_dir_clone, exec_env)
                        .map_err(|e| ApiError::BadRequest(e.to_string()));
                    drop(permit); // free the slot once execution is done
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
                    let permit = permit; // held for the duration of execution
                    let result = executor::execute_source_project(
                        &files,
                        &entry,
                        &lang,
                        exec_env,
                        &work_dir_clone,
                        &limits_clone,
                        Some(cancel_worker),
                    );
                    drop(permit); // free the slot once execution is done
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
                    let permit = permit; // held for the duration of execution
                    let result = executor::execute_source(
                        &source,
                        &lang,
                        exec_env,
                        &work_dir_clone,
                        &limits_clone,
                        Some(cancel_worker),
                    );
                    drop(permit); // free the slot once execution is done
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
                    let permit = permit; // held for the duration of execution
                    let result = execute_wasm_bytes_with_env(
                        &wasm_bytes,
                        exec_env,
                        function,
                        args,
                        exec_limits,
                        Some(cancel_worker),
                    )
                    .map_err(|e| ApiError::Internal(e.to_string()));
                    drop(permit); // free the slot once execution is done
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
                let truncated = read_env_truncated(&wasi_env);
                self.metrics.record_exec_timeout(duration_ms);
                if truncated {
                    self.metrics.record_output_truncated();
                }
                return Ok(ExecResponse {
                    stdout: read_env_stdout(&wasi_env),
                    stderr: read_env_stderr(&wasi_env),
                    exit_code: -1,
                    duration_ms,
                    output_truncated: truncated,
                    error: Some(format!("Execution timed out after {timeout_secs}s")),
                });
            }
            Err(_) => {
                duration_ms = start.elapsed().as_millis() as u64;
                self.metrics.record_exec_error(duration_ms);
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

        let truncated = read_env_truncated(&wasi_env);
        if truncated {
            self.metrics.record_output_truncated();
        }
        match exec_result {
            Ok(exit_code) => {
                self.metrics.record_exec_success(duration_ms);
                Ok(ExecResponse {
                    stdout: read_env_stdout(&wasi_env),
                    stderr: read_env_stderr(&wasi_env),
                    exit_code,
                    duration_ms,
                    output_truncated: truncated,
                    error: None,
                })
            }
            Err(e) => {
                self.metrics.record_exec_error(duration_ms);
                Ok(ExecResponse {
                    stdout: read_env_stdout(&wasi_env),
                    stderr: read_env_stderr(&wasi_env),
                    exit_code: -1,
                    duration_ms,
                    output_truncated: truncated,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    // ── File endpoints ────────────────────────────────────────────

    pub fn handle_write_file(
        &self,
        id: &str,
        body: &str,
        caller: Option<&str>,
    ) -> std::result::Result<MessageResponse, ApiError> {
        let req: WriteFileRequest =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let (work_dir, limits) = self
            .session_manager
            .get_session(id, caller, |s| {
                (s.work_dir().to_path_buf(), s.limits().clone())
            })
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
        caller: Option<&str>,
    ) -> std::result::Result<ReadFileResponse, ApiError> {
        let work_dir = self
            .session_manager
            .get_session(id, caller, |s| s.work_dir().to_path_buf())
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
        caller: Option<&str>,
    ) -> std::result::Result<ListFilesResponse, ApiError> {
        let work_dir = self
            .session_manager
            .get_session(id, caller, |s| s.work_dir().to_path_buf())
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
        caller: Option<&str>,
    ) -> std::result::Result<MessageResponse, ApiError> {
        if path.is_empty() {
            return Err(ApiError::BadRequest("Missing path parameter".into()));
        }

        let work_dir = self
            .session_manager
            .get_session(id, caller, |s| s.work_dir().to_path_buf())
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
        caller: Option<&str>,
    ) -> std::result::Result<MessageResponse, ApiError> {
        let vars: HashMap<String, String> =
            serde_json::from_str(body).map_err(|e| ApiError::BadRequest(e.to_string()))?;

        self.session_manager
            .get_session(id, caller, |s| {
                for (k, v) in &vars {
                    s.set_env(k, v);
                }
            })
            .map_err(map_session_err)?;

        Ok(MessageResponse {
            message: format!("Set {} environment variable(s)", vars.len()),
        })
    }

    pub fn handle_get_env(
        &self,
        id: &str,
        caller: Option<&str>,
    ) -> std::result::Result<EnvVarsResponse, ApiError> {
        let env = self
            .session_manager
            .get_session(id, caller, |s| {
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
        log: &ReqLog,
    ) -> Result<()> {
        let (status, body) = match result {
            Ok(data) => (200, serde_json::to_string(&data).unwrap_or_default()),
            Err(e) => {
                let code = e.status_code();
                let body = serde_json::to_string(&e.to_error_response()).unwrap_or_default();
                (code, body)
            }
        };
        self.send(request, status, body, "application/json", log)
    }

    fn respond_empty(&self, request: Request, status: u16, log: &ReqLog) -> Result<()> {
        self.send(request, status, String::new(), "application/json", log)
    }

    /// Send a response with the given status/body/content-type, attaching CORS
    /// and the `X-Request-Id` header, and emit the structured access-log line.
    /// Every response in the server funnels through here so logging and the id
    /// header are uniform across all routes and early returns.
    fn send(
        &self,
        request: Request,
        status: u16,
        body: String,
        content_type: &str,
        log: &ReqLog,
    ) -> Result<()> {
        log.emit(status);
        let mut response = Response::from_string(body).with_status_code(StatusCode(status));
        for h in self.cors_headers() {
            response = response.with_header(h);
        }
        response = response.with_header(
            Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap(),
        );
        response = response
            .with_header(Header::from_bytes(&b"X-Request-Id"[..], log.id.as_bytes()).unwrap());
        request
            .respond(response)
            .map_err(|e| WasmrunError::from(format!("Response error: {e}")))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Per-request context for the always-on structured access log and the
/// `X-Request-Id` response header. One is built at the top of every request
/// and carried through to whichever response path runs.
struct ReqLog {
    id: String,
    method: String,
    path: String,
    /// Authenticated tenant id, or `"-"` in open mode / before auth resolves.
    tenant: String,
    start: Instant,
}

impl ReqLog {
    /// Emit the one-line `key=value` access record to stderr (always on).
    /// Greppable and dependency-free; `--verbose` adds the request-received
    /// line separately at the top of `handle_request`.
    fn emit(&self, status: u16) {
        let dur_ms = self.start.elapsed().as_millis();
        let ts = chrono::Utc::now().to_rfc3339();
        eprintln!(
            "ts={ts} id={id} method={method} path={path} status={status} dur_ms={dur_ms} tenant={tenant}",
            id = self.id,
            method = self.method,
            path = self.path,
            tenant = self.tenant,
        );
    }
}

/// Generate a short random hex request id (16 chars) for access logs and the
/// `X-Request-Id` header. Mirrors the session-id generator's xorshift mixing;
/// not cryptographically secure — only needs to be unique enough to correlate
/// a log line with a response.
fn generate_request_id() -> String {
    use std::sync::atomic::AtomicU64;
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    let mut state = nanos ^ (count.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut s = String::with_capacity(16);
    for _ in 0..8 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        s.push_str(&format!("{:02x}", state & 0xFF));
    }
    s
}

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

/// Format a count cap for the banner, where `0` means unlimited.
fn fmt_count(n: usize, unit: &str) -> String {
    if n == 0 {
        "unlimited".to_string()
    } else {
        format!("{n} {unit}")
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
        SessionError::TenantMaxSessionsReached { max } => {
            ApiError::RateLimited(format!("tenant session limit reached ({max})"))
        }
        SessionError::IoError { message } => ApiError::Internal(message),
        SessionError::LockError => ApiError::Internal("Lock error".into()),
    }
}

/// Last-modified time of `path`, or `None` if it can't be read.
fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Reload the auth config from `path` and atomically swap it into `cell`.
///
/// On success returns the new tenant count and the live config is replaced. On a
/// parse/validation error the previous config is **kept** and the error string
/// is returned — a bad edit must never crash the server or silently open it.
/// Factored out of the watcher thread so it can be unit-tested directly.
fn reload_auth(
    path: &Path,
    cell: &Arc<RwLock<Arc<AuthConfig>>>,
) -> std::result::Result<usize, String> {
    match AuthConfig::load(path) {
        Ok(new_cfg) => {
            let n = new_cfg.tenant_count();
            *cell.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(new_cfg);
            Ok(n)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Spawn a background thread that polls `path`'s mtime every `interval` and
/// hot-swaps the live auth config in `cell` when it changes. Logs each reload
/// outcome; exits promptly once `shutdown` is set.
fn spawn_auth_watcher(
    path: PathBuf,
    cell: Arc<RwLock<Arc<AuthConfig>>>,
    interval: Duration,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut last_mtime = file_mtime(&path);
        let slice = Duration::from_millis(500);
        loop {
            // Sleep up to `interval` in slices so shutdown stays responsive.
            let mut waited = Duration::ZERO;
            while waited < interval {
                if shutdown.load(Ordering::Relaxed) {
                    return;
                }
                let nap = slice.min(interval - waited);
                std::thread::sleep(nap);
                waited += nap;
            }
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            let cur = file_mtime(&path);
            if cur == last_mtime {
                continue;
            }
            last_mtime = cur;
            match reload_auth(&path, &cell) {
                Ok(n) => eprintln!("auth: reloaded {} ({n} tenants)", path.display()),
                Err(e) => eprintln!(
                    "auth: reload of {} failed, keeping previous config: {e}",
                    path.display()
                ),
            }
        }
    })
}

/// Extract the bearer token from a request's `Authorization` header.
///
/// Returns `Some(token)` only for a well-formed `Authorization: Bearer <token>`
/// with a non-empty token. The header name and the `Bearer` scheme are matched
/// case-insensitively (per RFC 7235); the token itself is taken verbatim.
fn bearer_token(request: &Request) -> Option<&str> {
    let header = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))?;
    let (scheme, token) = header.value.as_str().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
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

/// Read the full request body as a UTF-8 string.
///
/// When `max_bytes` is set, reads at most `max_bytes + 1` bytes so an oversize
/// body is detected (and rejected with 413) without buffering beyond the cap —
/// the `Content-Length` header is never trusted. `None` reads the body in full.
fn read_body(
    reader: &mut dyn Read,
    max_bytes: Option<usize>,
) -> std::result::Result<String, ApiError> {
    let Some(limit) = max_bytes else {
        let mut body = String::new();
        reader
            .read_to_string(&mut body)
            .map_err(|e| ApiError::BadRequest(format!("Failed to read request body: {e}")))?;
        return Ok(body);
    };

    let mut buf = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| ApiError::BadRequest(format!("Failed to read request body: {e}")))?;
    if buf.len() > limit {
        return Err(ApiError::PayloadTooLarge(limit));
    }
    String::from_utf8(buf)
        .map_err(|e| ApiError::BadRequest(format!("Request body is not valid UTF-8: {e}")))
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
        test_server_with_concurrency(100)
    }

    fn test_server_with_concurrency(max_concurrent_exec: usize) -> AgentServer {
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
            max_body_bytes: Some(32 * 1024 * 1024),
            max_concurrent_exec,
            auth: None,
            auth_path: None,
        })
    }

    /// Build an `AuthConfig` for the given `(key, tenant_id)` pairs by round-
    /// tripping through a TOML file (exercises the real `load` path).
    fn auth_config_for(tenants: &[(&str, &str)]) -> AuthConfig {
        use crate::agent::auth::hash_key;
        let toml = tenants
            .iter()
            .map(|(key, id)| {
                format!(
                    "[[tenants]]\nid = \"{id}\"\nkey_sha256 = \"{}\"\n",
                    hash_key(key)
                )
            })
            .collect::<String>();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, toml.as_bytes()).unwrap();
        AuthConfig::load(f.path()).unwrap()
    }

    /// An open-mode (no-auth) server bound to a specific `port`, for HTTP tests.
    fn open_server(port: u16) -> AgentServer {
        AgentServer::new(AgentConfig {
            port,
            session_config: SessionConfig {
                default_timeout: Duration::from_secs(60),
                max_sessions: 10,
                cleanup_interval: Duration::from_secs(300),
                limits: crate::agent::limits::ResourceLimits::default(),
            },
            allow_cors: true,
            verbose: false,
            max_body_bytes: Some(32 * 1024 * 1024),
            max_concurrent_exec: 100,
            auth: None,
            auth_path: None,
        })
    }

    /// A server on `port` with auth enabled for the given `(key, tenant_id)` pairs.
    fn auth_server(port: u16, tenants: &[(&str, &str)]) -> AgentServer {
        AgentServer::new(AgentConfig {
            port,
            session_config: SessionConfig {
                default_timeout: Duration::from_secs(60),
                max_sessions: 10,
                cleanup_interval: Duration::from_secs(300),
                limits: crate::agent::limits::ResourceLimits::default(),
            },
            allow_cors: true,
            verbose: false,
            max_body_bytes: Some(32 * 1024 * 1024),
            max_concurrent_exec: 100,
            auth: Some(Arc::new(auth_config_for(tenants))),
            auth_path: None,
        })
    }

    /// A server whose auth config is loaded from a full TOML body (so tests can
    /// include `[tenants.rate]` sub-tables). Generous server-wide caps so the
    /// per-tenant ceilings are what's actually exercised.
    fn auth_server_from_toml(toml: &str) -> AgentServer {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, toml.as_bytes()).unwrap();
        let auth = AuthConfig::load(f.path()).unwrap();
        AgentServer::new(AgentConfig {
            port: 0,
            session_config: SessionConfig {
                default_timeout: Duration::from_secs(60),
                max_sessions: 100,
                cleanup_interval: Duration::from_secs(300),
                limits: crate::agent::limits::ResourceLimits::default(),
            },
            allow_cors: true,
            verbose: false,
            max_body_bytes: Some(32 * 1024 * 1024),
            max_concurrent_exec: 100,
            auth: Some(Arc::new(auth)),
            auth_path: None,
        })
    }

    #[test]
    fn test_tenant_session_cap_enforced() {
        use crate::agent::auth::hash_key;
        let toml = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n[tenants.rate]\nmax_sessions = 2\n\n[[tenants]]\nid = \"bob\"\nkey_sha256 = \"{}\"\n",
            hash_key("k_alice"),
            hash_key("k_bob"),
        );
        let server = auth_server_from_toml(&toml);

        // alice capped at 2; the 3rd create is a 429-class RateLimited error.
        assert!(server
            .handle_create_session_with_body("", Some("alice"))
            .is_ok());
        assert!(server
            .handle_create_session_with_body("", Some("alice"))
            .is_ok());
        let err = server
            .handle_create_session_with_body("", Some("alice"))
            .unwrap_err();
        assert!(matches!(err, ApiError::RateLimited(_)));
        assert_eq!(err.status_code(), 429);

        // bob has no per-tenant cap — unaffected by alice's ceiling.
        for _ in 0..5 {
            assert!(server
                .handle_create_session_with_body("", Some("bob"))
                .is_ok());
        }
    }

    #[test]
    fn test_tenant_concurrent_exec_permit_saturates() {
        use crate::agent::auth::hash_key;
        let toml = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n[tenants.rate]\nmax_concurrent_exec = 1\n\n[[tenants]]\nid = \"bob\"\nkey_sha256 = \"{}\"\n",
            hash_key("a"),
            hash_key("b"),
        );
        let server = auth_server_from_toml(&toml);

        // alice's single slot: first acquire succeeds, second saturates.
        let p1 = server.try_tenant_exec_permit(Some("alice"));
        assert!(p1.is_some());
        assert!(server.try_tenant_exec_permit(Some("alice")).is_none());

        // bob (no cap) always gets a no-op permit; open mode (None) too.
        assert!(server.try_tenant_exec_permit(Some("bob")).is_some());
        assert!(server.try_tenant_exec_permit(None).is_some());

        // Releasing alice's permit frees her slot.
        drop(p1);
        assert!(server.try_tenant_exec_permit(Some("alice")).is_some());
    }

    #[test]
    fn test_tenant_requests_per_min_window() {
        use crate::agent::auth::hash_key;
        let toml = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n[tenants.rate]\nmax_requests_per_min = 2\n",
            hash_key("a"),
        );
        let server = auth_server_from_toml(&toml);

        assert!(server.allow_request_rate(Some("alice")));
        assert!(server.allow_request_rate(Some("alice")));
        assert!(!server.allow_request_rate(Some("alice"))); // 3rd within the window
                                                            // Open-mode caller is never throttled.
        assert!(server.allow_request_rate(None));
    }

    #[test]
    fn test_rate_window_basic_and_reset() {
        let w = RateWindow::new(1);
        assert!(w.allow());
        assert!(!w.allow());
        // Rewind the window start so the next call sees a fresh minute.
        {
            let mut g = w.state.lock().unwrap();
            g.0 = Instant::now() - Duration::from_secs(61);
        }
        assert!(w.allow());
    }

    #[test]
    fn test_rate_window_unlimited() {
        let w = RateWindow::new(0);
        for _ in 0..1000 {
            assert!(w.allow());
        }
    }

    #[test]
    fn test_tenant_limit_is_hard_ceiling() {
        use crate::agent::auth::hash_key;
        let toml = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n[tenants.limits]\nmax_memory_mb = 128\n",
            hash_key("a"),
        );
        let server = auth_server_from_toml(&toml);
        let cap_pages = 128 * 16; // MB → 64 KiB pages

        // No per-session override → the tenant baseline applies.
        let l = server.resolve_session_limits("", Some("alice")).unwrap();
        assert_eq!(l.max_memory_pages, Some(cap_pages));

        // A per-session override below the ceiling is honored.
        let l = server
            .resolve_session_limits(r#"{"limits":{"max_memory_mb":64}}"#, Some("alice"))
            .unwrap();
        assert_eq!(l.max_memory_pages, Some(64 * 16));

        // A per-session override above the ceiling is clamped down to it.
        let l = server
            .resolve_session_limits(r#"{"limits":{"max_memory_mb":512}}"#, Some("alice"))
            .unwrap();
        assert_eq!(l.max_memory_pages, Some(cap_pages));

        // A per-session "unlimited" (0) is clamped to the tenant's finite ceiling.
        let l = server
            .resolve_session_limits(r#"{"limits":{"max_memory_mb":0}}"#, Some("alice"))
            .unwrap();
        assert_eq!(l.max_memory_pages, Some(cap_pages));
    }

    #[test]
    fn test_open_mode_limits_unchanged() {
        let server = open_server(0);
        let defaults = crate::agent::limits::ResourceLimits::default();

        // No body → plain server defaults.
        assert_eq!(server.resolve_session_limits("", None).unwrap(), defaults);

        // With no tenant baseline, a per-session override applies un-clamped and
        // may exceed the server default (existing 0.20.1 behavior, back-compat).
        let l = server
            .resolve_session_limits(r#"{"limits":{"max_memory_mb":1024}}"#, None)
            .unwrap();
        assert_eq!(l.max_memory_pages, Some(1024 * 16));
    }

    #[test]
    fn test_reload_auth_swaps_live_config() {
        use crate::agent::auth::hash_key;
        // Initial config: only "alice".
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let v1 = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n",
            hash_key("ka")
        );
        std::io::Write::write_all(&mut f, v1.as_bytes()).unwrap();
        let cell = Arc::new(RwLock::new(Arc::new(AuthConfig::load(f.path()).unwrap())));

        assert_eq!(cell.read().unwrap().resolve("ka"), Some("alice"));
        assert_eq!(cell.read().unwrap().resolve("kb"), None);

        // Rewrite: revoke alice's key, add bob with a rate cap.
        let v2 = format!(
            "[[tenants]]\nid = \"bob\"\nkey_sha256 = \"{}\"\n[tenants.rate]\nmax_sessions = 7\n",
            hash_key("kb"),
        );
        std::fs::write(f.path(), v2).unwrap();

        assert_eq!(reload_auth(f.path(), &cell).unwrap(), 1);

        // The new config is live: alice's key is gone, bob resolves with its rate.
        let live = cell.read().unwrap().clone();
        assert_eq!(live.resolve("ka"), None);
        assert_eq!(live.resolve("kb"), Some("bob"));
        assert_eq!(live.rate("bob").unwrap().max_sessions, 7);
    }

    #[test]
    fn test_reload_auth_keeps_prior_config_on_error() {
        use crate::agent::auth::hash_key;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let v1 = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n",
            hash_key("ka")
        );
        std::io::Write::write_all(&mut f, v1.as_bytes()).unwrap();
        let cell = Arc::new(RwLock::new(Arc::new(AuthConfig::load(f.path()).unwrap())));

        // A malformed edit must not swap in a broken config.
        std::fs::write(f.path(), "this is not valid toml = = =").unwrap();
        assert!(reload_auth(f.path(), &cell).is_err());

        // The previous config is retained — alice still resolves.
        assert_eq!(cell.read().unwrap().resolve("ka"), Some("alice"));
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
        let resp = server.handle_get_session(&id, None).unwrap();
        assert_eq!(resp.session_id, id);
        assert_eq!(resp.state, "active");
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_delete_session() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        server.handle_delete_session(&id, None).unwrap();
        assert!(server.handle_get_session(&id, None).is_err());
    }

    #[test]
    fn test_session_not_found() {
        let server = test_server();
        let err = server.handle_get_session("nonexistent", None).unwrap_err();
        assert_eq!(err.status_code(), 404);
    }

    // ── File CRUD ─────────────────────────────────────────────────

    #[test]
    fn test_write_and_read_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(
                &id,
                r#"{"path": "test.txt", "content": "hello agent"}"#,
                None,
            )
            .unwrap();

        let resp = server.handle_read_file(&id, "test.txt", None).unwrap();
        assert_eq!(resp.content, "hello agent");

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_write_nested_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(
                &id,
                r#"{"path": "sub/dir/file.txt", "content": "nested"}"#,
                None,
            )
            .unwrap();

        let resp = server
            .handle_read_file(&id, "sub/dir/file.txt", None)
            .unwrap();
        assert_eq!(resp.content, "nested");

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_list_files() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        server
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "a"}"#, None)
            .unwrap();
        server
            .handle_write_file(&id, r#"{"path": "b.txt", "content": "bb"}"#, None)
            .unwrap();

        let resp = server.handle_list_files(&id, "/", None).unwrap();
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
            .handle_write_file(&id, r#"{"path": "del.txt", "content": "x"}"#, None)
            .unwrap();

        server.handle_delete_file(&id, "del.txt", None).unwrap();
        assert!(server.handle_read_file(&id, "del.txt", None).is_err());

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_read_nonexistent_file() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let err = server.handle_read_file(&id, "nope.txt", None).unwrap_err();
        assert_eq!(err.status_code(), 404);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_path_traversal_rejected() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let err = server
            .handle_read_file(&id, "../../../etc/passwd", None)
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
            .handle_set_env(&id, r#"{"FOO": "bar", "BAZ": "qux"}"#, None)
            .unwrap();

        let resp = server.handle_get_env(&id, None).unwrap();
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
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
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
            .handle_exec(&id, r#"{"wasm_path": "nope.wasm"}"#, None)
            .unwrap_err();
        assert_eq!(err.status_code(), 404);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_missing_wasm_path_and_source() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let err = server.handle_exec(&id, r#"{}"#, None).unwrap_err();
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
            .handle_exec(
                &id,
                r#"{"source": "print('hello')", "language": "python"}"#,
                None,
            )
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
        let err = server.handle_exec(&id, body, None).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("entry"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_files_with_unknown_entry_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{"files": {"main.js": "x"}, "entry": "missing.js"}"#;
        let err = server.handle_exec(&id, body, None).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("missing.js"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_files_with_unsupported_language_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{"files": {"a.py": "print(1)"}, "entry": "a.py", "language": "python"}"#;
        let err = server.handle_exec(&id, body, None).unwrap_err();
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
        let resp = server.handle_exec(&id, body, None).unwrap();
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
        let extra = server.handle_read_file(&id, "extra.js", None).unwrap();
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
        let result = server.handle_exec(&id, body, None);
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
        let result = server.handle_exec(&id, r#"{"source": "1+1"}"#, None);
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
            .handle_exec(&id, r#"{"command": "echo hello"}"#, None)
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
                None,
            )
            .unwrap();
        assert_eq!(resp.exit_code, 0);
        assert_eq!(resp.stdout, "persisted\n");

        // Verify the file is actually in the session work_dir
        let content = server.handle_read_file(&id, "log.txt", None).unwrap();
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
                None,
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
            .handle_exec(&id, r#"{"command": "export GREETING=hi"}"#, None)
            .unwrap();

        let env = server.handle_get_env(&id, None).unwrap();
        assert_eq!(env.env.get("GREETING").map(|s| s.as_str()), Some("hi"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_command_parse_error_returns_400() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Unclosed quote → parse error → BadRequest
        let resp = server
            .handle_exec(&id, r#"{"command": "echo \"oops"}"#, None)
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
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

        // First exec
        let resp1 = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
            .unwrap();
        assert_eq!(resp1.stdout, "Hello, World!\n");

        // Second exec should not accumulate
        let resp2 = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
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
        server
            .handle_set_env(&id, r#"{"APP": "test"}"#, None)
            .unwrap();

        // 3. Write WASM file
        let wasm = hello_wasm();
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

        // 4. Write a data file
        server
            .handle_write_file(&id, r#"{"path": "data.txt", "content": "test data"}"#, None)
            .unwrap();

        // 5. List files
        let files = server.handle_list_files(&id, "/", None).unwrap();
        assert!(files.entries.len() >= 2);

        // 6. Execute WASM
        let exec = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
            .unwrap();
        assert_eq!(exec.stdout, "Hello, World!\n");
        assert_eq!(exec.exit_code, 0);

        // 7. Read file back
        let content = server.handle_read_file(&id, "data.txt", None).unwrap();
        assert_eq!(content.content, "test data");

        // 8. Check env
        let env = server.handle_get_env(&id, None).unwrap();
        assert_eq!(env.env.get("APP").unwrap(), "test");

        // 9. Destroy
        server.handle_delete_session(&id, None).unwrap();
        assert!(server.handle_get_session(&id, None).is_err());
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
                    srv.handle_write_file(&id, &body, None).unwrap();

                    // Write and exec WASM
                    let work_dir = srv
                        .session_manager
                        .get_session(&id, None, |s| s.work_dir().to_path_buf())
                        .unwrap();
                    std::fs::write(work_dir.join("hello.wasm"), &wasm).unwrap();

                    let exec = srv
                        .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
                        .unwrap();
                    assert_eq!(exec.stdout, "Hello, World!\n");

                    // Verify isolation
                    let content = srv.handle_read_file(&id, "id.txt", None).unwrap();
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
            .create_session_with_limits(Duration::from_secs(60), limits, None, None)
            .unwrap()
    }

    #[test]
    fn test_create_session_with_limits_override() {
        let server = test_server();
        let body = r#"{"limits":{"max_fuel":500,"max_output_mb":0,"max_file_size_mb":1}}"#;
        let id = server
            .handle_create_session_with_body(body, None)
            .unwrap()
            .session_id;

        let limits = server
            .session_manager
            .get_session(&id, None, |s| s.limits().clone())
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
            .handle_create_session_with_body("", None)
            .unwrap()
            .session_id;
        let limits = server
            .session_manager
            .get_session(&id, None, |s| s.limits().clone())
            .unwrap();
        assert_eq!(limits, server.config.session_config.limits);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_create_session_invalid_limits_body_returns_400() {
        let server = test_server();
        let err = server
            .handle_create_session_with_body(r#"{"limits": "not-an-object"}"#, None)
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
                None,
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
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "12345"}"#, None)
            .unwrap();
        // Second 6-byte file pushes total to 11 > 10 → rejected.
        let err = server
            .handle_write_file(&id, r#"{"path": "b.txt", "content": "678901"}"#, None)
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
            .handle_exec(&id, r#"{"command": "echo hello"}"#, None)
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
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("loop.wasm"), infinite_loop_wasm()).unwrap();

        // With a fuel cap the runaway loop aborts well before the exec timeout.
        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "loop.wasm", "timeout": 30}"#, None)
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
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("loop.wasm"), infinite_loop_wasm()).unwrap();
        std::fs::write(work_dir.join("hello.wasm"), hello_wasm()).unwrap();

        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "loop.wasm", "timeout": 1}"#, None)
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
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm", "timeout": 10}"#, None)
            .unwrap();
        assert_eq!(ok.stdout, "Hello, World!\n");
        assert_eq!(ok.exit_code, 0);

        server.session_manager.destroy_all().unwrap();
    }

    // ── Request body size limit (0.20.3) ──────────────────────────

    #[test]
    fn test_read_body_within_limit() {
        let mut cur = std::io::Cursor::new(&b"hello"[..]);
        assert_eq!(read_body(&mut cur, Some(5)).unwrap(), "hello");
    }

    #[test]
    fn test_read_body_unlimited() {
        let data = vec![b'x'; 1024];
        let mut cur = std::io::Cursor::new(&data[..]);
        assert_eq!(read_body(&mut cur, None).unwrap().len(), 1024);
    }

    #[test]
    fn test_read_body_rejects_oversize_with_413() {
        let mut cur = std::io::Cursor::new(&b"hello world"[..]);
        let err = read_body(&mut cur, Some(5)).unwrap_err();
        assert_eq!(err.status_code(), 413);
        assert!(matches!(err, ApiError::PayloadTooLarge(5)));
    }

    #[test]
    fn test_read_body_at_exact_limit_is_ok() {
        // Exactly `limit` bytes must be accepted; only `> limit` is rejected.
        let mut cur = std::io::Cursor::new(&b"12345"[..]);
        assert_eq!(read_body(&mut cur, Some(5)).unwrap(), "12345");
    }

    // ── Exec concurrency cap (0.20.3) ─────────────────────────────

    #[test]
    fn test_exec_slots_saturation_and_release() {
        let slots = ExecSlots::new(2);
        let p1 = slots.try_acquire().unwrap();
        let p2 = slots.try_acquire().unwrap();
        // Saturated: third acquire fails.
        assert!(slots.try_acquire().is_none());
        // Releasing one frees a slot.
        drop(p1);
        let p3 = slots.try_acquire().unwrap();
        drop(p2);
        drop(p3);
        // After all release, capacity is restored.
        assert!(slots.try_acquire().is_some());
    }

    #[test]
    fn test_exec_slots_unlimited() {
        let slots = ExecSlots::new(0);
        let permits: Vec<_> = (0..1000).map(|_| slots.try_acquire().unwrap()).collect();
        assert_eq!(permits.len(), 1000);
    }

    #[test]
    fn test_exec_returns_429_when_saturated() {
        let server = test_server_with_concurrency(1);
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), hello_wasm()).unwrap();

        // Hold the only slot to simulate a worker already in flight.
        let held = server.exec_slots.try_acquire().unwrap();
        let err = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
            .unwrap_err();
        assert_eq!(err.status_code(), 429);
        assert!(matches!(err, ApiError::TooManyRequests(1)));

        // Releasing the slot lets the next exec through.
        drop(held);
        let ok = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
            .unwrap();
        assert_eq!(ok.exit_code, 0);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_permit_released_after_worker_completion() {
        // With a single slot, several *sequential* execs must all succeed —
        // proving each worker's permit is released when it completes (not
        // leaked), or the second call would 429.
        let server = test_server_with_concurrency(1);
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), hello_wasm()).unwrap();

        for _ in 0..3 {
            let ok = server
                .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
                .unwrap();
            assert_eq!(ok.exit_code, 0);
        }
        // The slot is free again after the loop.
        assert!(server.exec_slots.try_acquire().is_some());

        server.session_manager.destroy_all().unwrap();
    }

    // ── Authentication & tenant isolation ─────────────────────────

    #[test]
    fn test_disabled_auth_stamps_no_owner() {
        // Open mode (no auth): sessions have no owner; the keyless path is
        // unchanged. (The rest of the suite exercises this implicitly.)
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let owner = server
            .session_manager
            .get_session(&id, None, |s| s.owner().map(str::to_string))
            .unwrap();
        assert_eq!(owner, None);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_handler_tenant_isolation() {
        let server = test_server();

        // Tenant "alice" creates and populates a session.
        let id = server
            .handle_create_session_with_body("", Some("alice"))
            .unwrap()
            .session_id;
        let owner = server
            .session_manager
            .get_session(&id, Some("alice"), |s| s.owner().map(str::to_string))
            .unwrap();
        assert_eq!(owner, Some("alice".to_string()));

        server
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "hi"}"#, Some("alice"))
            .unwrap();
        assert!(server.handle_get_session(&id, Some("alice")).is_ok());
        assert!(server.handle_read_file(&id, "a.txt", Some("alice")).is_ok());

        // Tenant "bob" sees 404 on every operation against alice's session.
        let bob = Some("bob");
        assert_eq!(
            server
                .handle_get_session(&id, bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server
                .handle_exec(&id, r#"{"command": "echo hi"}"#, bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server
                .handle_read_file(&id, "a.txt", bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server
                .handle_write_file(&id, r#"{"path": "x", "content": "y"}"#, bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server
                .handle_list_files(&id, "/", bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server
                .handle_delete_file(&id, "a.txt", bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server
                .handle_set_env(&id, r#"{"K": "V"}"#, bob)
                .unwrap_err()
                .status_code(),
            404
        );
        assert_eq!(
            server.handle_get_env(&id, bob).unwrap_err().status_code(),
            404
        );
        assert_eq!(
            server
                .handle_delete_session(&id, bob)
                .unwrap_err()
                .status_code(),
            404
        );

        // Open-mode caller (None) is also blocked from an owned session.
        assert_eq!(
            server
                .handle_get_session(&id, None)
                .unwrap_err()
                .status_code(),
            404
        );

        // bob's failed delete did not destroy the session; alice still owns it.
        assert!(server.handle_delete_session(&id, Some("alice")).is_ok());

        server.session_manager.destroy_all().unwrap();
    }

    /// Status code of a ureq result, treating 4xx/5xx (returned as `Err`) and
    /// 2xx alike so assertions can compare the numeric code.
    fn http_status(r: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> u16 {
        match r {
            Ok(resp) => resp.status().as_u16(),
            Err(ureq::Error::StatusCode(code)) => code,
            Err(e) => panic!("transport error: {e}"),
        }
    }

    #[test]
    fn test_auth_gate_and_isolation_over_http() {
        use std::io::Read;

        // Grab a free port, then release it for the server to bind.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let server = auth_server(port, &[("key_a", "alice"), ("key_b", "bob")]);
        std::thread::spawn(move || {
            let _ = server.start();
        });

        let base = format!("http://127.0.0.1:{port}/api/v1");

        // Wait until the listener is accepting (any HTTP response, incl. 401).
        for _ in 0..100 {
            match ureq::get(format!("{base}/tools")).call() {
                Err(ureq::Error::Io(_)) => std::thread::sleep(Duration::from_millis(20)),
                _ => break,
            }
        }

        let sessions = format!("{base}/sessions");

        // Missing header → 401.
        assert_eq!(http_status(ureq::post(&sessions).send_empty()), 401);
        // Wrong scheme → 401.
        assert_eq!(
            http_status(
                ureq::post(&sessions)
                    .header("Authorization", "Token key_a")
                    .send_empty()
            ),
            401
        );
        // Unknown key → 401.
        assert_eq!(
            http_status(
                ureq::post(&sessions)
                    .header("Authorization", "Bearer nope")
                    .send_empty()
            ),
            401
        );
        // /tools is gated too: no key → 401, valid key → 200.
        assert_eq!(http_status(ureq::get(format!("{base}/tools")).call()), 401);
        assert_eq!(
            http_status(
                ureq::get(format!("{base}/tools"))
                    .header("Authorization", "Bearer key_a")
                    .call()
            ),
            200
        );

        // Valid key → 200; capture the session id.
        let mut resp = ureq::post(&sessions)
            .header("Authorization", "Bearer key_a")
            .send_empty()
            .unwrap();
        let mut body = String::new();
        resp.body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let id = v["session_id"].as_str().unwrap().to_string();

        let one = format!("{base}/sessions/{id}");

        // Owner (alice) → 200; other tenant (bob) → 404 across read/exec/delete.
        assert_eq!(
            http_status(
                ureq::get(&one)
                    .header("Authorization", "Bearer key_a")
                    .call()
            ),
            200
        );
        assert_eq!(
            http_status(
                ureq::get(&one)
                    .header("Authorization", "Bearer key_b")
                    .call()
            ),
            404
        );
        assert_eq!(
            http_status(
                ureq::post(format!("{one}/exec"))
                    .header("Authorization", "Bearer key_b")
                    .send(r#"{"command": "echo hi"}"#)
            ),
            404
        );
        assert_eq!(
            http_status(
                ureq::delete(&one)
                    .header("Authorization", "Bearer key_b")
                    .call()
            ),
            404
        );
        // Owner can delete its own session.
        assert_eq!(
            http_status(
                ureq::delete(&one)
                    .header("Authorization", "Bearer key_a")
                    .call()
            ),
            200
        );
    }

    // ── Observability / metrics (0.20.5) ──────────────────────────

    #[test]
    fn test_metrics_exec_success_recorded() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), hello_wasm()).unwrap();

        server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
            .unwrap();

        let v = server.metrics_json();
        assert_eq!(v["exec_total"]["success"], 1);
        assert_eq!(v["exec_total"]["error"], 0);
        assert_eq!(v["exec_total"]["timeout"], 0);
        assert_eq!(v["exec_duration_ms_count"], 1);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_metrics_session_created_and_active_gauge() {
        let server = test_server();
        let a = server.handle_create_session().unwrap().session_id;
        let _b = server.handle_create_session().unwrap().session_id;

        let v = server.metrics_json();
        assert_eq!(v["sessions_created_total"], 2);
        assert_eq!(v["sessions_active"], 2);

        // Destroying one drops the active gauge but not the cumulative counter.
        server.handle_delete_session(&a, None).unwrap();
        let v = server.metrics_json();
        assert_eq!(v["sessions_created_total"], 2);
        assert_eq!(v["sessions_active"], 1);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_metrics_concurrency_rejection_recorded() {
        let server = test_server_with_concurrency(1);
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("hello.wasm"), hello_wasm()).unwrap();

        // Hold the only slot so the exec is rejected with 429.
        let held = server.exec_slots.try_acquire().unwrap();
        let err = server
            .handle_exec(&id, r#"{"wasm_path": "hello.wasm"}"#, None)
            .unwrap_err();
        assert_eq!(err.status_code(), 429);
        drop(held);

        let v = server.metrics_json();
        assert_eq!(v["exec_rejected_total"]["concurrency"], 1);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_metrics_json_includes_per_session_in_open_mode() {
        let server = test_server(); // open mode (auth = None)
        let id = server.handle_create_session().unwrap().session_id;
        server
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "hello"}"#, None)
            .unwrap();

        let v = server.metrics_json();
        let sessions = v["sessions"].as_array().expect("per-session rows present");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], id);
        assert!(sessions[0]["disk_bytes"].as_u64().unwrap() >= 5); // "hello"
                                                                   // Aggregate disk gauge reflects the written file too.
        assert!(v["sessions_disk_bytes"].as_u64().unwrap() >= 5);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_metrics_json_omits_per_session_in_auth_mode() {
        // Per-session rows are withheld in auth mode (Q2: aggregates only).
        let server = auth_server(0, &[("key_a", "alice")]);
        let id = server
            .handle_create_session_with_body("", Some("alice"))
            .unwrap()
            .session_id;
        server
            .handle_write_file(&id, r#"{"path": "a.txt", "content": "hi"}"#, Some("alice"))
            .unwrap();

        let v = server.metrics_json();
        assert!(
            v.get("sessions").is_none(),
            "per-session rows must be hidden in auth mode"
        );
        // Global aggregates are still present.
        assert_eq!(v["sessions_active"], 1);
        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_metrics_prometheus_render_contains_families() {
        let server = test_server();
        let _ = server.handle_create_session().unwrap();
        let text = server.metrics_prometheus();
        assert!(text.contains("# TYPE wasmrun_agent_exec_total counter"));
        assert!(text.contains("wasmrun_agent_sessions_created_total 1"));
        assert!(text.contains("wasmrun_agent_sessions_active 1"));
        assert!(text.contains("# TYPE wasmrun_agent_sessions_active gauge"));
        server.session_manager.destroy_all().unwrap();
    }

    /// Spin up a real server on `port` and wait until it accepts connections.
    fn wait_until_ready(port: u16) {
        let probe = format!("http://127.0.0.1:{port}/api/v1/metrics");
        for _ in 0..100 {
            match ureq::get(&probe).call() {
                Err(ureq::Error::Io(_)) => std::thread::sleep(Duration::from_millis(20)),
                _ => break,
            }
        }
    }

    #[test]
    fn test_metrics_over_http_open_mode() {
        use std::io::Read;
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::thread::spawn(move || {
            let _ = open_server(port).start();
        });
        wait_until_ready(port);

        let metrics = format!("http://127.0.0.1:{port}/api/v1/metrics");

        // Default: Prometheus text exposition + X-Request-Id header.
        let resp = ureq::get(&metrics).call().unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let ctype = resp
            .headers()
            .get("Content-Type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(ctype.starts_with("text/plain"), "got content-type {ctype}");
        assert!(
            resp.headers().get("X-Request-Id").is_some(),
            "X-Request-Id header missing"
        );
        let mut resp = resp;
        let mut body = String::new();
        resp.body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .unwrap();
        assert!(body.contains("wasmrun_agent_exec_total"));
        assert!(body.contains("# HELP wasmrun_agent_sessions_active"));

        // JSON variant parses and carries the same families.
        let mut resp = ureq::get(format!("{metrics}?format=json")).call().unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let ctype = resp
            .headers()
            .get("Content-Type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(ctype.starts_with("application/json"), "got {ctype}");
        let mut body = String::new();
        resp.body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v["exec_total"].is_object());
        assert!(v["sessions_active"].is_u64());
    }

    #[test]
    fn test_metrics_auth_gated_and_counts_unauthorized() {
        use std::io::Read;
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::thread::spawn(move || {
            let _ = auth_server(port, &[("key_a", "alice")]).start();
        });

        let base = format!("http://127.0.0.1:{port}/api/v1");
        // Wait until accepting (a 401 counts as ready).
        for _ in 0..100 {
            match ureq::get(format!("{base}/metrics")).call() {
                Err(ureq::Error::Io(_)) => std::thread::sleep(Duration::from_millis(20)),
                _ => break,
            }
        }

        // No key → 401 (this also bumps the unauthorized rejection counter).
        assert_eq!(
            http_status(ureq::get(format!("{base}/metrics")).call()),
            401
        );

        // Valid key → 200 JSON; unauthorized counter recorded; no per-session rows.
        let mut resp = ureq::get(format!("{base}/metrics?format=json"))
            .header("Authorization", "Bearer key_a")
            .call()
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let mut body = String::new();
        resp.body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v["exec_rejected_total"]["unauthorized"].as_u64().unwrap() >= 1);
        assert!(
            v.get("sessions").is_none(),
            "auth mode must not expose per-session rows"
        );
    }
}
