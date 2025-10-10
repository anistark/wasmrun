use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
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
            if let Err(e) = run_dev_server(port, project_root, stop_signal_clone) {
                eprintln!("Dev server error for PID {}: {}", pid, e);
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

    pub fn reload_server(&self, pid: Pid) -> Result<()> {
        let servers = self.servers.lock().unwrap();
        if let Some(_server) = servers.get(&pid) {
            println!("🔄 Reloading dev server for PID {}", pid);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Server not found for PID {}", pid))
        }
    }
}

fn run_dev_server(port: u16, _project_root: String, stop_signal: Arc<Mutex<bool>>) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to start dev server: {}", e))?;

    println!("✅ Dev server started on http://{}", addr);

    for request in server.incoming_requests() {
        {
            let should_stop = *stop_signal.lock().unwrap();
            if should_stop {
                println!("🛑 Stopping dev server on port {}", port);
                break;
            }
        }

        let response = Response::from_string(format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Dev Server - Port {}</title>
    <style>
        body {{
            font-family: system-ui, -apple-system, sans-serif;
            background: #0a0a0a;
            color: #10b981;
            padding: 2rem;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
        }}
        .container {{
            text-align: center;
            border: 2px solid #10b981;
            padding: 3rem;
            border-radius: 1rem;
            background: rgba(16, 185, 129, 0.1);
        }}
        h1 {{ color: #10b981; margin-bottom: 1rem; }}
        p {{ color: #a0a0a0; margin: 0.5rem 0; }}
        .status {{
            display: inline-block;
            padding: 0.5rem 1rem;
            background: #10b981;
            color: #000;
            border-radius: 0.5rem;
            font-weight: bold;
            margin-top: 1rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 wasmrun Dev Server</h1>
        <p>Port: {}</p>
        <p>Status: <span class="status">RUNNING</span></p>
        <p style="margin-top: 2rem; color: #666;">
            This is a development server managed by wasmrun OS mode
        </p>
    </div>
</body>
</html>"#,
            port, port
        ))
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                .unwrap(),
        );

        if let Err(e) = request.respond(response) {
            eprintln!("Failed to send response: {}", e);
        }
    }

    Ok(())
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
}
