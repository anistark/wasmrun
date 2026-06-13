//! Agent mode: Session lifecycle management.
//!
//! Each session represents an isolated WASM sandbox with its own:
//! - WASI filesystem (isolated temp directory with preopen)
//! - WasiEnv (independent stdout/stderr buffers, args, env vars)
//! - Timeout tracking (auto-cleanup on idle expiry)

use crate::agent::limits::ResourceLimits;
use crate::runtime::wasi::WasiEnv;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ── Session ID generation ─────────────────────────────────────────────

/// Generate a random hex session ID (32 chars = 16 bytes).
///
/// Uses system time nanos + counter for entropy, mixed via xorshift64.
/// Not cryptographically secure — sufficient for session identifiers.
fn generate_session_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let time_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    // Mix time and counter via xorshift64
    let mut state = time_nanos ^ (count.wrapping_mul(0x9E3779B97F4A7C15));
    let mut bytes = [0u8; 16];
    for b in &mut bytes {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state & 0xFF) as u8;
    }

    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── Session state ─────────────────────────────────────────────────────

/// Current lifecycle state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is active and accepting commands.
    Active,
    /// Session has expired due to idle timeout.
    Expired,
}

// ── Session ───────────────────────────────────────────────────────────

/// An isolated WASM sandbox session.
///
/// Each session owns a temporary directory on the host filesystem,
/// a WASI environment with independent I/O buffers, and timeout tracking.
pub struct Session {
    /// Unique session identifier (32-char hex string).
    id: String,
    /// When the session was created.
    created_at: Instant,
    /// When the session was last accessed (updated on every operation).
    last_accessed: Mutex<Instant>,
    /// Idle timeout duration — session expires after this much inactivity.
    timeout: Duration,
    /// Current session state.
    state: Mutex<SessionState>,
    /// WASI environment with stdout/stderr buffers, args, env vars, fd table.
    wasi_env: Arc<Mutex<WasiEnv>>,
    /// Isolated working directory (temp dir on host filesystem).
    work_dir: PathBuf,
    /// Whether we own the work_dir and should delete it on drop.
    owns_work_dir: bool,
    /// Resource ceilings applied to executions in this session.
    limits: ResourceLimits,
    /// Tenant that owns this session (the authenticated caller at creation).
    /// `None` in open mode (no auth config). Cross-tenant access is rejected
    /// as `NotFound` to hide the session's existence.
    owner: Option<String>,
}

impl Session {
    /// Create a new session with an isolated temp directory.
    ///
    /// The temp directory is preopened at `/` in the WASI environment,
    /// giving the sandboxed code access to a clean, isolated filesystem.
    /// `limits` configure the session's WASI output/file-size caps.
    /// `owner` is the authenticated tenant id (`None` in open mode).
    pub fn new(
        timeout: Duration,
        limits: ResourceLimits,
        owner: Option<String>,
    ) -> Result<Self, SessionError> {
        let id = generate_session_id();
        let work_dir = std::env::temp_dir().join(format!("wasmrun-session-{id}"));

        std::fs::create_dir_all(&work_dir).map_err(|e| SessionError::IoError {
            message: format!("Failed to create session directory: {e}"),
        })?;

        let wasi_env = Self::build_wasi_env(&work_dir, &limits);

        Ok(Session {
            id,
            created_at: Instant::now(),
            last_accessed: Mutex::new(Instant::now()),
            timeout,
            state: Mutex::new(SessionState::Active),
            wasi_env: Arc::new(Mutex::new(wasi_env)),
            work_dir,
            owns_work_dir: true,
            limits,
            owner,
        })
    }

    /// Build a WASI environment for the session, preopened at `/` and
    /// configured with the session's output and per-file size caps.
    fn build_wasi_env(work_dir: &Path, limits: &ResourceLimits) -> WasiEnv {
        let mut wasi_env = WasiEnv::new().with_preopen("/", work_dir);
        wasi_env.set_max_output_bytes(limits.max_output_bytes);
        wasi_env.set_max_file_size(limits.max_file_size);
        wasi_env
    }

    /// Create a session with a specific work directory (for testing).
    #[cfg(test)]
    fn with_work_dir(timeout: Duration, work_dir: PathBuf) -> Result<Self, SessionError> {
        Self::with_work_dir_and_limits(timeout, work_dir, ResourceLimits::default())
    }

