//! Agent mode: long-lived servers inside a session.
//!
//! `POST /exec` runs to completion and answers with what the program printed.
//! A server never completes, so that shape cannot express one: by the time the
//! response carries the port, the thing listening on it is gone. This module
//! is the other lifecycle. A serve execution is started, keeps running in the
//! background, and is addressable until it is stopped or its session expires.
//!
//! **The port is bound here, not in the guest.** WASI Preview 1 has no call
//! that creates a socket, so the host binds one and hands it over as an fd, the
//! same way a preopened directory arrives. wasmhub's `net` and `http` read the
//! descriptor number out of `WASMHUB_LISTEN_FD` and the address out of
//! `WASMHUB_LISTEN_ADDR`, which is what makes `server.listen()` and
//! `server.address()` work without the guest ever asking for a port it might
//! not be allowed to have.
//!
//! **Loopback only.** The bound socket is reachable by anything that can reach
//! the host, and what serves on it is code the caller supplied. Exposing it
//! further is a reverse-proxy decision, the same one agent mode already makes
//! for its own listener.

use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::agent::api::ApiError;
use crate::agent::limits::ResourceLimits;
use crate::runtime::wasi::WasiEnv;

/// Environment variable naming the fd the listener arrived on.
const LISTEN_FD_VAR: &str = "WASMHUB_LISTEN_FD";
/// Environment variable naming the address it is bound to.
const LISTEN_ADDR_VAR: &str = "WASMHUB_LISTEN_ADDR";

/// A server running in the background of a session.
///
/// Dropping this stops the execution: the handle owns the cancellation flag,
/// and a session that goes away takes its server with it rather than leaking a
/// bound port and a thread for the lifetime of the process.
pub struct RunningServer {
    /// Identifier the caller uses to refer to this server.
    pub id: String,
    /// Where it is actually listening, after an ephemeral port is resolved.
    pub addr: SocketAddr,
    /// When it started, for the status endpoint's uptime.
    pub started_at: Instant,
    /// The serve execution's own output buffers, kept apart from the
    /// session's so an interactive exec running beside the server does not
    /// interleave with it.
    pub wasi_env: Arc<Mutex<WasiEnv>>,
    /// Tripped to stop the execution. The blocking socket calls and
    /// `poll_oneoff` watch this in slices, which is the only reason a server
    /// parked in `accept` can be stopped at all.
    cancel: Arc<AtomicBool>,
    /// Set when the execution ends on its own, so status can tell a running
    /// server from one whose program returned or trapped.
    outcome: Arc<Mutex<Option<ServerOutcome>>>,
}

/// How a serve execution ended.
#[derive(Debug, Clone)]
pub enum ServerOutcome {
    /// The program returned or called `proc_exit`.
    Exited(i32),
    /// The execution failed.
    Failed(String),
    /// It was cancelled by a stop request or session teardown.
    Stopped,
}

impl RunningServer {
    /// Whether the execution is still going.
    pub fn is_running(&self) -> bool {
        self.outcome.lock().map(|o| o.is_none()).unwrap_or(false)
    }

    /// The outcome, if the execution has ended.
    pub fn outcome(&self) -> Option<ServerOutcome> {
        self.outcome.lock().ok().and_then(|o| o.clone())
    }

    /// Ask the execution to stop.
    ///
    /// Cooperative, like every other cancellation in agent mode: the flag is
    /// checked between instructions and inside the blocking socket calls. This
    /// returns as soon as the flag is set rather than joining, because a guest
    /// that ignores it must not be able to block the HTTP handler.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.outcome.lock() {
            if slot.is_none() {
                *slot = Some(ServerOutcome::Stopped);
            }
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// Bind a loopback port and build the WASI environment a server runs in.
///
/// Separate from spawning so the bind can fail as a 4xx/5xx *before* any
/// thread exists, and so the caller learns the resolved port synchronously:
/// an ephemeral bind is the whole reason the address cannot be predicted.
pub fn prepare(
    work_dir: &Path,
    limits: &ResourceLimits,
    port: u16,
) -> std::result::Result<(Arc<Mutex<WasiEnv>>, SocketAddr), ApiError> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| {
        if port == 0 {
            ApiError::Internal(format!("Could not bind a loopback port: {e}"))
        } else {
            ApiError::BadRequest(format!("Could not bind 127.0.0.1:{port}: {e}"))
        }
    })?;
    let addr = listener
        .local_addr()
        .map_err(|e| ApiError::Internal(format!("Bound socket has no address: {e}")))?;

    let mut env = WasiEnv::new().with_preopen("/", work_dir);
    env.set_max_output_bytes(limits.max_output_bytes);
    env.set_max_file_size(limits.max_file_size);
    env.set_max_disk_bytes(limits.max_disk_bytes);
    env.seed_disk_used(crate::agent::limits::dir_size(work_dir));

    // The fd is whatever the table hands out, and it is not 3 here: the work
    // directory is preopened first. The guest is told the number rather than
    // left to assume one.
    let fd = env.add_tcp_listener(listener);
    env.add_env(LISTEN_FD_VAR.to_string(), fd.to_string());
    env.add_env(LISTEN_ADDR_VAR.to_string(), addr.to_string());

    Ok((Arc::new(Mutex::new(env)), addr))
}

