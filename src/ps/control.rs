//! [Process Manager] How `wasmrun ps` talks to a running supervisor: a small
//! HTTP API on a loopback port, and a state file saying which port and which
//! token. The file is readable by its owner only, and every request but
//! `/health` must carry the token, so another user on the machine cannot stop
//! or start your programs.
//!
//! ```text
//! GET  /health                      liveness, no token
//! GET  /processes                   every program's status
//! GET  /processes/<name>            one program's status
//! POST /processes/<name>/start|stop|restart
//! POST /load      {"config": "/abs/processes.toml"}
//! POST /shutdown  stop every program, then the supervisor
//! ```

use crate::ps::supervisor::Supervisor;
use serde::{Deserialize, Serialize};
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const TOKEN_HEADER: &str = "x-wasmrun-ps-token";

/// What a running supervisor writes to `supervisor.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub started_at: String,
    pub version: String,
}

/// Where the process manager keeps its state: `WASMRUN_PS_HOME`, or
/// `~/.wasmrun/ps`.
pub fn ps_home() -> PathBuf {
    if let Ok(dir) = std::env::var("WASMRUN_PS_HOME") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".wasmrun")
        .join("ps")
}

pub fn state_file(home: &Path) -> PathBuf {
    home.join("supervisor.json")
}

/// 256 bits from the standard library's per-process random hash keys, which
/// is plenty for a token that only has to be unguessable by other local users.
pub fn new_token() -> String {
    (0..4)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i ^ std::process::id() as u64);
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0),
            );
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Write the state file, readable by its owner only.
pub fn write_state(home: &Path, info: &SupervisorInfo) -> std::io::Result<()> {
    std::fs::create_dir_all(home)?;
    let path = state_file(home);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(info).unwrap_or_default())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}

/// The running supervisor, if there is one: a state file whose process is a
/// live wasmrun answering on its port. A stale file is removed.
pub fn running(home: &Path) -> Option<SupervisorInfo> {
    let path = state_file(home);
    let info: SupervisorInfo = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    let alive = crate::utils::process::is_wasmrun(info.pid)
        && request(&info, "GET", "/health", None).is_ok_and(|(code, _)| code == 200);
    if alive {
        Some(info)
    } else {
        let _ = std::fs::remove_file(path);
        None
    }
}

/// One request to the supervisor: the status code and the body.
pub fn request(
    info: &SupervisorInfo,
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<(u16, String), String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], info.port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|e| format!("cannot reach the supervisor on port {}: {e}", info.port))?;
    // Stopping a program waits out its grace period before answering.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{TOKEN_HEADER}: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        info.token,
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .map_err(|e| e.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    let code = response
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or("malformed response from the supervisor")?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Ok((code, body))
}

/// Serve the control API until a `/shutdown` or `stop` flips `stop`.
pub fn serve(
    server: tiny_http::Server,
    token: &str,
    supervisor: &Supervisor,
    load: &dyn Fn(&Path) -> Result<Vec<String>, String>,
    stop: &Arc<AtomicBool>,
) {
    while !stop.load(Ordering::SeqCst) {
        let mut request = match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => break,
        };
        let url = request.url().to_string();
        let method = request.method().clone();
        if url == "/health" {
            let _ = request.respond(tiny_http::Response::from_string("ok"));
            continue;
        }
        let authorized = request
            .headers()
            .iter()
            .any(|h| h.field.equiv(TOKEN_HEADER) && h.value.as_str() == token);
        if !authorized {
            let _ = request.respond(json(
                401,
                &serde_json::json!({"error": "bad or missing token"}),
            ));
            continue;
        }
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);

        use tiny_http::Method::{Get, Post};
        let parts: Vec<&str> = url.trim_start_matches('/').split('/').collect();
        let reply = match (&method, parts.as_slice()) {
            (Get, ["processes"]) => Ok(serde_json::json!(supervisor.list())),
            (Get, ["processes", name]) => supervisor.status(name).map(|s| serde_json::json!(s)),
            (Post, ["processes", name, action]) => match *action {
                "start" => supervisor.start(name),
                "stop" => supervisor.stop(name),
                "restart" => supervisor.restart(name),
                other => Err(format!("unknown action '{other}'")),
            }
            .map(|()| serde_json::json!({"ok": true})),
            (Post, ["load"]) => serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("config").and_then(|c| c.as_str()).map(PathBuf::from))
                .ok_or_else(|| "expected {\"config\": \"/path/processes.toml\"}".to_string())
                .and_then(|path| load(&path))
                .map(|names| serde_json::json!({"added": names})),
            (Post, ["shutdown"]) => {
                stop.store(true, Ordering::SeqCst);
                Ok(serde_json::json!({"ok": true}))
            }
            _ => Err(format!("no route for {method} {url}")),
        };
        let response = match reply {
            Ok(value) => json(200, &value),
            Err(e) if e.starts_with("no process named") || e.starts_with("no route") => {
                json(404, &serde_json::json!({"error": e}))
            }
            Err(e) => json(400, &serde_json::json!({"error": e})),
        };
        let _ = request.respond(response);
    }
}

fn json(code: u16, value: &serde_json::Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(value.to_string())
        .with_status_code(code)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_and_differ() {
        let a = new_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, new_token());
    }

    #[cfg(unix)]
    #[test]
    fn the_state_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let info = SupervisorInfo {
            pid: 1,
            port: 1,
            token: "t".into(),
            started_at: String::new(),
            version: String::new(),
        };
        write_state(dir.path(), &info).unwrap();
        let mode = std::fs::metadata(state_file(dir.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