    /// Create a session with a specific work directory and limits (for testing).
    #[cfg(test)]
    fn with_work_dir_and_limits(
        timeout: Duration,
        work_dir: PathBuf,
        limits: ResourceLimits,
    ) -> Result<Self, SessionError> {
        let id = generate_session_id();

        std::fs::create_dir_all(&work_dir).map_err(|e| SessionError::IoError {
            message: format!("Failed to create session directory: {e}"),
        })?;

        let wasi_env = Self::build_wasi_env(&work_dir, &limits);

        Ok(Session {
            id,
            created_at: Instant::now(),
            last_accessed: Mutex::new(Instant::now()),
            timeout,
            state: Mutex::new(SessionState::Active),
            wasi_env: Arc::new(Mutex::new(wasi_env)),
            work_dir,
            owns_work_dir: false, // test manages cleanup
            limits,
            owner: None,
        })
    }

    /// Session ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// When the session was created.
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// When the session was last accessed.
    pub fn last_accessed(&self) -> Instant {
        *self.last_accessed.lock().unwrap()
    }

    /// Configured idle timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Current session state.
    pub fn state(&self) -> SessionState {
        *self.state.lock().unwrap()
    }

    /// Path to the session's isolated working directory.
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    /// Resource ceilings applied to executions in this session.
    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Tenant that owns this session, or `None` in open mode.
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    /// Get a clone of the WASI environment Arc for use with the executor.
    pub fn wasi_env(&self) -> Arc<Mutex<WasiEnv>> {
        self.wasi_env.clone()
    }

    /// Record an access — resets the idle timeout clock.
    pub fn touch(&self) {
        *self.last_accessed.lock().unwrap() = Instant::now();
    }

    /// Check if the session has expired due to inactivity.
    pub fn is_expired(&self) -> bool {
        let state = self.state.lock().unwrap();
        if *state == SessionState::Expired {
            return true;
        }
        self.last_accessed().elapsed() > self.timeout
    }

    /// Mark the session as expired.
    pub fn mark_expired(&self) {
        *self.state.lock().unwrap() = SessionState::Expired;
    }

    /// Set environment variables on this session's WASI environment.
    pub fn set_env(&self, key: &str, value: &str) {
        if let Ok(mut env) = self.wasi_env.lock() {
            env.add_env(key.to_string(), value.to_string());
        }
        self.touch();
    }

    /// Get captured stdout from the session's WASI environment.
    #[allow(dead_code)] // TODO: Used by agent tools/shell
    pub fn get_stdout(&self) -> Vec<u8> {
        self.wasi_env
            .lock()
            .map(|e| e.get_stdout())
            .unwrap_or_default()
    }

    /// Get captured stderr from the session's WASI environment.
    #[allow(dead_code)] // TODO: Used by agent tools/shell
    pub fn get_stderr(&self) -> Vec<u8> {
        self.wasi_env
            .lock()
            .map(|e| e.get_stderr())
            .unwrap_or_default()
    }

    /// Clear captured stdout/stderr buffers (e.g., between exec calls).
    #[allow(dead_code)] // TODO: Used by agent tools/shell
    pub fn clear_output(&self) {
        if let Ok(mut env) = self.wasi_env.lock() {
            env.clear_stdout();
            env.clear_stderr();
        }
    }

    /// Clean up session resources (delete work directory).
    fn cleanup(&self) {
        if self.owns_work_dir && self.work_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.work_dir);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cleanup();
    }
}

// ── Session Manager ───────────────────────────────────────────────────

/// Configuration for the SessionManager.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Default idle timeout for new sessions.
    pub default_timeout: Duration,
    /// Maximum number of concurrent sessions.
    pub max_sessions: usize,
    /// How often the cleanup thread checks for expired sessions.
    pub cleanup_interval: Duration,
    /// Default resource ceilings applied to each new session.
    pub limits: ResourceLimits,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            default_timeout: Duration::from_secs(300), // 5 minutes
            max_sessions: 100,
            cleanup_interval: Duration::from_secs(30),
            limits: ResourceLimits::default(),
        }
    }
}

/// Manages the lifecycle of agent sessions.
///
/// Thread-safe: all operations are behind a RwLock.
/// Optionally runs a background cleanup thread for expired sessions.
pub struct SessionManager {
    sessions: RwLock<HashMap<String, Session>>,
    config: SessionConfig,
    /// Flag to signal the cleanup thread to stop.
    cleanup_stop: Arc<Mutex<bool>>,
}

impl SessionManager {
    /// Create a new SessionManager with default configuration.
    pub fn new() -> Self {
        Self::with_config(SessionConfig::default())
    }