/// Run `body` on a background thread as the session's server.
///
/// `body` is the execution itself, which this module deliberately does not
/// know the shape of: source, a project, or a `.wasm` are the caller's
/// business, and every one of them ends up in the same interpreter.
pub fn spawn<F>(
    id: String,
    addr: SocketAddr,
    wasi_env: Arc<Mutex<WasiEnv>>,
    stack_bytes: usize,
    body: F,
) -> std::result::Result<RunningServer, ApiError>
where
    F: FnOnce(Arc<AtomicBool>) -> std::result::Result<i32, ApiError> + Send + 'static,
{
    let cancel = Arc::new(AtomicBool::new(false));
    let outcome: Arc<Mutex<Option<ServerOutcome>>> = Arc::new(Mutex::new(None));

    let cancel_worker = cancel.clone();
    let outcome_worker = outcome.clone();
    std::thread::Builder::new()
        .stack_size(stack_bytes)
        .spawn(move || {
            let result = body(cancel_worker);
            if let Ok(mut slot) = outcome_worker.lock() {
                // A stop that already recorded its outcome wins: the execution
                // ending *because* it was cancelled is not an independent exit.
                if slot.is_none() {
                    *slot = Some(match result {
                        Ok(code) => ServerOutcome::Exited(code),
                        Err(e) => ServerOutcome::Failed(e.to_string()),
                    });
                }
            }
        })
        .map_err(|e| ApiError::Internal(format!("Failed to spawn server thread: {e}")))?;

    Ok(RunningServer {
        id,
        addr,
        started_at: Instant::now(),
        wasi_env,
        cancel,
        outcome,
    })
}

/// Generate a server identifier.
pub fn generate_server_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0);
    format!("srv_{:012x}", nanos & 0xffff_ffff_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;
    use std::time::Duration;

    fn limits() -> ResourceLimits {
        ResourceLimits::default()
    }

    #[test]
    fn prepare_binds_loopback_and_reports_the_port() {
        let dir = tempfile::tempdir().unwrap();
        let (env, addr) = prepare(dir.path(), &limits(), 0).unwrap();
        assert!(addr.ip().is_loopback());
        assert_ne!(addr.port(), 0, "an ephemeral bind must resolve to a port");
        // The port is actually bound: something can connect to it.
        assert!(TcpStream::connect(addr).is_ok());
        drop(env);
    }

    #[test]
    fn the_guest_is_told_which_fd_the_listener_is_on() {
        let dir = tempfile::tempdir().unwrap();
        let (env, addr) = prepare(dir.path(), &limits(), 0).unwrap();
        let locked = env.lock().unwrap();
        let vars: Vec<(String, String)> = locked.env_vars().to_vec();
        let fd = vars
            .iter()
            .find(|(k, _)| k == LISTEN_FD_VAR)
            .map(|(_, v)| v.clone())
            .expect("WASMHUB_LISTEN_FD must be set");
        // Not 3: the work directory is preopened first and takes that one, so
        // a guest assuming the exec-mode number would accept on a directory.
        assert_eq!(fd, "4", "listener follows the preopened work dir");
        let reported = vars
            .iter()
            .find(|(k, _)| k == LISTEN_ADDR_VAR)
            .map(|(_, v)| v.clone())
            .expect("WASMHUB_LISTEN_ADDR must be set");
        assert_eq!(reported, addr.to_string());
    }

    #[test]
    fn a_port_already_taken_is_a_bad_request_not_a_500() {
        let dir = tempfile::tempdir().unwrap();
        let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let taken = held.local_addr().unwrap().port();
        // The caller named the port, so the caller made the mistake.
        match prepare(dir.path(), &limits(), taken) {
            Err(ApiError::BadRequest(_)) => {}
            Err(other) => panic!("expected 400, got {other}"),
            Ok(_) => panic!("binding a taken port must fail"),
        }
    }

    #[test]
    fn a_server_runs_until_it_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let (env, addr) = prepare(dir.path(), &limits(), 0).unwrap();
        let server = spawn("srv_test".into(), addr, env, 1024 * 1024, |cancel| {
            // Stands in for a guest parked in `sock_accept`, which watches
            // the same flag in slices.
            while !cancel.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(0)
        })
        .unwrap();

        assert!(server.is_running());
        server.stop();
        assert!(matches!(server.outcome(), Some(ServerOutcome::Stopped)));
    }

    #[test]
    fn a_program_that_returns_records_its_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let (env, addr) = prepare(dir.path(), &limits(), 0).unwrap();
        let server = spawn("srv_test".into(), addr, env, 1024 * 1024, |_| Ok(3)).unwrap();

        // The worker ends on its own; wait briefly for it to record that.
        for _ in 0..200 {
            if !server.is_running() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(server.outcome(), Some(ServerOutcome::Exited(3))));
    }

    #[test]
    fn dropping_the_handle_cancels_the_execution() {
        let dir = tempfile::tempdir().unwrap();
        let (env, addr) = prepare(dir.path(), &limits(), 0).unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let seen = flag.clone();
        let server = spawn("srv_test".into(), addr, env, 1024 * 1024, move |cancel| {
            while !cancel.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            seen.store(true, Ordering::SeqCst);
            Ok(0)
        })
        .unwrap();

        drop(server);
        for _ in 0..200 {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            flag.load(Ordering::SeqCst),
            "a dropped handle must stop its execution, not leak the thread"
        );
    }
}
