//! OS mode: the wasmnet proxy that gives the browser VM real sockets.
//!
//! The browser has no raw sockets, so `sock_*` calls made inside the VM leave
//! through a WebSocket to wasmnet, which opens the real connection under a
//! policy. wasmnet is async and the OS server is `tiny_http`, which is not and
//! has no upgrade path, so the proxy gets its own port and its own tokio
//! runtime on its own thread. That thread is the whole async boundary: nothing
//! else in wasmrun becomes async, and the OS server talks to it only through
//! the handle below.

use crate::error::{Result, WasmrunError};
use std::net::TcpListener;
use std::thread::JoinHandle;
use tokio::sync::oneshot;
use wasmnet::policy::PolicyConfig;

/// How far above the OS server's port the proxy looks for one of its own.
/// The default OS port is 8420, so the proxy defaults to 8440, leaving 8430
/// (agent mode) alone.
const PORT_OFFSET: u16 = 20;

/// How many ports to try before giving up.
const PORT_SCAN_LIMIT: u16 = 20;

/// A running wasmnet proxy: the port it bound, the thread driving it, and the
/// channel that stops it.
///
/// Dropping the handle shuts the proxy down and waits for its thread, so the
/// proxy cannot outlive the OS server that started it.
pub struct NetworkServer {
    port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl NetworkServer {
    /// Start the proxy on a free port at or above `os_port + PORT_OFFSET`.
    ///
    /// Binding happens on the proxy's own thread, so this returns as soon as
    /// the thread is spawned rather than when the listener is up. The port is
    /// picked here, synchronously, so the caller has something to advertise
    /// immediately.
    pub fn start(host: &str, os_port: u16, policy: PolicyConfig) -> Result<Self> {
        let port = pick_port(host, os_port.saturating_add(PORT_OFFSET))?;
        let (tx, rx) = oneshot::channel();
        let host = host.to_string();

        let thread = std::thread::Builder::new()
            .name("wasmnet".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        eprintln!("⚠️ Network proxy failed to start its runtime: {e}");
                        return;
                    }
                };

                let server = match wasmnet::Server::builder()
                    .host(host)
                    .port(port)
                    .policy_config(policy)
                    .build()
                {
                    Ok(server) => server,
                    Err(e) => {
                        eprintln!("⚠️ Network proxy failed to start: {e}");
                        return;
                    }
                };

                if let Err(e) = runtime.block_on(server.listen_with_shutdown(rx)) {
                    eprintln!("⚠️ Network proxy stopped: {e}");
                }
            })
            .map_err(|e| {
                WasmrunError::from(format!("Failed to spawn network proxy thread: {e}"))
            })?;

        Ok(Self {
            port,
            shutdown: Some(tx),
            thread: Some(thread),
        })
    }

    /// The port the proxy bound.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The URL the browser shim connects to.
    pub fn url(&self, host: &str) -> String {
        format!("ws://{host}:{}", self.port)
    }

    /// Signal the proxy to stop and wait for its thread.
    ///
    /// A send failure means the runtime is already gone, which is the outcome
    /// this asks for, so it is not an error.
    fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for NetworkServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Find a free port at or above `preferred`.
///
/// The listener is dropped before wasmnet binds it for real, so this is a
/// probe rather than a reservation: another process can take the port in
/// between. That window is the same one every other port check in wasmrun
/// lives with, and wasmnet reports the bind failure if it loses the race.
fn pick_port(host: &str, preferred: u16) -> Result<u16> {
    for offset in 0..PORT_SCAN_LIMIT {
        let port = match preferred.checked_add(offset) {
            Some(port) => port,
            None => break,
        };
        if TcpListener::bind((host, port)).is_ok() {
            return Ok(port);
        }
    }

    Err(WasmrunError::from(format!(
        "No free port for the network proxy in {}-{}",
        preferred,
        preferred.saturating_add(PORT_SCAN_LIMIT - 1)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    /// Wait for the proxy to accept a connection, since it binds on its own
    /// thread after `start` returns.
    fn wait_until_listening(port: u16) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    fn wait_until_closed(port: u16) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_err() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn picks_the_preferred_port_when_it_is_free() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let free = listener.local_addr().unwrap().port();
        drop(listener);

        assert_eq!(pick_port("127.0.0.1", free).unwrap(), free);
    }

    #[test]
    fn skips_a_taken_port() {
        let taken = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = taken.local_addr().unwrap().port();

        let picked = pick_port("127.0.0.1", port).unwrap();
        assert!(picked > port, "expected a port above {port}, got {picked}");
    }

    #[test]
    fn defaults_to_twenty_above_the_os_port() {
        // 8420 is the OS default, and 8430 is agent mode's: the proxy must not
        // land on it.
        assert_eq!(8420u16.saturating_add(PORT_OFFSET), 8440);
    }

    #[test]
    fn starts_and_stops_with_its_handle() {
        // `start` adds PORT_OFFSET to what it is given, so hand it an
        // ephemeral port minus the offset to land back in unprivileged range.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let free = listener.local_addr().unwrap().port();
        drop(listener);

        let server = NetworkServer::start("127.0.0.1", free - PORT_OFFSET, PolicyConfig::default())
            .expect("proxy starts");
        let port = server.port();

        assert!(wait_until_listening(port), "proxy never listened on {port}");
        assert_eq!(server.url("127.0.0.1"), format!("ws://127.0.0.1:{port}"));

        drop(server);
        assert!(wait_until_closed(port), "proxy still listening on {port}");
    }
}
