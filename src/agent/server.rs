//! Agent mode: REST API server for AI agent sandbox management.

use crate::agent::api::*;
use crate::agent::auth::{AuthConfig, TenantRate};
use crate::agent::executor;
use crate::agent::limits::{dir_size, LimitsOverride, ResourceLimits};
use crate::agent::metrics::{Gauges, Metrics, SessionResourceRow};
use crate::agent::pool::{resolve_workers, PoolStats, WorkerPool, DEFAULT_SHUTDOWN_GRACE};
use crate::agent::session;
use crate::agent::session::{SessionConfig, SessionError, SessionManager, SessionState};
use crate::agent::shell;
use crate::agent::tools;
use crate::agent::vendor;
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

/// An exec that has been spawned and is running detached. The session's WASI
/// buffers accumulate output as it goes, which is what lets the streaming
/// endpoint report progress without the worker knowing about HTTP.
struct RunningExec {
    rx: std::sync::mpsc::Receiver<std::result::Result<i32, ApiError>>,
    cancel: Arc<AtomicBool>,
    wasi_env: Arc<Mutex<crate::runtime::wasi::WasiEnv>>,
    lock_slot: Arc<Mutex<Option<vendor::Lockfile>>>,
    start: Instant,
    timeout: Duration,
    timeout_secs: u64,
}

/// How often a streaming exec samples the session buffers. Short enough to
/// feel live, long enough that a tight loop is not one frame per line.
const STREAM_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long the accept loop waits for a connection before re-checking the
/// shutdown flag. Short enough that Ctrl+C is not held up by an idle server.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Shared by the single-session and list endpoints so they cannot disagree.
fn session_status_row(s: &crate::agent::session::Session) -> SessionStatusResponse {
    SessionStatusResponse {
        session_id: s.id().to_string(),
        state: match s.state() {
            SessionState::Active => "active".into(),
            SessionState::Expired => "expired".into(),
        },
        created_at_elapsed_ms: s.created_at().elapsed().as_millis() as u64,
        last_accessed_elapsed_ms: s.last_accessed().elapsed().as_millis() as u64,
        timeout_secs: s.timeout().as_secs(),
    }
}