    /// Create a new SessionManager with custom configuration.
    pub fn with_config(config: SessionConfig) -> Self {
        SessionManager {
            sessions: RwLock::new(HashMap::new()),
            config,
            cleanup_stop: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a new session with the default timeout and default limits.
    ///
    /// Returns the session ID on success.
    #[allow(dead_code)] // Used by tests; HTTP path uses create_session_with_limits.
    pub fn create_session(&self) -> Result<String, SessionError> {
        self.create_session_with_limits(
            self.config.default_timeout,
            self.config.limits.clone(),
            None,
        )
    }

    /// Create a new session with a custom timeout and the default limits.
    ///
    /// Returns the session ID on success.
    #[allow(dead_code)] // Used by tests; HTTP path uses create_session_with_limits.
    pub fn create_session_with_timeout(&self, timeout: Duration) -> Result<String, SessionError> {
        self.create_session_with_limits(timeout, self.config.limits.clone(), None)
    }

    /// Create a new session with a custom timeout, resource limits, and owner.
    ///
    /// `owner` is the authenticated tenant id, or `None` in open mode.
    /// Returns the session ID on success.
    pub fn create_session_with_limits(
        &self,
        timeout: Duration,
        limits: ResourceLimits,
        owner: Option<String>,
    ) -> Result<String, SessionError> {
        let mut sessions = self.sessions.write().map_err(|_| SessionError::LockError)?;

        // Enforce max sessions limit (after removing expired ones)
        let active_count = sessions.values().filter(|s| !s.is_expired()).count();
        if active_count >= self.config.max_sessions {
            return Err(SessionError::MaxSessionsReached {
                max: self.config.max_sessions,
            });
        }

        let session = Session::new(timeout, limits, owner)?;
        let id = session.id().to_string();
        sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Get a session by ID. Returns an error if not found, not owned by the
    /// caller, or expired.
    ///
    /// `caller` is the authenticated tenant id (`None` in open mode). A session
    /// whose owner does not match `caller` is reported as `NotFound` to hide its
    /// existence from other tenants. One equality covers all cases: open mode
    /// (`None == None`), same tenant (`Some(x) == Some(x)`), cross tenant
    /// (`Some(x) != Some(y)` → `NotFound`).
    ///
    /// Touches the session (resets idle timeout) on success.
    pub fn get_session<F, R>(&self, id: &str, caller: Option<&str>, f: F) -> Result<R, SessionError>
    where
        F: FnOnce(&Session) -> R,
    {
        let sessions = self.sessions.read().map_err(|_| SessionError::LockError)?;
        let session = sessions
            .get(id)
            .filter(|s| s.owner() == caller)
            .ok_or_else(|| SessionError::NotFound { id: id.to_string() })?;

        if session.is_expired() {
            return Err(SessionError::Expired { id: id.to_string() });
        }

        session.touch();
        Ok(f(session))
    }

    /// Destroy a session by ID. Cleans up resources.
    ///
    /// `caller` is the authenticated tenant id (`None` in open mode); a session
    /// owned by another tenant is reported as `NotFound`. Returns `Ok(())` if the
    /// session was found, owned by the caller, and destroyed.
    pub fn destroy_session(&self, id: &str, caller: Option<&str>) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().map_err(|_| SessionError::LockError)?;
        match sessions.get(id) {
            Some(s) if s.owner() == caller => {
                sessions.remove(id); // Session::drop handles cleanup
                Ok(())
            }
            // Absent or owned by another tenant → NotFound (hides existence).
            _ => Err(SessionError::NotFound { id: id.to_string() }),
        }
    }

    /// List all active (non-expired) session IDs.
    #[allow(dead_code)] // TODO: Used by agent list sessions endpoint
    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>, SessionError> {
        let sessions = self.sessions.read().map_err(|_| SessionError::LockError)?;
        Ok(sessions
            .values()
            .filter(|s| !s.is_expired())
            .map(|s| SessionInfo {
                id: s.id().to_string(),
                state: s.state(),
                created_at_elapsed: s.created_at().elapsed(),
                last_accessed_elapsed: s.last_accessed().elapsed(),
                timeout: s.timeout(),
            })
            .collect())
    }

    /// Remove all expired sessions and clean up their resources.
    ///
    /// Returns the number of sessions removed.
    pub fn cleanup_expired(&self) -> Result<usize, SessionError> {
        let mut sessions = self.sessions.write().map_err(|_| SessionError::LockError)?;
        let before = sessions.len();

        // Mark expired sessions
        for session in sessions.values() {
            if session.last_accessed().elapsed() > session.timeout() {
                session.mark_expired();
            }
        }

        // Remove expired sessions (Drop handles cleanup)
        sessions.retain(|_, s| !s.is_expired());
        Ok(before - sessions.len())
    }

    /// Get the number of active (non-expired) sessions.
    #[allow(dead_code)] // TODO: Used by agent metrics
    pub fn active_count(&self) -> usize {
        self.sessions
            .read()
            .map(|s| s.values().filter(|s| !s.is_expired()).count())
            .unwrap_or(0)
    }

    /// Get the total number of sessions (including expired ones not yet cleaned up).
    pub fn total_count(&self) -> usize {
        self.sessions.read().map(|s| s.len()).unwrap_or(0)
    }

    /// Per-session resource report (id, disk footprint, configured memory cap)
    /// for active sessions. Used by the JSON metrics breakdown.
    ///
    /// The work dirs are collected under the read lock and the (potentially
    /// slow) filesystem walks run *after* the lock is released, so a metrics
    /// scrape never blocks session creation/access while walking the FS.
    pub fn session_reports(&self) -> Vec<SessionReport> {
        let dirs: Vec<(String, std::path::PathBuf, Option<u32>)> = match self.sessions.read() {
            Ok(sessions) => sessions
                .values()
                .filter(|s| !s.is_expired())
                .map(|s| {
                    (
                        s.id().to_string(),
                        s.work_dir().to_path_buf(),
                        s.limits().max_memory_pages,
                    )
                })
                .collect(),
            Err(_) => return Vec::new(),
        };
        dirs.into_iter()
            .map(|(id, dir, memory_cap_pages)| SessionReport {
                id,
                disk_bytes: crate::agent::limits::dir_size(&dir),
                memory_cap_pages,
            })
            .collect()
    }

    /// Total on-disk footprint across active sessions, in bytes. Sums the
    /// per-session reports; see [`session_reports`](Self::session_reports) for
    /// the locking strategy.
    pub fn total_disk_bytes(&self) -> u64 {
        self.session_reports().iter().map(|r| r.disk_bytes).sum()
    }

    /// Start the background cleanup thread.
    ///
    /// The thread periodically removes expired sessions.
    /// Call `stop_cleanup` to stop it.
    pub fn start_cleanup_thread(manager: Arc<Self>) -> std::thread::JoinHandle<()> {
        let interval = manager.config.cleanup_interval;
        let stop_flag = manager.cleanup_stop.clone();

        std::thread::spawn(move || loop {
            std::thread::sleep(interval);

            if let Ok(stop) = stop_flag.lock() {
                if *stop {
                    break;
                }
            }

            let _ = manager.cleanup_expired();
        })
    }

    /// Signal the cleanup thread to stop.
    pub fn stop_cleanup(&self) {
        if let Ok(mut stop) = self.cleanup_stop.lock() {
            *stop = true;
        }
    }

    /// Destroy all sessions and clean up resources.
    pub fn destroy_all(&self) -> Result<usize, SessionError> {
        let mut sessions = self.sessions.write().map_err(|_| SessionError::LockError)?;
        let count = sessions.len();
        sessions.clear(); // Drop handles cleanup for each session
        Ok(count)
    }

    /// Get session configuration.
    #[allow(dead_code)] // TODO: Used by agent status endpoint
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Session Info (for listing) ────────────────────────────────────────

/// Summary information about a session (safe to serialize/return via API).
#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO: Used by agent list sessions endpoint
pub struct SessionInfo {
    pub id: String,
    pub state: SessionState,
    pub created_at_elapsed: Duration,
    pub last_accessed_elapsed: Duration,
    pub timeout: Duration,
}

/// Per-session resource footprint, produced by
/// [`SessionManager::session_reports`] for the metrics endpoint.
#[derive(Debug, Clone)]
pub struct SessionReport {
    pub id: String,
    pub disk_bytes: u64,
    pub memory_cap_pages: Option<u32>,
}

// ── Errors ────────────────────────────────────────────────────────────

/// Errors specific to session management.
#[derive(Debug)]
pub enum SessionError {
    /// Session not found.
    NotFound { id: String },
    /// Session has expired.
    Expired { id: String },
    /// Maximum concurrent sessions reached.
    MaxSessionsReached { max: usize },
    /// I/O error during session setup/cleanup.
    IoError { message: String },
    /// Internal lock poisoned.
    LockError,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotFound { id } => write!(f, "Session not found: {id}"),
            SessionError::Expired { id } => write!(f, "Session expired: {id}"),
            SessionError::MaxSessionsReached { max } => {
                write!(f, "Maximum concurrent sessions reached: {max}")
            }
            SessionError::IoError { message } => write!(f, "Session I/O error: {message}"),
            SessionError::LockError => write!(f, "Internal lock error"),
        }
    }
}

impl std::error::Error for SessionError {}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SessionConfig {
        SessionConfig {
            default_timeout: Duration::from_secs(60),
            max_sessions: 10,
            cleanup_interval: Duration::from_millis(100),
            limits: ResourceLimits::default(),
        }
    }

