use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Response, Server};

use super::microkernel::Pid;
use super::registry::DevServerStatus;
use super::wasi_fs::WasiFilesystem;

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

    pub fn start_server(
        &self,
        pid: Pid,
        port: u16,
        project_root: String,
        wasi_fs: Arc<WasiFilesystem>,
    ) -> Result<()> {
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
            if let Err(e) = serve_wasi_files(port, &project_root, wasi_fs, stop_signal_clone) {
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

    #[cfg(test)]
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
}

fn serve_wasi_files(
    port: u16,
    project_root: &str,
    wasi_fs: Arc<WasiFilesystem>,
    stop_signal: Arc<Mutex<bool>>,
) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let server =
        Server::http(&addr).map_err(|e| anyhow::anyhow!("Failed to start dev server: {e}"))?;

    println!("Dev server (WASI files) started on http://{addr}");

    loop {
        if *stop_signal.lock().unwrap() {
            break;
        }

        let request = match server.recv_timeout(Duration::from_millis(250)) {
            Ok(Some(req)) => req,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("Dev server recv error: {e}");
                break;
            }
        };

        let mut url_path = request.url().to_string();
        if url_path == "/" {
            url_path = "/index.html".to_string();
        }

        let vfs_path = format!(
            "{}/{}",
            project_root.trim_end_matches('/'),
            url_path.trim_start_matches('/')
        );

        let response = match wasi_fs.read_file(&vfs_path) {
            Ok(content) => {
                let content_type = get_content_type(Path::new(&url_path));
                Response::from_data(content).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                        .unwrap(),
                )
            }
            Err(_) => {
                Response::from_string("404 Not Found").with_status_code(tiny_http::StatusCode(404))
            }
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
    use std::io::Read;
    use std::time::Duration;
    use tempfile::tempdir;

    fn create_wasi_fs_with_mount(mount_path: &str, host_dir: &Path) -> Arc<WasiFilesystem> {
        let fs = Arc::new(WasiFilesystem::new());
        fs.mount(mount_path, host_dir).unwrap();
        fs
    }

    #[test]
    fn test_dev_server_manager_creation() {
        let manager = DevServerManager::new();
        assert_eq!(manager.list_servers().len(), 0);
    }

    #[test]
    fn test_start_server() {
        let temp = tempdir().unwrap();
        let wasi_fs = create_wasi_fs_with_mount("/projects/1", temp.path());

        let manager = DevServerManager::new();
        let result = manager.start_server(1, 9999, "/projects/1".to_string(), wasi_fs);
        assert!(result.is_ok());

        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(manager.get_port(1), Some(9999));
        let status = manager.get_status(1);
        assert!(status.is_some());

        let _ = manager.stop_server(1);
    }

    #[test]
    fn test_stop_server() {
        let temp = tempdir().unwrap();
        let wasi_fs = create_wasi_fs_with_mount("/projects/2", temp.path());

        let manager = DevServerManager::new();
        manager
            .start_server(2, 9998, "/projects/2".to_string(), wasi_fs)
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));

        let result = manager.stop_server(2);
        assert!(result.is_ok());
        assert!(manager.get_status(2).is_none());
    }

    #[test]
    fn test_list_servers() {
        let temp3 = tempdir().unwrap();
        let temp4 = tempdir().unwrap();
        let wasi_fs = Arc::new(WasiFilesystem::new());
        wasi_fs.mount("/projects/3", temp3.path()).unwrap();
        wasi_fs.mount("/projects/4", temp4.path()).unwrap();

        let manager = DevServerManager::new();
        manager
            .start_server(3, 9997, "/projects/3".to_string(), Arc::clone(&wasi_fs))
            .unwrap();
        manager
            .start_server(4, 9996, "/projects/4".to_string(), Arc::clone(&wasi_fs))
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));

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
        assert_eq!(get_content_type(Path::new("file.css")), "text/css");
        assert_eq!(get_content_type(Path::new("file.json")), "application/json");
    }

    #[test]
    fn test_dev_server_serves_wasi_files() {
        let temp = tempdir().unwrap();
        std::fs::write(temp.path().join("index.html"), b"<h1>Hello</h1>").unwrap();
        std::fs::write(temp.path().join("style.css"), b"body {}").unwrap();

        let wasi_fs = create_wasi_fs_with_mount("/projects/10", temp.path());

        let manager = DevServerManager::new();
        manager
            .start_server(10, 19876, "/projects/10".to_string(), wasi_fs)
            .unwrap();

        std::thread::sleep(Duration::from_millis(200));

        let mut resp = ureq::get("http://127.0.0.1:19876/").call().unwrap();
        let mut body = String::new();
        resp.body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .unwrap();
        assert_eq!(body, "<h1>Hello</h1>");

        let mut resp = ureq::get("http://127.0.0.1:19876/style.css")
            .call()
            .unwrap();
        let mut body = String::new();
        resp.body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .unwrap();
        assert_eq!(body, "body {}");

        let result = ureq::get("http://127.0.0.1:19876/missing.txt").call();
        assert!(result.is_err());

        manager.stop_server(10).unwrap();
    }

    #[test]
    fn test_stop_signal_terminates_promptly() {
        let temp = tempdir().unwrap();
        let wasi_fs = create_wasi_fs_with_mount("/projects/20", temp.path());

        let stop = Arc::new(Mutex::new(false));
        let stop_clone = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            serve_wasi_files(19877, "/projects/20", wasi_fs, stop_clone).unwrap();
        });

        thread::sleep(Duration::from_millis(200));

        *stop.lock().unwrap() = true;

        // Should join within ~500ms (250ms poll + margin), not hang forever
        let joined = thread::spawn(move || handle.join().unwrap());
        thread::sleep(Duration::from_millis(500));
        assert!(joined.is_finished(), "Server thread did not stop promptly");
    }
}