/// Work out which npm dependencies an exec should install, and validate them.
///
/// Reading package.json is opt-in because an uploaded one would otherwise turn
/// an ordinary exec into a network fetch nobody asked for. The explicit
/// `dependencies` map wins on conflict, being the more specific instruction.
fn resolve_exec_deps(
    req: &ExecRequest,
    work_dir: &Path,
) -> std::result::Result<Option<HashMap<String, String>>, ApiError> {
    let mut deps = HashMap::new();

    if req.install_package_json.unwrap_or(false) {
        let raw = match req.files.as_ref().and_then(|f| f.get("package.json")) {
            Some(uploaded) => Some(uploaded.clone()),
            None => std::fs::read_to_string(work_dir.join("package.json")).ok(),
        };
        let raw = raw.ok_or_else(|| {
            ApiError::BadRequest(
                "'install_package_json' is set but no package.json was uploaded or found in the session".into(),
            )
        })?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid package.json: {e}")))?;
        // devDependencies: nothing in the sandbox runs them yet.
        if let Some(map) = parsed.get("dependencies").and_then(|d| d.as_object()) {
            for (name, range) in map {
                let range = range.as_str().ok_or_else(|| {
                    ApiError::BadRequest(format!(
                        "package.json dependency '{name}' must map to a version range string"
                    ))
                })?;
                deps.insert(name.clone(), range.to_string());
            }
        }
    }

    if let Some(explicit) = &req.dependencies {
        deps.extend(explicit.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    if deps.is_empty() {
        return Ok(None);
    }
    vendor::validate_deps(&deps)?;
    Ok(Some(deps))
}

/// Install an exec's npm dependencies, recording the tree in `lock_out`.
///
/// Runs on the exec worker because it is network-bound. A supplied lockfile is
/// replayed first and carried into the result, so pinning a tree and adding a
/// dependency yields a lockfile describing both.
fn vendor_for_exec(
    registry: &str,
    deps: Option<&HashMap<String, String>>,
    lock_in: Option<&vendor::Lockfile>,
    work_dir: &Path,
    limits: &ResourceLimits,
    lock_out: &Mutex<Option<vendor::Lockfile>>,
) -> std::result::Result<(), ApiError> {
    if deps.is_none() && lock_in.is_none() {
        return Ok(());
    }
    let v = vendor::Vendor::new(registry)?;
    let mut lock = vendor::Lockfile::new();
    if let Some(l) = lock_in {
        v.vendor_locked(l, work_dir, limits)?;
        lock.extend(l.iter().map(|(k, e)| (k.clone(), e.clone())));
    }
    if let Some(d) = deps {
        lock.extend(v.vendor(d, work_dir, limits)?);
    }
    if let Ok(mut slot) = lock_out.lock() {
        *slot = Some(lock);
    }
    Ok(())
}

const API_PREFIX: &str = "/api/v1";
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 30;
// Language runtimes (e.g. QuickJS compiled to WASM) generate deep call chains that
// overflow the default 8 MB thread stack when run through the WASM interpreter.
const EXEC_THREAD_STACK_BYTES: usize = 64 * 1024 * 1024;
/// Default request body cap (32 MB) when none is configured.
const DEFAULT_MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
/// Default ceiling on concurrent exec workers when none is configured.
const DEFAULT_MAX_CONCURRENT_EXEC: usize = 100;
/// Default bind address. Loopback, not `0.0.0.0`: the server runs arbitrary
/// WASM and JavaScript on request, so reaching it from another host is opt-in
/// (`--host`) and, without auth, needs an explicit `--insecure`.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// Missed heartbeats before another server's session tree is considered
/// orphaned. Generous: sweeping a live instance's sessions is far worse than
/// leaving a dead one's directories around for another few minutes.
const ORPHAN_GRACE_TICKS: u32 = 10;
/// Floor on that window, so a short `cleanup_interval` cannot shrink it to the
/// point where a briefly stalled server looks dead.
const MIN_ORPHAN_GRACE: Duration = Duration::from_secs(300);
/// Default npm cache ceiling, in MB.
pub const DEFAULT_MAX_CACHE_MB: u64 = 2048;
/// How often the cache is trimmed while the server runs. Long, because the
/// pass walks the whole cache and packages arrive in bursts, not steadily.
const CACHE_SWEEP_INTERVAL: Duration = Duration::from_secs(300);
/// How recently an entry must have been installed from to be spared. Covers
/// any in-progress copy out of the cache by orders of magnitude.
const CACHE_ENTRY_MIN_AGE: Duration = Duration::from_secs(600);

#[derive(Clone)]
pub struct AgentConfig {
    pub port: u16,
    /// Address to bind. Defaults to [`DEFAULT_HOST`] (loopback); `0.0.0.0`
    /// exposes the server on every interface, which [`validate_bind`] only
    /// allows with auth or `insecure`.
    pub host: String,
    /// Bind a non-loopback address even with auth disabled. The escape hatch
    /// for a genuinely trusted network; never a good default.
    pub insecure: bool,
    pub session_config: SessionConfig,
    pub allow_cors: bool,
    pub verbose: bool,
    /// Maximum accepted request body size in bytes. `None` = unlimited.
    pub max_body_bytes: Option<usize>,
    /// Maximum number of exec workers allowed to run concurrently across all
    /// sessions. `0` = unlimited. Bounds thread / stack / memory footprint
    /// independently of `max_sessions` (which only bounds session count).
    pub max_concurrent_exec: usize,
    /// Maximum number of HTTP request-handling threads. `0` = auto, sized from
    /// `max_concurrent_exec` (see [`resolve_workers`]). Threads are spawned on
    /// demand, so this is a ceiling and not a startup cost.
    pub workers: usize,
    /// How long shutdown waits for in-flight requests before abandoning them.
    pub shutdown_grace: Duration,
    /// Ceiling on the shared npm cache in bytes. `None` = unlimited, which
    /// still clears interrupted-install debris.
    pub max_cache_bytes: Option<u64>,
    /// API-key authentication. `None` = open mode (no auth; back-compat). When
    /// `Some`, every `/api/v1/*` request must present a valid `Bearer` key and
    /// sessions are isolated per tenant.
    pub auth: Option<Arc<AuthConfig>>,
    /// Path to the auth config file, retained so the server can watch it for
    /// live reloads. `None` when `--auth` was not given (open mode).
    pub auth_path: Option<PathBuf>,
    /// npm registry base URL used to vendor `dependencies` (private
    /// registries and tests point this elsewhere).
    pub npm_registry: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            port: 8430,
            host: DEFAULT_HOST.to_string(),
            insecure: false,
            session_config: SessionConfig::default(),
            allow_cors: false,
            verbose: false,
            max_body_bytes: Some(DEFAULT_MAX_BODY_BYTES),
            max_concurrent_exec: DEFAULT_MAX_CONCURRENT_EXEC,
            workers: 0,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            max_cache_bytes: Some(DEFAULT_MAX_CACHE_MB * 1024 * 1024),
            auth: None,
            auth_path: None,
            npm_registry: crate::agent::vendor::DEFAULT_NPM_REGISTRY.to_string(),
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

    /// Whether every slot is taken, so the next exec would be refused. Always
    /// `false` when unlimited.
    fn saturated(&self) -> bool {
        self.max != 0 && self.in_flight.load(Ordering::Acquire) >= self.max
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
    /// Live request-worker counters. Owned here so `/metrics` can read them;
    /// the pool itself only exists while `start()` is listening.
    pool_stats: Arc<PoolStats>,
    /// Live, swappable auth config (`None` in open mode). Read on every request
    /// via a brief read lock; replaced wholesale by the reload watcher when the
    /// auth file changes. The inner `Arc` makes each request's snapshot cheap.
    live_auth: Option<Arc<RwLock<Arc<AuthConfig>>>>,
    /// The auth file to watch for live reloads (`None` if `--auth` was not set).
    auth_path: Option<PathBuf>,
    /// When the process started serving, for `/health`'s uptime.
    started: Instant,
    /// Set by Ctrl+C. Owned here rather than by `start()` so `/ready` can fail
    /// the probe as soon as shutdown begins, which is what lets a load balancer
    /// take the instance out before the listener stops.
    shutdown: Arc<AtomicBool>,
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
            pool_stats: Arc::new(PoolStats::default()),
            live_auth,
            auth_path,
            started: Instant::now(),
            shutdown: Arc::new(AtomicBool::new(false)),
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
        validate_bind(
            &self.config.host,
            self.config.auth.is_some(),
            self.config.insecure,
        )?;

        // Shut down on Ctrl+C, and on SIGTERM/SIGHUP: a container or a systemd
        // unit is stopped with SIGTERM, and ignoring it means being SIGKILLed
        // at the end of the stop timeout with every session directory leaked.
        //
        // Installed before anything is claimed on disk, so a signal arriving
        // during startup still reaches the shutdown path rather than the
        // default disposition, which would kill the process mid-sweep.
        let shutdown = self.shutdown.clone();
        let shutdown_flag = shutdown.clone();
        let _ = ctrlc::set_handler(move || {
            shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        let addr = format!("{}:{}", self.config.host, self.config.port);
        let server = Server::http(&addr)
            .map_err(|e| WasmrunError::from(format!("Failed to start agent server: {e}")))?;

        let workers = resolve_workers(self.config.workers, self.config.max_concurrent_exec);
        print!("{}", self.banner(workers));

        // Claim this instance's session tree before sweeping, so a server
        // starting up alongside this one never mistakes it for an orphan.
        session::heartbeat();
        let swept = session::sweep_orphans(self.orphan_grace());
        if swept > 0 {
            println!("   Swept {swept} orphaned session tree(s) from a previous run\n");
        }

        // The npm and runtime caches are shared across sessions and outlive
        // every one of them, so nothing else would ever bound them.
        report_cache_sweep(sweep_caches(self.config.max_cache_bytes));
        let cache_handle = spawn_cache_sweeper(self.config.max_cache_bytes, shutdown.clone());

        let cleanup_handle = SessionManager::start_cleanup_thread(self.session_manager.clone());

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

        // Requests are handed to worker threads, so a long exec never stalls the
        // accept loop (and with it every other session, tenant and /metrics
        // scrape). Accepting with a timeout also makes Ctrl+C take effect
        // without waiting for one more request to arrive.
        let mut pool = WorkerPool::new(workers, self.pool_stats.clone());
        let this = Arc::new(self);
        while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            let request = match server.recv_timeout(ACCEPT_POLL_INTERVAL) {
                Ok(Some(request)) => request,
                Ok(None) => continue,
                Err(e) => {
                    // Back off rather than spinning, in case the listener is
                    // failing every call rather than just this one.
                    eprintln!("Accept error: {e}");
                    std::thread::sleep(ACCEPT_POLL_INTERVAL);
                    continue;
                }
            };
            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                let _ =
                    request.respond(Response::from_string("").with_status_code(StatusCode(503)));
                break;
            }
            let srv = this.clone();
            pool.dispatch(Box::new(move || {
                if let Err(e) = srv.handle_request(request) {
                    eprintln!("Request error: {e}");
                }
            }));
        }

        // Draining: the listener is closed, `/ready` has been reporting
        // `shutting_down` since the flag was set, and in-flight requests get
        // `--shutdown-timeout` to finish before the process gives up on them.
        let grace = this.config.shutdown_grace;
        eprintln!("\n🛑 Shutting down...");
        let in_flight = this.pool_stats.busy();
        if in_flight > 0 {
            eprintln!(
                "   Draining {in_flight} in-flight request(s), up to {}s",
                grace.as_secs()
            );
        }
        let abandoned = pool.shutdown(grace);
        if abandoned > 0 {
            eprintln!("   Gave up on {abandoned} request(s) still running at the deadline");
        }
        let destroyed = this.session_manager.destroy_all().unwrap_or(0);
        this.session_manager.stop_cleanup();
        let _ = cleanup_handle.join();
        let _ = cache_handle.join();
        if let Some(handle) = auth_watcher {
            let _ = handle.join();
        }
        if destroyed > 0 {
            eprintln!("   Cleaned up {destroyed} session(s)");
        }
        // Last, so the tree is gone only once its sessions are: a crash between
        // the two would leave the heartbeat behind for the next sweep.
        session::remove_instance_root();
        eprintln!("   Goodbye.");
        Ok(())
    }

    /// How stale another instance's heartbeat must be before its session tree
    /// is swept. A comfortable multiple of the cleanup interval that refreshes
    /// it, floored so a short interval cannot make the window jumpy.
    fn orphan_grace(&self) -> Duration {
        (self.config.session_config.cleanup_interval * ORPHAN_GRACE_TICKS).max(MIN_ORPHAN_GRACE)
    }

    /// The startup banner, built as a string so its security warnings can be
    /// asserted on in tests rather than only read by eye.
    fn banner(&self, workers: usize) -> String {
        use std::fmt::Write as _;

        let host = &self.config.host;
        let port = self.config.port;
        let max = self.config.session_config.max_sessions;
        let timeout = self.config.session_config.default_timeout.as_secs();
        let limits = &self.config.session_config.limits;
        let cors = if self.config.allow_cors {
            "open"
        } else {
            "restricted"
        };
        let exposed = !is_loopback_host(host);
        let reach = if exposed {
            "reachable from other hosts"
        } else {
            "loopback only"
        };

        let mut b = String::new();
        let _ = writeln!(b, "\n🤖 Wasmrun Agent Server");
        let _ = writeln!(b, "   Endpoint:        http://{host}:{port}{API_PREFIX}");
        let _ = writeln!(b, "   Bind:            {host} ({reach}), plaintext HTTP");
        let _ = writeln!(b, "   Max sessions:    {max}");
        let _ = writeln!(b, "   Session timeout: {timeout}s");
        let _ = writeln!(
            b,
            "   Memory limit:    {}",
            fmt_pages_mb(limits.max_memory_pages)
        );
        let _ = writeln!(
            b,
            "   Fuel limit:      {}",
            fmt_opt_u64(limits.max_fuel, "instructions")
        );
        let _ = writeln!(
            b,
            "   Output limit:    {}",
            fmt_bytes_mb(limits.max_output_bytes.map(|b| b as u64))
        );
        let _ = writeln!(
            b,
            "   File size limit: {}",
            fmt_bytes_mb(limits.max_file_size)
        );
        let _ = writeln!(
            b,
            "   Disk limit:      {}",
            fmt_bytes_mb(limits.max_disk_bytes)
        );
        let _ = writeln!(
            b,
            "   Max body size:   {}",
            fmt_bytes_mb(self.config.max_body_bytes.map(|b| b as u64))
        );
        let _ = writeln!(
            b,
            "   Max concurrent:  {}",
            fmt_count(self.config.max_concurrent_exec, "exec(s)")
        );
        let _ = writeln!(b, "   Request workers: {workers} max");
        let _ = writeln!(
            b,
            "   Shutdown drain:  {}s",
            self.config.shutdown_grace.as_secs()
        );
        let _ = writeln!(
            b,
            "   npm cache cap:   {}",
            match self.config.max_cache_bytes {
                Some(bytes) => format!("{} MB (shared, host-wide)", bytes / (1024 * 1024)),
                None => "unlimited".to_string(),
            }
        );
        match &self.config.auth {
            Some(auth) => {
                let _ = writeln!(
                    b,
                    "   Auth:            enabled ({} tenants)",
                    auth.tenant_count()
                );
                if let Some(path) = &self.auth_path {
                    let _ = writeln!(b, "   Auth reload:     watching {}", path.display());
                }
            }
            None => {
                let _ = writeln!(b, "   Auth:            disabled (open)");
            }
        }
        let _ = writeln!(b, "   CORS:            {cors}");

        // Deployment warnings. Loud on purpose: an exec API on a routable
        // address is a remote-code-execution endpoint for whoever can reach it,
        // and nothing else in the output says so.
        if exposed {
            let _ = writeln!(b);
            if self.config.auth.is_none() {
                let _ = writeln!(
                    b,
                    "   ⚠️  WARNING: AUTH IS DISABLED on a non-loopback bind (--insecure)."
                );
                let _ = writeln!(
                    b,
                    "       Anyone who can reach {host}:{port} can run arbitrary code here."
                );
            }
            let _ = writeln!(
                b,
                "   ⚠️  Traffic is plaintext, including API keys. Terminate TLS at a"
            );
            let _ = writeln!(
                b,
                "       reverse proxy and let it be the only thing reaching this port."
            );
        }
        let _ = writeln!(b);
        let _ = writeln!(b, "   Endpoints:");
        let _ = writeln!(b, "     POST   /sessions              create session");
        let _ = writeln!(b, "     GET    /sessions/:id           session status");
        let _ = writeln!(b, "     DELETE /sessions/:id           destroy session");
        let _ = writeln!(b, "     POST   /sessions/:id/exec      execute WASM");
        let _ = writeln!(b, "     POST   /sessions/:id/files     write file");
        let _ = writeln!(b, "     GET    /sessions/:id/files     read / list files");
        let _ = writeln!(b, "     DELETE /sessions/:id/files     delete file");
        let _ = writeln!(b, "     POST   /sessions/:id/env       set env vars");
        let _ = writeln!(b, "     GET    /sessions/:id/env       get env vars");
        let _ = writeln!(b, "     GET    /tools                  LLM tool schemas");
        let _ = writeln!(
            b,
            "     GET    /metrics                metrics (Prometheus | ?format=json)"
        );
        let _ = writeln!(
            b,
            "     GET    /health                 liveness (unauthenticated)"
        );
        let _ = writeln!(
            b,
            "     GET    /ready                  readiness (unauthenticated)"
        );
        let _ = writeln!(b);
        b
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

        let segments: Vec<&str> = path
            .trim_start_matches(API_PREFIX)
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        // Probe endpoints, answered before the auth gate: a liveness check that
        // needs a credential is not a liveness check, and `/metrics` is
        // auth-gated so it cannot stand in for one. Both are deliberately
        // uninteresting to an anonymous caller — status, version and uptime,
        // with the load figures left to `/metrics`.
        match (&method, segments.as_slice()) {
            (Method::Get, ["health"]) => {
                return self.respond_json(request, Ok::<_, ApiError>(self.health_json()), &log)
            }
            (Method::Get, ["ready"]) => {
                let (ready, reason) = self.readiness();
                let body = serde_json::json!({
                    "status": if ready { "ready" } else { "unready" },
                    "reason": reason,
                });
                let status = if ready { 200 } else { 503 };
                return self.send(
                    request,
                    status,
                    serde_json::to_string(&body).unwrap_or_default(),
                    "application/json",
                    &log,
                );
            }
            _ => {}
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
            (Method::Get, ["sessions"]) => {
                self.respond_json(request, self.handle_list_sessions(tenant), &log)
            }
            (Method::Get, ["sessions", id]) => {
                self.respond_json(request, self.handle_get_session(id, tenant), &log)
            }
            (Method::Delete, ["sessions", id]) => {
                self.respond_json(request, self.handle_delete_session(id, tenant), &log)
            }
            (Method::Post, ["sessions", id, "exec"]) => {
                // Read before the request is consumed; a malformed body still
                // reaches the buffered path, which reports it as a 400.
                let stream = serde_json::from_str::<ExecRequest>(&body)
                    .ok()
                    .and_then(|r| r.stream)
                    .unwrap_or(false);
                if stream {
                    self.handle_exec_stream(request, id, &body, tenant, &log);
                    Ok(())
                } else {
                    self.respond_json(request, self.handle_exec(id, &body, tenant), &log)
                }
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

    /// `/health`: the process is alive and serving. Never reports a problem —
    /// anything that makes this fail to answer (deadlock, OOM, a dead listener)
    /// is exactly what a liveness probe is meant to catch.
    fn health_json(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": self.started.elapsed().as_secs(),
        })
    }

    /// `/ready`: whether this instance can take new work. Unready is a signal
    /// to route elsewhere, not a fault: a saturated server still serves the
    /// sessions it already has.
    fn readiness(&self) -> (bool, &'static str) {
        if self.shutdown.load(Ordering::Relaxed) {
            return (false, "shutting_down");
        }
        if self.session_manager.active_count() >= self.config.session_config.max_sessions {
            return (false, "at_session_capacity");
        }
        if self.exec_slots.saturated() {
            return (false, "at_exec_capacity");
        }
        (true, "ok")
    }

    /// Sample the live gauge values at scrape time.
    fn current_gauges(&self) -> Gauges {
        Gauges {
            sessions_active: self.session_manager.active_count() as u64,
            sessions_total: self.session_manager.total_count() as u64,
            exec_in_flight: self.exec_slots.in_flight(),
            sessions_disk_bytes: self.session_manager.total_disk_bytes(),
            workers_live: self.pool_stats.live(),
            requests_in_flight: self.pool_stats.busy(),
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
            workers_live: self.pool_stats.live(),
            requests_in_flight: self.pool_stats.busy(),
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

    pub fn handle_list_sessions(
        &self,
        caller: Option<&str>,
    ) -> std::result::Result<ListSessionsResponse, ApiError> {
        let sessions = self
            .session_manager
            .list_sessions(caller, session_status_row);
        Ok(ListSessionsResponse {
            count: sessions.len(),
            sessions,
        })
    }

    pub fn handle_get_session(
        &self,
        id: &str,
        caller: Option<&str>,
    ) -> std::result::Result<SessionStatusResponse, ApiError> {
        self.session_manager
            .get_session(id, caller, session_status_row)
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

    /// Buffered exec: start the run and wait for it to finish.
    pub fn handle_exec(
        &self,
        id: &str,
        body: &str,
        caller: Option<&str>,
    ) -> std::result::Result<ExecResponse, ApiError> {
        let run = self.start_exec(id, body, caller)?;
        self.collect_exec(run)
    }

    /// Validate the request, prepare the session, and spawn the exec worker.
    ///
    /// Split from collection so the buffered and streaming endpoints share
    /// every decision about *what* runs. Everything that can fail with a 4xx
    /// happens here, before a worker or a permit is taken.
    fn start_exec(
        &self,
        id: &str,
        body: &str,
        caller: Option<&str>,
    ) -> std::result::Result<RunningExec, ApiError> {
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
            // Rewound every exec: a request without stdin must see EOF, not
            // the last run's leftovers.
            env.set_stdin(req.stdin.clone().unwrap_or_default().into_bytes());
            if let Some(ref vars) = req.env {
                for (k, v) in vars {
                    env.add_env(k.clone(), v.clone());
                }
            }
            // Re-seed the disk counter from the work dir's actual footprint so
            // agent-side file writes since the last exec are reflected. Within
            // the exec the counter is then maintained incrementally (O(1)/write).
            if env.max_disk_bytes().is_some() {
                env.seed_disk_used(dir_size(&work_dir));
            }
        }

        // Validated synchronously so bad input is a 400 before a worker runs.
        let resolved_deps = resolve_exec_deps(&req, &work_dir)?;

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

        // Vendoring runs on the worker but its result belongs in the response,
        // including on timeout; a slot keeps the channel carrying the exit code.
        let lock_slot: Arc<Mutex<Option<vendor::Lockfile>>> = Arc::new(Mutex::new(None));

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
            executor::resolve_language(&lang)?;
            let entry = req
                .entry
                .clone()
                .ok_or_else(|| ApiError::BadRequest("'entry' is required with 'files'".into()))?;
            if !files.contains_key(&entry) {
                return Err(ApiError::BadRequest(format!(
                    "Entry '{entry}' not found in 'files' map"
                )));
            }
            // Validate dependency names/ranges before spawning (fast 400);
            // the network-bound vendoring itself runs on the worker.
            let deps = resolved_deps.clone();
            let lock_in = req.lockfile.clone();
            let registry = self.config.npm_registry.clone();
            let work_dir_clone = work_dir.clone();
            let limits_clone = limits.clone();
            let cancel_worker = cancel.clone();
            let lock_out = lock_slot.clone();
            std::thread::Builder::new()
                .stack_size(EXEC_THREAD_STACK_BYTES)
                .spawn(move || {
                    let permit = permit; // held for the duration of execution
                    let result = (|| {
                        vendor_for_exec(
                            &registry,
                            deps.as_ref(),
                            lock_in.as_ref(),
                            &work_dir_clone,
                            &limits_clone,
                            &lock_out,
                        )?;
                        executor::execute_source_project(
                            &files,
                            &entry,
                            &lang,
                            exec_env,
                            &work_dir_clone,
                            &limits_clone,
                            Some(cancel_worker),
                        )
                    })();
                    drop(permit); // free the slot once execution is done
                    let _ = tx.send(result);
                })
                .map_err(|e| ApiError::Internal(format!("Failed to spawn exec thread: {e}")))?;
        } else if let Some(source) = req.source {
            // Source execution: write code to session FS and run via language runtime
            let lang = req.language.unwrap_or_else(|| "javascript".into());
            // Validate language before spawning so callers get a 400 immediately
            executor::resolve_language(&lang)?;
            let deps = resolved_deps.clone();
            let lock_in = req.lockfile.clone();
            let registry = self.config.npm_registry.clone();
            let work_dir_clone = work_dir.clone();
            let limits_clone = limits.clone();
            let cancel_worker = cancel.clone();
            let lock_out = lock_slot.clone();
            std::thread::Builder::new()
                .stack_size(EXEC_THREAD_STACK_BYTES)
                .spawn(move || {
                    let permit = permit; // held for the duration of execution
                    let result = (|| {
                        vendor_for_exec(
                            &registry,
                            deps.as_ref(),
                            lock_in.as_ref(),
                            &work_dir_clone,
                            &limits_clone,
                            &lock_out,
                        )?;
                        executor::execute_source(
                            &source,
                            &lang,
                            exec_env,
                            &work_dir_clone,
                            &limits_clone,
                            Some(cancel_worker),
                        )
                    })();
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

        Ok(RunningExec {
            rx,
            cancel,
            wasi_env,
            lock_slot,
            start,
            timeout,
            timeout_secs,
        })
    }

    /// Streaming exec: emit output as Server-Sent Events while it runs.
    ///
    /// Takes the raw socket rather than `respond_json`, because the point is
    /// to send bytes before the result exists. A final `result` event carries
    /// the same object the buffered endpoint returns, so a client can ignore
    /// the intermediate frames.
    fn handle_exec_stream(
        &self,
        request: Request,
        id: &str,
        body: &str,
        caller: Option<&str>,
        log: &ReqLog,
    ) {
        let run = match self.start_exec(id, body, caller) {
            Ok(run) => run,
            // Nothing has been written yet, so a failure to start is still an
            // ordinary JSON error response.
            Err(e) => {
                let _ = self.respond_json(request, Err::<(), _>(e), log);
                return;
            }
        };

        let mut out = request.into_writer();
        let headers = "HTTP/1.1 200 OK\r\n\
             Content-Type: text/event-stream\r\n\
             Cache-Control: no-cache\r\n\
             Connection: close\r\n\r\n";
        if out.write_all(headers.as_bytes()).is_err() {
            run.cancel.store(true, Ordering::Relaxed);
            return;
        }

        let response = self.stream_exec(run, &mut out);
        let event = serde_json::to_string(&response)
            .unwrap_or_else(|e| format!(r#"{{"error":"failed to serialize result: {e}"}}"#));
        let _ = out.write_all(format!("event: result\ndata: {event}\n\n").as_bytes());
        let _ = out.flush();
        log.emit(200);
    }

    /// Drive a running exec, writing `output` events as it produces them, and
    /// return the response that describes how it ended.
    fn stream_exec(&self, run: RunningExec, out: &mut dyn std::io::Write) -> ExecResponse {
        let RunningExec {
            rx,
            cancel,
            wasi_env,
            lock_slot,
            start,
            timeout,
            timeout_secs,
        } = run;

        let (mut seen_out, mut seen_err) = (0usize, 0usize);
        // False once the client has gone away, which is the signal to stop.
        let mut flush = |out: &mut dyn std::io::Write| -> bool {
            let (stdout, stderr) = match wasi_env.lock() {
                Ok(env) => (
                    take_new_output(&env.get_stdout(), &mut seen_out),
                    take_new_output(&env.get_stderr(), &mut seen_err),
                ),
                Err(_) => return true,
            };
            if stdout.is_empty() && stderr.is_empty() {
                return true;
            }
            let payload = serde_json::json!({"stdout": stdout, "stderr": stderr});
            out.write_all(format!("event: output\ndata: {payload}\n\n").as_bytes())
                .and_then(|()| out.flush())
                .is_ok()
        };

        let exec_result = loop {
            match rx.recv_timeout(STREAM_POLL_INTERVAL) {
                Ok(result) => {
                    flush(out);
                    break Some(result);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !flush(out) || start.elapsed() >= timeout {
                        cancel.store(true, Ordering::Relaxed);
                        break None;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let truncated = read_env_truncated(&wasi_env);
        if truncated {
            self.metrics.record_output_truncated();
        }
        let lockfile = lock_slot.lock().ok().and_then(|mut l| l.take());

        // Repeat the full output so a client that dropped a frame still has it.
        let (stdout, stderr) = (read_env_stdout(&wasi_env), read_env_stderr(&wasi_env));
        match exec_result {
            Some(Ok(exit_code)) => {
                self.metrics.record_exec_success(duration_ms);
                ExecResponse {
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms,
                    output_truncated: truncated,
                    error: None,
                    lockfile,
                }
            }
            Some(Err(e)) => {
                self.metrics.record_exec_error(duration_ms);
                ExecResponse {
                    stdout,
                    stderr,
                    exit_code: -1,
                    duration_ms,
                    output_truncated: truncated,
                    error: Some(e.to_string()),
                    lockfile,
                }
            }
            None => {
                self.metrics.record_exec_timeout(duration_ms);
                ExecResponse {
                    stdout,
                    stderr,
                    exit_code: -1,
                    duration_ms,
                    output_truncated: truncated,
                    error: Some(format!("Execution timed out after {timeout_secs}s")),
                    lockfile,
                }
            }
        }
    }

    /// Wait for a running exec and build its response.
    fn collect_exec(&self, run: RunningExec) -> std::result::Result<ExecResponse, ApiError> {
        let RunningExec {
            rx,
            cancel,
            wasi_env,
            lock_slot,
            start,
            timeout,
            timeout_secs,
        } = run;
        let take_lock = || lock_slot.lock().ok().and_then(|mut l| l.take());

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
                    lockfile: take_lock(),
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
                    lockfile: take_lock(),
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
                    lockfile: take_lock(),
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
                    lockfile: take_lock(),
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

/// One pass over both host-level caches: trim the npm cache to `max_bytes`
/// (`None` = unlimited, debris only) and drop runtime artifacts left behind by
/// an earlier wasmhub pin. Returns the npm result and the runtime file count.
fn sweep_caches(max_bytes: Option<u64>) -> (vendor::Eviction, usize) {
    let npm = match vendor::default_cache_dir() {
        Some(root) => vendor::evict_npm_cache(&root, max_bytes.unwrap_or(0), CACHE_ENTRY_MIN_AGE),
        None => vendor::Eviction::default(),
    };
    let runtimes = crate::runtime::runtime_cache::RuntimeCache::new()
        .map(|c| c.prune_unreferenced())
        .unwrap_or(0);
    (npm, runtimes)
}

/// Log a sweep, and only a sweep that did something: a quiet cache should stay
/// quiet in the access log.
fn report_cache_sweep((npm, runtimes): (vendor::Eviction, usize)) {
    if npm.removed > 0 {
        eprintln!(
            "Cache: evicted {} npm entr(ies), freed {}, {} in use",
            npm.removed,
            fmt_mb(npm.freed_bytes),
            fmt_mb(npm.total_bytes)
        );
    }
    if runtimes > 0 {
        eprintln!("Cache: removed {runtimes} superseded runtime artifact(s)");
    }
    if npm.still_over {
        eprintln!(
            "Cache: still over the ceiling at {}; everything left was installed from too recently to evict",
            fmt_mb(npm.total_bytes)
        );
    }
}

fn fmt_mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// Trim the caches on a long interval for as long as the server runs.
///
/// Sleeps in slices so shutdown is not held up by a pass that is not due.
fn spawn_cache_sweeper(
    max_bytes: Option<u64>,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut waited = Duration::ZERO;
        while !shutdown.load(Ordering::Relaxed) {
            std::thread::sleep(ACCEPT_POLL_INTERVAL);
            waited += ACCEPT_POLL_INTERVAL;
            if waited < CACHE_SWEEP_INTERVAL {
                continue;
            }
            waited = Duration::ZERO;
            report_cache_sweep(sweep_caches(max_bytes));
        }
    })
}

/// Whether `host` resolves to this machine only. Conservative: a name other
/// than `localhost` is treated as remote rather than resolved, so an unknown
/// host never talks its way past [`validate_bind`].
fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    // v6 literals are written `[::1]` in URLs and `::1` on the command line.
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// Refuse to serve an unauthenticated exec API to the network.
///
/// The agent runs arbitrary WASM and JavaScript on request, so an open server
/// on a routable address is a remote-code-execution endpoint for anyone who can
/// reach it. Loopback is the default; going wider needs either auth or a
/// deliberate `--insecure`.
pub fn validate_bind(host: &str, auth_enabled: bool, insecure: bool) -> Result<()> {
    if auth_enabled || insecure || is_loopback_host(host) {
        return Ok(());
    }
    Err(WasmrunError::from(format!(
        "refusing to bind {host} with authentication disabled.\n\
         \x20  The agent server executes arbitrary code, so an open listener on a\n\
         \x20  non-loopback address lets anyone who can reach it run code on this machine.\n\
         \x20  Pick one:\n\
         \x20    --auth <PATH>      enable API-key auth (keys: wasmrun agent --hash-key <key>)\n\
         \x20    --host {DEFAULT_HOST}   keep the server on loopback (the default)\n\
         \x20    --insecure         bind anyway; only on a network you control"
    )))
}

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

/// Bytes appended to a session buffer since `seen`, advancing `seen`.
///
/// The buffers only grow during an exec, so an offset is enough. A trailing
/// partial UTF-8 character is held back rather than emitted as U+FFFD.
fn take_new_output(buffer: &[u8], seen: &mut usize) -> String {
    if buffer.len() <= *seen {
        return String::new();
    }
    let fresh = &buffer[*seen..];
    let valid = match std::str::from_utf8(fresh) {
        Ok(s) => s.len(),
        Err(e) => e.valid_up_to(),
    };
    *seen += valid;
    String::from_utf8_lossy(&fresh[..valid]).into_owned()
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
            workers: 0,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            max_cache_bytes: Some(DEFAULT_MAX_CACHE_MB * 1024 * 1024),
            auth: None,
            auth_path: None,
            npm_registry: crate::agent::vendor::DEFAULT_NPM_REGISTRY.to_string(),
            host: DEFAULT_HOST.to_string(),
            insecure: false,
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
        open_server_with_concurrency(port, 100)
    }

    /// The same, with an explicit server-wide exec cap.
    fn open_server_with_concurrency(port: u16, max_concurrent_exec: usize) -> AgentServer {
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
            max_concurrent_exec,
            workers: 0,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            max_cache_bytes: Some(DEFAULT_MAX_CACHE_MB * 1024 * 1024),
            auth: None,
            auth_path: None,
            npm_registry: crate::agent::vendor::DEFAULT_NPM_REGISTRY.to_string(),
            host: DEFAULT_HOST.to_string(),
            insecure: false,
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
            workers: 0,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            max_cache_bytes: Some(DEFAULT_MAX_CACHE_MB * 1024 * 1024),
            auth: Some(Arc::new(auth_config_for(tenants))),
            auth_path: None,
            npm_registry: crate::agent::vendor::DEFAULT_NPM_REGISTRY.to_string(),
            host: DEFAULT_HOST.to_string(),
            insecure: false,
        })
    }

    /// A server whose auth config is loaded from a full TOML body (so tests can
    /// include `[tenants.rate]` sub-tables). Generous server-wide caps so the
    /// per-tenant ceilings are what's actually exercised.
    fn auth_server_from_toml(toml: &str) -> AgentServer {
        auth_server_from_toml_on_port(0, toml)
    }

    /// The same, bound to `port` so it can be driven over HTTP.
    fn auth_server_from_toml_on_port(port: u16, toml: &str) -> AgentServer {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, toml.as_bytes()).unwrap();
        let auth = AuthConfig::load(f.path()).unwrap();
        AgentServer::new(AgentConfig {
            port,
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
            workers: 0,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            max_cache_bytes: Some(DEFAULT_MAX_CACHE_MB * 1024 * 1024),
            auth: Some(Arc::new(auth)),
            auth_path: None,
            npm_registry: crate::agent::vendor::DEFAULT_NPM_REGISTRY.to_string(),
            host: DEFAULT_HOST.to_string(),
            insecure: false,
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

    // Hand-built WASM that reads up to 64 bytes from fd 0 and writes exactly
    // what it read to fd 1. Verified against wasmtime before being pasted here.
    fn echo_stdin_wasm() -> Vec<u8> {
        #[rustfmt::skip]
        let wasm: Vec<u8> = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0c, 0x02, 0x60,
            0x04, 0x7f, 0x7f, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x00, 0x00, 0x02, 0x44,
            0x02, 0x16, 0x77, 0x61, 0x73, 0x69, 0x5f, 0x73, 0x6e, 0x61, 0x70, 0x73,
            0x68, 0x6f, 0x74, 0x5f, 0x70, 0x72, 0x65, 0x76, 0x69, 0x65, 0x77, 0x31,
            0x07, 0x66, 0x64, 0x5f, 0x72, 0x65, 0x61, 0x64, 0x00, 0x00, 0x16, 0x77,
            0x61, 0x73, 0x69, 0x5f, 0x73, 0x6e, 0x61, 0x70, 0x73, 0x68, 0x6f, 0x74,
            0x5f, 0x70, 0x72, 0x65, 0x76, 0x69, 0x65, 0x77, 0x31, 0x08, 0x66, 0x64,
            0x5f, 0x77, 0x72, 0x69, 0x74, 0x65, 0x00, 0x00, 0x03, 0x02, 0x01, 0x01,
            0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x13, 0x02, 0x06, 0x6d, 0x65, 0x6d,
            0x6f, 0x72, 0x79, 0x02, 0x00, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
            0x00, 0x02, 0x0a, 0x3c, 0x01, 0x3a, 0x00, 0x41, 0x00, 0x41, 0xe4, 0x00,
            0x36, 0x02, 0x00, 0x41, 0x04, 0x41, 0xc0, 0x00, 0x36, 0x02, 0x00, 0x41,
            0x00, 0x41, 0x00, 0x41, 0x01, 0x41, 0x08, 0x10, 0x00, 0x1a, 0x41, 0x10,
            0x41, 0xe4, 0x00, 0x36, 0x02, 0x00, 0x41, 0x14, 0x41, 0x08, 0x28, 0x02,
            0x00, 0x36, 0x02, 0x00, 0x41, 0x01, 0x41, 0x10, 0x41, 0x01, 0x41, 0x18,
            0x10, 0x01, 0x1a, 0x0b,
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

    /// `stdin` on the request reaches a program reading fd 0.
    #[test]
    fn test_exec_stdin_reaches_the_program() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("echo.wasm"), echo_stdin_wasm()).unwrap();

        let resp = server
            .handle_exec(
                &id,
                r#"{"wasm_path": "echo.wasm", "stdin": "piped input\n"}"#,
                None,
            )
            .unwrap();

        assert_eq!(resp.exit_code, 0, "error: {:?}", resp.error);
        assert_eq!(resp.stdout, "piped input\n");

        server.session_manager.destroy_all().unwrap();
    }

    /// Without `stdin`, fd 0 is at EOF: the program reads nothing and does
    /// not hang.
    #[test]
    fn test_exec_without_stdin_reads_eof() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("echo.wasm"), echo_stdin_wasm()).unwrap();

        let resp = server
            .handle_exec(&id, r#"{"wasm_path": "echo.wasm"}"#, None)
            .unwrap();

        assert_eq!(resp.exit_code, 0, "error: {:?}", resp.error);
        assert_eq!(resp.stdout, "");

        server.session_manager.destroy_all().unwrap();
    }

    /// Each exec gets its own stdin, neither inherited nor pre-consumed.
    #[test]
    fn test_exec_stdin_is_per_request() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;
        let work_dir = server
            .session_manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();
        std::fs::write(work_dir.join("echo.wasm"), echo_stdin_wasm()).unwrap();

        let first = server
            .handle_exec(&id, r#"{"wasm_path": "echo.wasm", "stdin": "one"}"#, None)
            .unwrap();
        assert_eq!(first.stdout, "one");

        let second = server
            .handle_exec(&id, r#"{"wasm_path": "echo.wasm", "stdin": "two"}"#, None)
            .unwrap();
        assert_eq!(second.stdout, "two");

        let third = server
            .handle_exec(&id, r#"{"wasm_path": "echo.wasm"}"#, None)
            .unwrap();
        assert_eq!(third.stdout, "");

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

    /// Integration test: fetches the nodejs runtime from wasmhub (>= v0.3.0,
    /// which ships native CommonJS) and verifies that a multi-file project
    /// resolves a sibling file via relative `require()` — the v0.21.1 exit
    /// criteria.
    ///
    /// Ignored by default so the test suite stays offline-friendly (needs
    /// network on first run to fetch the runtime; cached after). Run with:
    ///   cargo test --release multi_file_js_require_integration -- --ignored --nocapture
    ///
    /// On a cold `~/.wasmrun/runtimes` cache, run the ignored tests with
    /// `--test-threads=1` the first time. The runtime fetch happens inside the
    /// timeout-guarded exec worker, so a parallel run has every test racing to
    /// download the same two artifacts and spending its timeout doing it.
    #[test]
    #[ignore]
    fn test_multi_file_js_require_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "files": {
                "main.js": "const {x} = require('./lib'); console.log(x);",
                "lib.js": "module.exports = {x: 2};"
            },
            "entry": "main.js",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains('2'),
            "stdout did not contain require()d value: {:?}",
            resp.stdout
        );

        // Verify the sibling file was actually written to the session FS
        let lib = server.handle_read_file(&id, "lib.js", None).unwrap();
        assert!(lib.content.contains("module.exports"));

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: bare `require('<name>')` resolves through the
    /// project's `node_modules/<name>` tree (wasmhub nodejs >= v0.3.0).
    /// Ignored by default; see test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_node_modules_resolution_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "files": {
                "main.js": "const greet = require('greet'); console.log(greet('agent'));",
                "node_modules/greet/index.js": "module.exports = (n) => 'hello ' + n;"
            },
            "entry": "main.js",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("hello agent"),
            "stdout did not contain node_modules output: {:?}",
            resp.stdout
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: the runtime's event loop and stdlib globals —
    /// setTimeout, Buffer, TextEncoder and a built-in module (path) — work
    /// end-to-end through /exec (wasmhub nodejs >= v0.3.0).
    /// Ignored by default; see test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_js_stdlib_and_timers_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "source": "const path = require('path'); console.log(path.join('a','b')); console.log(Buffer.from('hi').toString('base64')); console.log(new TextEncoder().encode('abc').length); setTimeout(() => console.log('timer-fired'), 5);",
            "language": "javascript",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("a/b"),
            "path.join output missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains("aGk="),
            "Buffer base64 output missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains('3'),
            "TextEncoder length missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains("timer-fired"),
            "setTimeout callback did not run before exit: {:?}",
            resp.stdout
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: single-file TypeScript is transpiled in-sandbox by
    /// the swc WASI transpiler, then run by the nodejs runtime.
    ///
    /// Needs the `swc` artifact on the pinned wasmhub release (or a
    /// `WASMRUN_WASMHUB_BASE_URL` override serving it). Ignored by default;
    /// see test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_typescript_single_file_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "source": "interface P {x: number}; const p: P = {x: 7}; console.log(p.x * 6);",
            "language": "typescript",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("42"),
            "stdout did not contain expected output: {:?}",
            resp.stdout
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: multi-file TypeScript with an ES `import` between
    /// files — the v0.21.2 exit criteria. The transpiler lowers ESM to
    /// CommonJS and the runtime resolves the emitted `.js` sibling.
    /// Ignored by default; see test_typescript_single_file_integration.
    #[test]
    #[ignore]
    fn test_typescript_multi_file_import_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "files": {
                "main.ts": "import {x} from './lib'; console.log(x)",
                "lib.ts": "export const x=2"
            },
            "entry": "main.ts",
            "language": "typescript",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains('2'),
            "stdout did not contain imported value: {:?}",
            resp.stdout
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: malformed TypeScript surfaces a clear transpilation
    /// error referencing the original .ts file, line and column.
    /// Ignored by default; see test_typescript_single_file_integration.
    #[test]
    #[ignore]
    fn test_typescript_syntax_error_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "source": "const x: = broken(",
            "language": "typescript",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(resp.exit_code, -1);
        let err = resp.error.expect("expected a transpilation error");
        assert!(
            err.contains("TypeScript transpilation failed") && err.contains("_run_.ts:1:"),
            "error should name the failure and the .ts location: {err}"
        );

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_invalid_dependencies_rejected_before_spawn() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // Invalid package name → immediate 400, no worker spawned, no network.
        let body = r#"{"source": "1", "dependencies": {"../evil": "*"}}"#;
        let err = server.handle_exec(&id, body, None).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Invalid package name"));

        // Composite ranges are supported now, so this needs a malformed one.
        let body = r#"{"source": "1", "dependencies": {"lodash": ">=not.a.version"}}"#;
        let err = server.handle_exec(&id, body, None).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Unsupported version range"));

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_take_new_output_emits_only_the_delta() {
        let mut seen = 0;
        assert_eq!(take_new_output(b"hello", &mut seen), "hello");
        assert_eq!(seen, 5);
        assert_eq!(take_new_output(b"hello", &mut seen), "");
        assert_eq!(take_new_output(b"hello world", &mut seen), " world");
        assert_eq!(seen, 11);
    }

    #[test]
    fn test_take_new_output_holds_back_split_utf8() {
        // A character split across samples must not become U+FFFD.
        let full = "aé".as_bytes(); // [0x61, 0xC3, 0xA9]
        let mut seen = 0;
        assert_eq!(take_new_output(&full[..2], &mut seen), "a");
        assert_eq!(seen, 1, "the partial character is not consumed");
        assert_eq!(take_new_output(full, &mut seen), "é");
        assert_eq!(seen, 3);
    }

    #[test]
    fn test_stream_exec_writes_output_and_result_events() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // The shell path is fast, so this covers event ordering, not sampling.
        let run = server
            .start_exec(&id, r#"{"command": "echo streamed"}"#, None)
            .unwrap();
        let mut out: Vec<u8> = Vec::new();
        let response = server.stream_exec(run, &mut out);

        assert_eq!(response.exit_code, 0);
        assert!(response.stdout.contains("streamed"));

        let events = String::from_utf8(out).unwrap();
        // Frames are best-effort, but anything emitted must be well-formed.
        for frame in events.split("\n\n").filter(|f| !f.trim().is_empty()) {
            assert!(frame.starts_with("event: output\ndata: {"), "{frame}");
        }

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_stream_flag_is_parsed_from_the_request() {
        let with: ExecRequest = serde_json::from_str(r#"{"source":"1","stream":true}"#).unwrap();
        assert_eq!(with.stream, Some(true));
        let without: ExecRequest = serde_json::from_str(r#"{"source":"1"}"#).unwrap();
        assert_eq!(without.stream, None, "buffered stays the default");
    }

    #[test]
    fn test_list_sessions() {
        let server = test_server();
        assert_eq!(server.handle_list_sessions(None).unwrap().count, 0);

        let a = server.handle_create_session().unwrap().session_id;
        let b = server.handle_create_session().unwrap().session_id;
        let listed = server.handle_list_sessions(None).unwrap();
        assert_eq!(listed.count, 2);
        let ids: Vec<&str> = listed
            .sessions
            .iter()
            .map(|s| s.session_id.as_str())
            .collect();
        assert!(ids.contains(&a.as_str()) && ids.contains(&b.as_str()));
        assert!(listed.sessions.iter().all(|s| s.state == "active"));

        server.handle_delete_session(&a, None).unwrap();
        let listed = server.handle_list_sessions(None).unwrap();
        assert_eq!(listed.count, 1);
        assert_eq!(listed.sessions[0].session_id, b);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_list_sessions_is_tenant_scoped() {
        let server = test_server();
        let mine = server
            .handle_create_session_with_body("{}", Some("tenant-a"))
            .unwrap()
            .session_id;
        server
            .handle_create_session_with_body("{}", Some("tenant-b"))
            .unwrap();

        // A tenant must not learn that another tenant's sessions exist.
        let listed = server.handle_list_sessions(Some("tenant-a")).unwrap();
        assert_eq!(listed.count, 1);
        assert_eq!(listed.sessions[0].session_id, mine);

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_resolve_exec_deps_from_package_json() {
        let work_dir = tempfile::tempdir().unwrap();
        let parse = |body: &str| -> ExecRequest { serde_json::from_str(body).unwrap() };

        // Without the flag an uploaded package.json is inert.
        let req = parse(
            r#"{"source":"1","files":{"package.json":"{\"dependencies\":{\"lodash\":\"^4.0.0\"}}"}}"#,
        );
        assert!(resolve_exec_deps(&req, work_dir.path()).unwrap().is_none());

        let req = parse(
            r#"{"source":"1","install_package_json":true,"files":{"package.json":"{\"dependencies\":{\"lodash\":\"^4.0.0\"},\"devDependencies\":{\"jest\":\"^29\"}}"}}"#,
        );
        let deps = resolve_exec_deps(&req, work_dir.path()).unwrap().unwrap();
        assert_eq!(deps.get("lodash").map(String::as_str), Some("^4.0.0"));
        assert!(!deps.contains_key("jest"), "devDependencies are ignored");

        // The explicit map wins over the file.
        let req = parse(
            r#"{"source":"1","install_package_json":true,"dependencies":{"lodash":"4.17.21"},"files":{"package.json":"{\"dependencies\":{\"lodash\":\"^4.0.0\"}}"}}"#,
        );
        let deps = resolve_exec_deps(&req, work_dir.path()).unwrap().unwrap();
        assert_eq!(deps.get("lodash").map(String::as_str), Some("4.17.21"));

        // Falls back to a package.json already in the session.
        std::fs::write(
            work_dir.path().join("package.json"),
            r#"{"dependencies":{"greet":"^1.0.0"}}"#,
        )
        .unwrap();
        let req = parse(r#"{"source":"1","install_package_json":true}"#);
        let deps = resolve_exec_deps(&req, work_dir.path()).unwrap().unwrap();
        assert_eq!(deps.get("greet").map(String::as_str), Some("^1.0.0"));
    }

    #[test]
    fn test_resolve_exec_deps_package_json_errors() {
        let work_dir = tempfile::tempdir().unwrap();
        let parse = |body: &str| -> ExecRequest { serde_json::from_str(body).unwrap() };

        let req = parse(r#"{"source":"1","install_package_json":true}"#);
        let err = resolve_exec_deps(&req, work_dir.path()).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("no package.json"));

        let req = parse(
            r#"{"source":"1","install_package_json":true,"files":{"package.json":"{not json"}}"#,
        );
        let err = resolve_exec_deps(&req, work_dir.path()).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Invalid package.json"));

        let req = parse(
            r#"{"source":"1","install_package_json":true,"files":{"package.json":"{\"dependencies\":{\"lodash\":5}}"}}"#,
        );
        let err = resolve_exec_deps(&req, work_dir.path()).unwrap_err();
        assert_eq!(err.status_code(), 400);

        // Bad names from a package.json are validated like any other.
        let req = parse(
            r#"{"source":"1","install_package_json":true,"files":{"package.json":"{\"dependencies\":{\"../evil\":\"*\"}}"}}"#,
        );
        let err = resolve_exec_deps(&req, work_dir.path()).unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Invalid package name"));
    }

    /// Integration test: the 0.21.3 exit criteria — a project depending on a
    /// real pure-JS npm package executes in one request, resolved through the
    /// runtime's own `require()` from the vendored node_modules.
    ///
    /// Needs network (npm registry + wasmhub runtime fetch on first run).
    /// Ignored by default; see test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_npm_dependency_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // `ms` rather than lodash: the vendoring path is what is under test
        // and 3 KB exercises all of it, where lodash's ~540 KB bundle takes
        // ~24 s to parse in release and past 300 s under a debug build.
        let body = r#"{
            "source": "const ms = require('ms'); console.log(ms('2 days') + '|' + ms(60000));",
            "language": "javascript",
            "dependencies": {"ms": "^2.1.3"},
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("172800000|1m"),
            "vendored package output missing: {:?}",
            resp.stdout
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: the 0.21.4 polyfill tail — `URL`/`URLSearchParams`,
    /// `crypto.getRandomValues`/`randomUUID`, and `structuredClone` run
    /// without ReferenceError, and `fetch` rejects with a clear
    /// network-not-supported message instead of a bare ReferenceError
    /// (wasmhub nodejs >= v0.3.2).
    /// Ignored by default; see test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_js_web_globals_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "source": "const u = new URL('https://example.com:8443/a/../b?x=1'); u.searchParams.append('y', 'z z'); console.log(u.href); const r = crypto.getRandomValues(new Uint8Array(16)); console.log('rand=' + (r.length === 16 && Array.from(r).some(b => b !== 0))); console.log('uuid=' + /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(crypto.randomUUID())); const o = {a: [1, 2], m: new Map([['k', 'v']])}; o.self = o; const c = structuredClone(o); console.log('clone=' + (c.self === c && c.m.get('k') === 'v' && c.a !== o.a)); fetch('http://example.com').catch(e => console.log('fetch=' + /network/.test(e.message)));",
            "language": "javascript",
            "timeout": 120
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("https://example.com:8443/b?x=1&y=z+z"),
            "URL/searchParams output missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains("rand=true"),
            "getRandomValues output missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains("uuid=true"),
            "randomUUID output missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains("clone=true"),
            "structuredClone output missing: {:?}",
            resp.stdout
        );
        assert!(
            resp.stdout.contains("fetch=true"),
            "fetch should reject with a clear network-unsupported error: {:?}",
            resp.stdout
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: an ESM-only package (nanoid declares `"type":
    /// "module"` and puts its entry only in an `exports` map) installs, is
    /// lowered to CommonJS, and loads through the runtime's own `require()`.
    /// Needs network; ignored by default, see
    /// test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_esm_only_package_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "source": "const { nanoid } = require('nanoid'); console.log('len=' + nanoid().length);",
            "language": "javascript",
            "dependencies": {"nanoid": "^6"},
            "timeout": 240
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("len=21"),
            "ESM package output missing: {:?}; stderr: {}",
            resp.stdout,
            resp.stderr
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: an ESM package whose own dependency is also ESM-only
    /// (p-limit → yocto-queue), so every level of the tree must be lowered.
    /// Needs network; ignored by default, see
    /// test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_esm_transitive_dependency_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        let body = r#"{
            "source": "const pLimit = require('p-limit').default; const limit = pLimit(2); Promise.all([limit(() => 1), limit(() => 2)]).then(r => console.log('plimit=' + JSON.stringify(r)));",
            "language": "javascript",
            "dependencies": {"p-limit": "^6"},
            "timeout": 240
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        assert!(
            resp.stdout.contains("plimit=[1,2]"),
            "transitive ESM output missing: {:?}; stderr: {}",
            resp.stdout,
            resp.stderr
        );

        server.session_manager.destroy_all().unwrap();
    }

    /// Integration test: the built-in module tail added by wasmhub v0.4.0 —
    /// `crypto`, `querystring`, `string_decoder`, `url` helpers, `fs/promises`,
    /// `timers/promises`, `node:` aliases, and the deliberately-throwing `zlib`
    /// stub — all reachable from agent code.
    ///
    /// Also the first execution `fs/promises` and `timers/promises` get:
    /// wasmhub's node harness cannot reach them, so they shipped unverified.
    /// Ignored by default; see test_multi_file_js_require_integration.
    #[test]
    #[ignore]
    fn test_js_builtin_modules_tail_integration() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // One 64-byte block on purpose: hashing is pure JS, ~1.6 s per block
        // in release and ~27 s under the debug build tests run on. createHmac
        // is left out for the same reason (4 blocks, same code path).
        let body = r#"{
            "source": "const crypto = require('node:crypto'); console.log('sha=' + crypto.createHash('sha256').update('abc').digest('hex').slice(0, 8)); console.log('qs=' + require('querystring').stringify({a: 1, b: 'x y'})); const {StringDecoder} = require('string_decoder'); const d = new StringDecoder('utf8'); console.log('sd=' + d.write(Buffer.from([0xe2, 0x82])) + d.end(Buffer.from([0xac]))); console.log('url=' + require('url').fileURLToPath('file:///tmp/a.txt')); try { require('zlib').gzipSync('x'); } catch (e) { console.log('zlib=' + (e.code === 'ERR_NOT_SUPPORTED')); } const fsp = require('node:fs/promises'); const tp = require('timers/promises'); (async () => { await fsp.writeFile('/promises.txt', 'hi'); console.log('fsp=' + (await fsp.readFile('/promises.txt', 'utf8'))); await tp.setTimeout(5); console.log('tp=ok'); })();",
            "language": "javascript",
            "timeout": 240
        }"#;
        let resp = server.handle_exec(&id, body, None).unwrap();
        assert_eq!(
            resp.exit_code, 0,
            "exit_code != 0; stderr: {}; error: {:?}",
            resp.stderr, resp.error
        );
        for expected in [
            "sha=ba7816bf",   // crypto.createHash, sha256("abc")
            "qs=a=1&b=x%20y", // querystring.stringify
            "sd=\u{20ac}",    // string_decoder holds back a split UTF-8 euro sign
            "url=/tmp/a.txt", // url.fileURLToPath
            "zlib=true",      // present-but-throwing stub, named error code
            "fsp=hi",         // fs/promises write + read round-trip
            "tp=ok",          // timers/promises setTimeout resolves
        ] {
            assert!(
                resp.stdout.contains(expected),
                "missing {expected:?} in stdout: {:?}; stderr: {}",
                resp.stdout,
                resp.stderr
            );
        }

        server.session_manager.destroy_all().unwrap();
    }

    #[test]
    fn test_exec_typescript_language_passes_validation() {
        let server = test_server();
        let id = server.handle_create_session().unwrap().session_id;

        // "ts" must be accepted at the synchronous validation stage — any
        // runtime/transpiler fetch failure surfaces later as ExecResponse.error,
        // not as an ApiError from handle_exec itself.
        let body = r#"{"files": {"main.ts": "const n: number = 1;"}, "entry": "main.ts", "language": "ts"}"#;
        let result = server.handle_exec(&id, body, None);
        assert!(
            result.is_ok(),
            "valid TS files+entry should not return ApiError, got: {result:?}"
        );

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
        assert_eq!(tools.len(), 7);
        assert_eq!(tools[0]["type"], "function");
        assert!(tools[0]["function"]["name"].is_string());
        assert!(tools[0]["function"]["parameters"].is_object());
    }

    #[test]
    fn test_get_tools_anthropic_format() {
        let server = test_server();
        let result = server.handle_get_tools("anthropic").unwrap();
        let tools = result.as_array().unwrap();
        assert_eq!(tools.len(), 7);
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

    /// A free localhost port, released immediately for a server to bind.
    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    /// Run `server` on its configured port in a background thread and return
    /// the API base URL, once the listener is accepting.
    fn serve(server: AgentServer) -> String {
        let base = format!("http://127.0.0.1:{}{API_PREFIX}", server.config.port);
        std::thread::spawn(move || {
            let _ = server.start();
        });
        for _ in 0..200 {
            // Any HTTP answer means it is up — 401 included, in auth mode.
            match ureq::get(format!("{base}/tools")).call() {
                Err(ureq::Error::Io(_)) => std::thread::sleep(Duration::from_millis(20)),
                _ => break,
            }
        }
        base
    }

    /// Status and body of a response. 4xx/5xx come back as a status with an
    /// empty body, which is all the assertions need.
    fn http_result(
        sent: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> (u16, String) {
        match sent {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let mut out = String::new();
                resp.body_mut()
                    .as_reader()
                    .read_to_string(&mut out)
                    .unwrap();
                (status, out)
            }
            Err(ureq::Error::StatusCode(code)) => (code, String::new()),
            Err(e) => panic!("transport error: {e}"),
        }
    }

    /// Attach the bearer key, if the server is in auth mode.
    fn with_key(
        builder: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
        key: Option<&str>,
    ) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        match key {
            Some(k) => builder.header("Authorization", format!("Bearer {k}")),
            None => builder,
        }
    }

    fn http_get(url: &str, key: Option<&str>) -> (u16, String) {
        http_result(with_key(ureq::get(url), key).call())
    }

    fn http_delete(url: &str, key: Option<&str>) -> (u16, String) {
        http_result(with_key(ureq::delete(url), key).call())
    }

    fn http_post(url: &str, key: Option<&str>, body: &str) -> (u16, String) {
        let builder = ureq::post(url).header("Content-Type", "application/json");
        let builder = match key {
            Some(k) => builder.header("Authorization", format!("Bearer {k}")),
            None => builder,
        };
        http_result(builder.send(body))
    }

    /// Create a session over HTTP and return its id.
    fn http_create_session(base: &str, key: Option<&str>) -> String {
        let (status, body) = http_post(&format!("{base}/sessions"), key, "");
        assert_eq!(status, 200, "session creation failed: {body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        v["session_id"].as_str().unwrap().to_string()
    }

    /// The work dir `SessionManager` derives from a session id. Tests driving
    /// the server over HTTP hold no handle to the `Session` itself, and the
    /// file API takes text, not a WASM binary.
    fn session_work_dir(id: &str) -> PathBuf {
        session::instance_root().join(format!("session-{id}"))
    }

    /// Poll the JSON metrics until `pred` holds. `false` if it never does —
    /// which is also what a blocked accept loop looks like, since the scrape
    /// itself would be queued behind the running exec.
    fn wait_for_metric(
        base: &str,
        key: Option<&str>,
        pred: impl Fn(&serde_json::Value) -> bool,
    ) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let (status, body) = http_get(&format!("{base}/metrics?format=json"), key);
            if status == 200 {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                    if pred(&v) {
                        return true;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Body for an exec that runs until its own wall-clock timeout stops it.
    const SLOW_EXEC: &str = r#"{"wasm_path": "loop.wasm", "timeout": 3}"#;

    /// Give a session a WASM module that never returns, for tests that need an
    /// exec to still be running while other requests are made.
    fn plant_slow_exec(id: &str) {
        std::fs::write(session_work_dir(id).join("loop.wasm"), infinite_loop_wasm()).unwrap();
    }

    // ── Concurrent request handling over HTTP (0.22.7) ────────────

    #[test]
    fn test_long_exec_does_not_block_other_requests_over_http() {
        let base = serve(open_server(free_port()));
        let id = http_create_session(&base, None);
        plant_slow_exec(&id);

        let exec_url = format!("{base}/sessions/{id}/exec");
        let runner = std::thread::spawn(move || http_post(&exec_url, None, SLOW_EXEC).0);
        assert!(
            wait_for_metric(&base, None, |v| v["exec_in_flight"] == 1),
            "the exec never showed up in flight"
        );

        // The serial accept loop this replaces would queue this GET behind the
        // three-second exec.
        let start = Instant::now();
        let (status, _) = http_get(&format!("{base}/sessions"), None);
        assert_eq!(status, 200);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "a plain GET waited on the running exec ({:?})",
            start.elapsed()
        );

        assert_eq!(runner.join().unwrap(), 200);
        http_delete(&format!("{base}/sessions/{id}"), None);
    }

    #[test]
    fn test_global_exec_cap_binds_over_http() {
        // One exec allowed server-wide; the cap is only meaningful once real
        // requests can overlap, which is what 0.22.7 makes true.
        let base = serve(open_server_with_concurrency(free_port(), 1));
        let busy = http_create_session(&base, None);
        let other = http_create_session(&base, None);
        plant_slow_exec(&busy);
        std::fs::write(session_work_dir(&other).join("hello.wasm"), hello_wasm()).unwrap();

        let exec_url = format!("{base}/sessions/{busy}/exec");
        let runner = std::thread::spawn(move || http_post(&exec_url, None, SLOW_EXEC).0);
        assert!(
            wait_for_metric(&base, None, |v| v["exec_in_flight"] == 1),
            "the exec never showed up in flight"
        );

        // A second session's exec is refused while the slot is held.
        let quick = format!("{base}/sessions/{other}/exec");
        let (status, _) = http_post(&quick, None, r#"{"wasm_path": "hello.wasm"}"#);
        assert_eq!(status, 429, "the global exec cap did not bind over HTTP");

        // Once the slot is released the same request succeeds.
        assert_eq!(runner.join().unwrap(), 200);
        let (status, body) = http_post(&quick, None, r#"{"wasm_path": "hello.wasm"}"#);
        assert_eq!(status, 200);
        assert!(body.contains("Hello, World!"));

        http_delete(&format!("{base}/sessions/{busy}"), None);
        http_delete(&format!("{base}/sessions/{other}"), None);
    }

    #[test]
    fn test_tenant_exec_cap_binds_over_http() {
        use crate::agent::auth::hash_key;
        let toml = format!(
            "[[tenants]]\nid = \"alice\"\nkey_sha256 = \"{}\"\n[tenants.rate]\nmax_concurrent_exec = 1\n\n[[tenants]]\nid = \"bob\"\nkey_sha256 = \"{}\"\n",
            hash_key("k_alice"),
            hash_key("k_bob"),
        );
        let base = serve(auth_server_from_toml_on_port(free_port(), &toml));
        let (alice, bob) = (Some("k_alice"), Some("k_bob"));

        let busy = http_create_session(&base, alice);
        let spare = http_create_session(&base, alice);
        let bobs = http_create_session(&base, bob);
        plant_slow_exec(&busy);
        std::fs::write(session_work_dir(&bobs).join("hello.wasm"), hello_wasm()).unwrap();

        let exec_url = format!("{base}/sessions/{busy}/exec");
        let runner = std::thread::spawn(move || http_post(&exec_url, alice, SLOW_EXEC).0);
        assert!(
            wait_for_metric(&base, alice, |v| v["exec_in_flight"] == 1),
            "the exec never showed up in flight"
        );

        // alice is at her per-tenant ceiling of one...
        let (status, _) = http_post(
            &format!("{base}/sessions/{spare}/exec"),
            alice,
            r#"{"wasm_path": "loop.wasm", "timeout": 1}"#,
        );
        assert_eq!(
            status, 429,
            "the per-tenant exec cap did not bind over HTTP"
        );

        // ...while bob, uncapped, runs unimpeded alongside her.
        let (status, body) = http_post(
            &format!("{base}/sessions/{bobs}/exec"),
            bob,
            r#"{"wasm_path": "hello.wasm"}"#,
        );
        assert_eq!(status, 200);
        assert!(body.contains("Hello, World!"));

        assert_eq!(runner.join().unwrap(), 200);
        for (id, key) in [(&busy, alice), (&spare, alice), (&bobs, bob)] {
            http_delete(&format!("{base}/sessions/{id}"), key);
        }
    }

    // ── Deployment posture (0.22.8) ───────────────────────────────

    #[test]
    fn test_validate_bind_allows_loopback_spellings() {
        for host in ["127.0.0.1", "localhost", "LOCALHOST", "::1", "[::1]"] {
            assert!(
                validate_bind(host, false, false).is_ok(),
                "{host} should be treated as loopback"
            );
        }
    }

    #[test]
    fn test_validate_bind_refuses_open_non_loopback() {
        for host in ["0.0.0.0", "::", "192.168.1.10", "example.internal"] {
            let err = validate_bind(host, false, false).unwrap_err().to_string();
            assert!(
                err.contains("refusing to bind") && err.contains("--auth"),
                "{host} was allowed, or the error does not say how to fix it: {err}"
            );
        }
    }

    #[test]
    fn test_validate_bind_accepts_auth_or_insecure() {
        assert!(validate_bind("0.0.0.0", true, false).is_ok());
        assert!(validate_bind("0.0.0.0", false, true).is_ok());
    }

    #[test]
    fn test_banner_warns_only_when_exposed() {
        let mut config = AgentConfig {
            host: "0.0.0.0".to_string(),
            insecure: true,
            ..Default::default()
        };
        let exposed = AgentServer::new(config.clone()).banner(8);
        assert!(exposed.contains("AUTH IS DISABLED"), "{exposed}");
        assert!(exposed.contains("Terminate TLS"), "{exposed}");

        config.host = DEFAULT_HOST.to_string();
        let local = AgentServer::new(config).banner(8);
        assert!(!local.contains("AUTH IS DISABLED"), "{local}");
        assert!(local.contains("loopback only"), "{local}");
    }

    #[test]
    fn test_probes_answer_without_a_key() {
        let base = serve(auth_server(free_port(), &[("k_alice", "alice")]));

        // /metrics is auth-gated, which is why it cannot double as a probe.
        assert_eq!(http_get(&format!("{base}/metrics"), None).0, 401);

        let (status, body) = http_get(&format!("{base}/health"), None);
        assert_eq!(status, 200, "/health demanded a key: {body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
        assert!(v["uptime_seconds"].is_number());

        let (status, body) = http_get(&format!("{base}/ready"), None);
        assert_eq!(status, 200, "/ready demanded a key: {body}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap()["status"],
            "ready"
        );
    }

    #[test]
    fn test_ready_turns_503_at_session_capacity() {
        let mut config = AgentConfig {
            port: free_port(),
            ..Default::default()
        };
        config.session_config.max_sessions = 1;
        let server = AgentServer::new(config);
        assert_eq!(server.readiness(), (true, "ok"));

        let id = server.handle_create_session().unwrap().session_id;
        assert_eq!(server.readiness(), (false, "at_session_capacity"));

        server.handle_delete_session(&id, None).unwrap();
        assert_eq!(server.readiness(), (true, "ok"));
    }

    #[test]
    fn test_ready_turns_503_once_shutdown_starts() {
        let server = test_server();
        assert_eq!(server.readiness(), (true, "ok"));
        server.shutdown.store(true, Ordering::Relaxed);
        assert_eq!(server.readiness(), (false, "shutting_down"));
    }

    #[test]
    fn test_ready_turns_503_when_exec_slots_are_full() {
        let server = test_server_with_concurrency(1);
        let _permit = server.exec_slots.try_acquire().unwrap();
        assert_eq!(server.readiness(), (false, "at_exec_capacity"));
    }

    // ── Lifecycle hygiene (0.22.9) ────────────────────────────────

    #[test]
    fn test_orphan_grace_scales_with_the_cleanup_interval_but_has_a_floor() {
        let with_interval = |secs: u64| {
            let mut config = AgentConfig::default();
            config.session_config.cleanup_interval = Duration::from_secs(secs);
            AgentServer::new(config).orphan_grace()
        };
        // A long interval means fewer heartbeats, so the window has to grow.
        assert_eq!(with_interval(120), Duration::from_secs(1200));
        // A short one must not shrink it: the floor keeps a briefly stalled
        // server from being swept by one that starts up next to it.
        assert_eq!(with_interval(1), MIN_ORPHAN_GRACE);
        assert_eq!(with_interval(30), MIN_ORPHAN_GRACE);
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
