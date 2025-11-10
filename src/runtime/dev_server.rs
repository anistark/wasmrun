use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Response, Server};

use super::microkernel::Pid;
use super::registry::DevServerStatus;

pub struct DevServerManager {
    servers: Arc<Mutex<HashMap<Pid, DevServerHandle>>>,
}

struct DevServerHandle {
    pid: Pid,
    port: u16,
    stop_signal: Arc<Mutex<bool>>,
}

impl Default for DevServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DevServerManager {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_server(&self, pid: Pid, port: u16, project_root: String) -> Result<()> {
        let stop_signal = Arc::new(Mutex::new(false));
        let stop_signal_clone = Arc::clone(&stop_signal);

        let handle = DevServerHandle {
            pid,
            port,
            stop_signal: Arc::clone(&stop_signal),
        };

        {
            let mut servers = self.servers.lock().unwrap();
            servers.insert(pid, handle);
        }

        thread::spawn(move || {
            if let Err(e) = serve_wasi_files(port, &project_root, stop_signal_clone) {
                eprintln!("Dev server error for PID {pid}: {e}");
            }
        });

        Ok(())
    }

    pub fn stop_server(&self, pid: Pid) -> Result<()> {
        let mut servers = self.servers.lock().unwrap();
        if let Some(server) = servers.remove(&pid) {
            let mut stop = server.stop_signal.lock().unwrap();
            *stop = true;
        }
        Ok(())
    }

    pub fn get_status(&self, pid: Pid) -> Option<DevServerStatus> {
        let servers = self.servers.lock().unwrap();
        servers.get(&pid).map(|s| DevServerStatus::Running(s.port))
    }

    #[allow(dead_code)]
    pub fn get_port(&self, pid: Pid) -> Option<u16> {
        let servers = self.servers.lock().unwrap();
        servers.get(&pid).map(|s| s.port)
    }

    pub fn list_servers(&self) -> Vec<(Pid, u16, DevServerStatus)> {
        let servers = self.servers.lock().unwrap();
        servers
            .values()
            .map(|s| (s.pid, s.port, DevServerStatus::Running(s.port)))
            .collect()
    }

    #[allow(dead_code)]
    pub fn reload_server(&self, pid: Pid) -> Result<()> {
        let servers = self.servers.lock().unwrap();
        if let Some(_server) = servers.get(&pid) {
            println!("Reloading dev server for PID {pid}");
            Ok(())
        } else {
            Err(anyhow::anyhow!("Server not found for PID {pid}"))
        }
    }
}

fn serve_wasi_files(
    port: u16,
    project_root: &str,
    stop_signal: Arc<Mutex<bool>>,
) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to start dev server: {e}"))?;

    println!("Dev server (WASI files) started on http://{addr}");

    for request in server.incoming_requests() {
        {
            let should_stop = *stop_signal.lock().unwrap();
            if should_stop {
                break;
            }
        }

        let mut path = request.url().to_string();
        if path == "/" {
            path = "/index.html".to_string();
        }

        let file_path = Path::new(project_root).join(path.trim_start_matches('/'));

        let response = if file_path.exists() && file_path.is_file() {
            match std::fs::read(&file_path) {
                Ok(content) => {
                    let content_type = get_content_type(&file_path);
                    Response::from_data(content)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Type"[..],
                                content_type.as_bytes(),
                            )
                            .unwrap(),
                        )
                }
                Err(_) => Response::from_string("404 Not Found")
                    .with_status_code(tiny_http::StatusCode(404)),
            }
        } else {
            Response::from_string("404 Not Found").with_status_code(tiny_http::StatusCode(404))
        };

        let _ = request.respond(response);
    }

    Ok(())
}

fn get_content_type(path: &Path) -> String {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        _ => "text/plain",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dev_server_manager_creation() {
        let manager = DevServerManager::new();
        assert_eq!(manager.list_servers().len(), 0);
    }

    #[test]
    fn test_start_server() {
        let manager = DevServerManager::new();
        let result = manager.start_server(1, 9999, "/tmp".to_string());
        assert!(result.is_ok());

        std::thread::sleep(std::time::Duration::from_millis(100));

        assert_eq!(manager.get_port(1), Some(9999));
        let status = manager.get_status(1);
        assert!(status.is_some());

        let _ = manager.stop_server(1);
    }

    #[test]
    fn test_stop_server() {
        let manager = DevServerManager::new();
        manager.start_server(2, 9998, "/tmp".to_string()).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let result = manager.stop_server(2);
        assert!(result.is_ok());
        assert!(manager.get_status(2).is_none());
    }

    #[test]
    fn test_list_servers() {
        let manager = DevServerManager::new();
        manager.start_server(3, 9997, "/tmp".to_string()).unwrap();
        manager.start_server(4, 9996, "/tmp".to_string()).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let servers = manager.list_servers();
        assert_eq!(servers.len(), 2);

        manager.stop_server(3).unwrap();
        manager.stop_server(4).unwrap();
    }

    #[test]
    fn test_content_type_detection() {
        assert_eq!(
            get_content_type(Path::new("file.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            get_content_type(Path::new("file.js")),
            "application/javascript"
        );
        assert_eq!(
            get_content_type(Path::new("file.css")),
            "text/css"
        );
        assert_eq!(get_content_type(Path::new("file.json")), "application/json");
    }
}