    // ── Session ID generation ─────────────────────────────────────

    #[test]
    fn test_session_id_is_32_hex_chars() {
        let id = generate_session_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_session_ids_are_unique() {
        let ids: Vec<String> = (0..100).map(|_| generate_session_id()).collect();
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }

    // ── Session creation ──────────────────────────────────────────

    #[test]
    fn test_session_create() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_secs(60), tmp.path().join("sess")).unwrap();

        assert_eq!(session.id().len(), 32);
        assert_eq!(session.state(), SessionState::Active);
        assert!(!session.is_expired());
        assert!(session.work_dir().exists());
    }

    #[test]
    fn test_session_isolation_has_own_wasi_env() {
        let tmp = tempfile::tempdir().unwrap();
        let s1 = Session::with_work_dir(Duration::from_secs(60), tmp.path().join("s1")).unwrap();
        let s2 = Session::with_work_dir(Duration::from_secs(60), tmp.path().join("s2")).unwrap();

        // Write to s1's stdout, s2 should be unaffected
        {
            let env1 = s1.wasi_env();
            env1.lock().unwrap().write_stdout(b"hello from s1");
        }

        assert_eq!(s1.get_stdout(), b"hello from s1");
        assert!(s2.get_stdout().is_empty());
    }

    #[test]
    fn test_session_isolation_has_own_filesystem() {
        let tmp = tempfile::tempdir().unwrap();
        let s1 = Session::with_work_dir(Duration::from_secs(60), tmp.path().join("s1")).unwrap();
        let s2 = Session::with_work_dir(Duration::from_secs(60), tmp.path().join("s2")).unwrap();

        // Write file in s1's work dir
        std::fs::write(s1.work_dir().join("test.txt"), b"s1 data").unwrap();

        assert!(s1.work_dir().join("test.txt").exists());
        assert!(!s2.work_dir().join("test.txt").exists());
    }

    #[test]
    fn test_session_set_env() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_secs(60), tmp.path().join("sess")).unwrap();

        session.set_env("MY_KEY", "my_value");

        let env = session.wasi_env();
        let locked = env.lock().unwrap();
        let vars = locked.env_vars();
        assert!(vars.iter().any(|(k, v)| k == "MY_KEY" && v == "my_value"));
    }

    #[test]
    fn test_session_clear_output() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_secs(60), tmp.path().join("sess")).unwrap();

        // Write some output
        {
            let env = session.wasi_env();
            let mut locked = env.lock().unwrap();
            locked.write_stdout(b"output");
            locked.write_stderr(b"error");
        }

        assert!(!session.get_stdout().is_empty());
        assert!(!session.get_stderr().is_empty());

        session.clear_output();

        assert!(session.get_stdout().is_empty());
        assert!(session.get_stderr().is_empty());
    }

    // ── Session timeout ───────────────────────────────────────────

    #[test]
    fn test_session_not_expired_when_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_secs(60), tmp.path().join("sess")).unwrap();
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_expires_after_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_millis(50), tmp.path().join("sess")).unwrap();

        assert!(!session.is_expired());
        std::thread::sleep(Duration::from_millis(80));
        assert!(session.is_expired());
    }

    #[test]
    fn test_session_touch_resets_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_millis(100), tmp.path().join("sess")).unwrap();

        std::thread::sleep(Duration::from_millis(60));
        assert!(!session.is_expired());

        session.touch(); // reset

        std::thread::sleep(Duration::from_millis(60));
        assert!(!session.is_expired()); // still alive because we touched

        std::thread::sleep(Duration::from_millis(60));
        assert!(session.is_expired()); // now expired
    }

    #[test]
    fn test_session_mark_expired() {
        let tmp = tempfile::tempdir().unwrap();
        let session =
            Session::with_work_dir(Duration::from_secs(60), tmp.path().join("sess")).unwrap();

        assert!(!session.is_expired());
        session.mark_expired();
        assert!(session.is_expired());
        assert_eq!(session.state(), SessionState::Expired);
    }

    // ── SessionManager creation ───────────────────────────────────

    #[test]
    fn test_manager_create_session() {
        let manager = SessionManager::with_config(test_config());
        let id = manager.create_session().unwrap();

        assert_eq!(id.len(), 32);
        assert_eq!(manager.active_count(), 1);
        assert_eq!(manager.total_count(), 1);

        // Cleanup
        manager.destroy_session(&id, None).unwrap();
    }

    #[test]
    fn test_manager_create_multiple_sessions() {
        let manager = SessionManager::with_config(test_config());
        let mut ids = Vec::new();

        for _ in 0..5 {
            ids.push(manager.create_session().unwrap());
        }

        assert_eq!(manager.active_count(), 5);

        // All IDs are unique
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), 5);

        // Cleanup
        for id in &ids {
            manager.destroy_session(id, None).unwrap();
        }
    }

    #[test]
    fn test_manager_get_session() {
        let manager = SessionManager::with_config(test_config());
        let id = manager.create_session().unwrap();

        let state = manager.get_session(&id, None, |s| s.state()).unwrap();
        assert_eq!(state, SessionState::Active);

        // Cleanup
        manager.destroy_session(&id, None).unwrap();
    }

    #[test]
    fn test_manager_get_nonexistent_session() {
        let manager = SessionManager::with_config(test_config());
        let result = manager.get_session("nonexistent", None, |s| s.state());

        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::NotFound { id } => assert_eq!(id, "nonexistent"),
            other => panic!("Expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn test_manager_destroy_session() {
        let manager = SessionManager::with_config(test_config());
        let id = manager.create_session().unwrap();

        assert_eq!(manager.active_count(), 1);
        manager.destroy_session(&id, None).unwrap();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_manager_destroy_nonexistent_session() {
        let manager = SessionManager::with_config(test_config());
        let result = manager.destroy_session("nonexistent", None);

        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::NotFound { id } => assert_eq!(id, "nonexistent"),
            other => panic!("Expected NotFound, got: {other}"),
        }
    }

    // ── SessionManager limits ─────────────────────────────────────

    #[test]
    fn test_manager_max_sessions_enforced() {
        let config = SessionConfig {
            max_sessions: 3,
            ..test_config()
        };
        let manager = SessionManager::with_config(config);
        let mut ids = Vec::new();

        for _ in 0..3 {
            ids.push(manager.create_session().unwrap());
        }

        // 4th session should fail
        let result = manager.create_session();
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::MaxSessionsReached { max } => assert_eq!(max, 3),
            other => panic!("Expected MaxSessionsReached, got: {other}"),
        }

        // Cleanup
        for id in &ids {
            manager.destroy_session(id, None).unwrap();
        }
    }

    #[test]
    fn test_manager_max_sessions_allows_after_destroy() {
        let config = SessionConfig {
            max_sessions: 2,
            ..test_config()
        };
        let manager = SessionManager::with_config(config);

        let id1 = manager.create_session().unwrap();
        let id2 = manager.create_session().unwrap();
        assert!(manager.create_session().is_err()); // at limit

        manager.destroy_session(&id1, None).unwrap();
        let id3 = manager.create_session().unwrap(); // should succeed now

        assert_eq!(manager.active_count(), 2);

        // Cleanup
        manager.destroy_session(&id2, None).unwrap();
        manager.destroy_session(&id3, None).unwrap();
    }

    // ── SessionManager listing ────────────────────────────────────

    #[test]
    fn test_manager_list_sessions() {
        let manager = SessionManager::with_config(test_config());
        let id1 = manager.create_session().unwrap();
        let id2 = manager.create_session().unwrap();

        let list = manager.list_sessions().unwrap();
        assert_eq!(list.len(), 2);

        let listed_ids: Vec<&str> = list.iter().map(|i| i.id.as_str()).collect();
        assert!(listed_ids.contains(&id1.as_str()));
        assert!(listed_ids.contains(&id2.as_str()));

        // All are active
        assert!(list.iter().all(|i| i.state == SessionState::Active));

        // Cleanup
        manager.destroy_session(&id1, None).unwrap();
        manager.destroy_session(&id2, None).unwrap();
    }

    #[test]
    fn test_manager_list_excludes_expired() {
        let config = SessionConfig {
            default_timeout: Duration::from_millis(50),
            ..test_config()
        };
        let manager = SessionManager::with_config(config);

        let _id1 = manager.create_session().unwrap();
        std::thread::sleep(Duration::from_millis(80)); // let it expire

        let id2 = manager
            .create_session_with_timeout(Duration::from_secs(60))
            .unwrap();

        let list = manager.list_sessions().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id2);

        // Cleanup
        manager.destroy_all().unwrap();
    }

    // ── SessionManager cleanup ────────────────────────────────────

    #[test]
    fn test_manager_cleanup_expired() {
        let config = SessionConfig {
            default_timeout: Duration::from_millis(50),
            ..test_config()
        };
        let manager = SessionManager::with_config(config);

        let _id1 = manager.create_session().unwrap();
        let _id2 = manager.create_session().unwrap();

        assert_eq!(manager.total_count(), 2);

        std::thread::sleep(Duration::from_millis(80));
        let removed = manager.cleanup_expired().unwrap();

        assert_eq!(removed, 2);
        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_manager_cleanup_only_expired() {
        let manager = SessionManager::with_config(test_config());

        let _short = manager
            .create_session_with_timeout(Duration::from_millis(50))
            .unwrap();
        let long_id = manager
            .create_session_with_timeout(Duration::from_secs(60))
            .unwrap();

        std::thread::sleep(Duration::from_millis(80));
        let removed = manager.cleanup_expired().unwrap();

        assert_eq!(removed, 1);
        assert_eq!(manager.active_count(), 1);

        // The long-lived session is still there
        let state = manager.get_session(&long_id, None, |s| s.state()).unwrap();
        assert_eq!(state, SessionState::Active);

        // Cleanup
        manager.destroy_all().unwrap();
    }

    #[test]
    fn test_manager_destroy_all() {
        let manager = SessionManager::with_config(test_config());

        for _ in 0..5 {
            manager.create_session().unwrap();
        }

        assert_eq!(manager.total_count(), 5);
        let removed = manager.destroy_all().unwrap();
        assert_eq!(removed, 5);
        assert_eq!(manager.total_count(), 0);
    }

    // ── SessionManager get_session touches the session ────────────

    #[test]
    fn test_manager_get_session_touches() {
        let config = SessionConfig {
            default_timeout: Duration::from_millis(150),
            ..test_config()
        };
        let manager = SessionManager::with_config(config);
        let id = manager.create_session().unwrap();

        // Wait 80ms, then access — should reset timeout
        std::thread::sleep(Duration::from_millis(80));
        manager.get_session(&id, None, |_| {}).unwrap(); // touch

        // Wait another 80ms — total 160ms from creation, but only 80ms from last access
        std::thread::sleep(Duration::from_millis(80));

        // Should still be alive because get_session touched it
        let result = manager.get_session(&id, None, |s| s.state());
        assert!(result.is_ok());

        // Cleanup
        manager.destroy_session(&id, None).unwrap();
    }

    #[test]
    fn test_manager_get_expired_session_returns_error() {
        let config = SessionConfig {
            default_timeout: Duration::from_millis(50),
            ..test_config()
        };
        let manager = SessionManager::with_config(config);
        let id = manager.create_session().unwrap();

        std::thread::sleep(Duration::from_millis(80));

        let result = manager.get_session(&id, None, |s| s.state());
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::Expired { .. } => {}
            other => panic!("Expected Expired, got: {other}"),
        }

        // Cleanup
        manager.destroy_all().unwrap();
    }

    // ── Concurrent session creation ───────────────────────────────

    #[test]
    fn test_concurrent_session_creation() {
        let config = SessionConfig {
            max_sessions: 100,
            ..test_config()
        };
        let manager = Arc::new(SessionManager::with_config(config));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let mgr = manager.clone();
            handles.push(std::thread::spawn(move || {
                let mut ids = Vec::new();
                for _ in 0..5 {
                    ids.push(mgr.create_session().unwrap());
                }
                ids
            }));
        }

        let all_ids: Vec<String> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // All IDs should be unique
        let unique: std::collections::HashSet<&String> = all_ids.iter().collect();
        assert_eq!(unique.len(), 50);
        assert_eq!(manager.active_count(), 50);

        // Cleanup
        manager.destroy_all().unwrap();
    }

    // ── Cleanup thread ────────────────────────────────────────────

    #[test]
    fn test_cleanup_thread() {
        let config = SessionConfig {
            default_timeout: Duration::from_millis(50),
            cleanup_interval: Duration::from_millis(50),
            max_sessions: 10,
            limits: ResourceLimits::default(),
        };
        let manager = Arc::new(SessionManager::with_config(config));

        let _id = manager.create_session().unwrap();
        assert_eq!(manager.total_count(), 1);

        let handle = SessionManager::start_cleanup_thread(manager.clone());

        // Wait for session to expire + cleanup to run
        std::thread::sleep(Duration::from_millis(200));

        assert_eq!(manager.total_count(), 0);

        manager.stop_cleanup();
        handle.join().unwrap();
    }

    // ── Session work dir cleanup ──────────────────────────────────

    #[test]
    fn test_session_cleanup_removes_work_dir() {
        let manager = SessionManager::with_config(test_config());
        let id = manager.create_session().unwrap();

        let work_dir = manager
            .get_session(&id, None, |s| s.work_dir().to_path_buf())
            .unwrap();

        assert!(work_dir.exists());
        manager.destroy_session(&id, None).unwrap();
        assert!(!work_dir.exists());
    }

    // ── SessionManager config ─────────────────────────────────────

    #[test]
    fn test_default_config() {
        let config = SessionConfig::default();
        assert_eq!(config.default_timeout, Duration::from_secs(300));
        assert_eq!(config.max_sessions, 100);
        assert_eq!(config.cleanup_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_manager_exposes_config() {
        let config = test_config();
        let manager = SessionManager::with_config(config.clone());
        assert_eq!(manager.config().default_timeout, config.default_timeout);
        assert_eq!(manager.config().max_sessions, config.max_sessions);
    }

    // ── Session per-session WASI env isolation ────────────────────

    #[test]
    fn test_sessions_have_independent_wasi_envs() {
        let manager = SessionManager::with_config(test_config());
        let id1 = manager.create_session().unwrap();
        let id2 = manager.create_session().unwrap();

        // Set env on session 1
        manager
            .get_session(&id1, None, |s| s.set_env("ONLY_S1", "yes"))
            .unwrap();

        // Session 2 should not have it
        let s2_vars = manager
            .get_session(&id2, None, |s| {
                let env = s.wasi_env();
                let locked = env.lock().unwrap();
                locked.env_vars().to_vec()
            })
            .unwrap();

        assert!(!s2_vars.iter().any(|(k, _)| k == "ONLY_S1"));

        // Session 1 should have it
        let s1_vars = manager
            .get_session(&id1, None, |s| {
                let env = s.wasi_env();
                let locked = env.lock().unwrap();
                locked.env_vars().to_vec()
            })
            .unwrap();

        assert!(s1_vars.iter().any(|(k, v)| k == "ONLY_S1" && v == "yes"));

        // Cleanup
        manager.destroy_all().unwrap();
    }

    // ── Tenant ownership ──────────────────────────────────────────

    #[test]
    fn test_owned_session_ownership_equality() {
        let manager = SessionManager::with_config(test_config());
        let id = manager
            .create_session_with_limits(
                Duration::from_secs(60),
                ResourceLimits::default(),
                Some("alice".to_string()),
            )
            .unwrap();

        // Owner is recorded on the session.
        let owner = manager
            .get_session(&id, Some("alice"), |s| s.owner().map(str::to_string))
            .unwrap();
        assert_eq!(owner, Some("alice".to_string()));

        // Cross tenant → NotFound (hides existence), not Expired.
        assert!(matches!(
            manager.get_session(&id, Some("bob"), |_| ()),
            Err(SessionError::NotFound { .. })
        ));
        // Open-mode caller (None) against an owned session → NotFound.
        assert!(matches!(
            manager.get_session(&id, None, |_| ()),
            Err(SessionError::NotFound { .. })
        ));

        // Cross-tenant destroy is a no-op reported as NotFound; session survives.
        assert!(matches!(
            manager.destroy_session(&id, Some("bob")),
            Err(SessionError::NotFound { .. })
        ));
        assert!(manager.get_session(&id, Some("alice"), |_| ()).is_ok());

        // Owner can destroy.
        assert!(manager.destroy_session(&id, Some("alice")).is_ok());
    }

    #[test]
    fn test_open_mode_session_has_no_owner() {
        let manager = SessionManager::with_config(test_config());
        let id = manager.create_session().unwrap(); // owner = None

        // Open-mode caller matches.
        assert!(manager.get_session(&id, None, |_| ()).is_ok());
        // A tenant cannot touch an unowned (open-mode) session.
        assert!(matches!(
            manager.get_session(&id, Some("alice"), |_| ()),
            Err(SessionError::NotFound { .. })
        ));
        manager.destroy_session(&id, None).unwrap();
    }
}
