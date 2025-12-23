use crate::error::{Result, WasmrunError};
use crate::logging::{LogEntry, LogSource, LogTrailSystem};
use crate::runtime::multilang_kernel::{MultiLanguageKernel, OsRunConfig};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tiny_http::{Header, Method, Request, Response, Server};

/// OS Mode server providing the browser-based development interface
pub struct OsServer {
    kernel: Arc<RwLock<MultiLanguageKernel>>,
    config: OsRunConfig,
    project_pid: Arc<RwLock<Option<u32>>>,
    template_cache: HashMap<String, String>,
    log_system: Arc<LogTrailSystem>,
}

impl OsServer {
    pub fn new(kernel: MultiLanguageKernel, config: OsRunConfig) -> Result<Self> {
        let log_system = kernel.log_system();
        let mut server = Self {
            kernel: Arc::new(RwLock::new(kernel)),
            config,
            project_pid: Arc::new(RwLock::new(None)),
            template_cache: HashMap::new(),
            log_system,
        };

        // Load and process templates
        server.load_templates()?;

        Ok(server)
    }

    /// Load OS mode templates and process variables
    fn load_templates(&mut self) -> Result<()> {
        let templates_dir = Path::new("templates/os");

        if !templates_dir.exists() {
            return Err(WasmrunError::from(
                "OS mode templates not found. Please ensure templates/os/ directory exists.",
            ));
        }

        // Load main template
        let index_path = templates_dir.join("index.html");
        let mut index_content = fs::read_to_string(&index_path)
            .map_err(|e| WasmrunError::from(format!("Failed to read index.html: {e}")))?;

        // Process template variables
        let project_name = Path::new(&self.config.project_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let detected_language = self.detect_project_language()?;
        let language = self
            .config
            .language
            .as_deref()
            .unwrap_or(&detected_language);

        // Replace template variables
        index_content = index_content
            .replace("$PROJECT_NAME$", &project_name)
            .replace("$LANGUAGE$", language)
            .replace("$PROJECT_PATH$", &self.config.project_path)
            .replace("$PORT$", &self.config.port.unwrap_or(8420).to_string());

        // Load and replace placeholders with actual CSS and JS
        let css_path = templates_dir.join("index.css");
        if css_path.exists() {
            let _css_content = fs::read_to_string(&css_path)
                .map_err(|e| WasmrunError::from(format!("Failed to read CSS bundle: {e}")))?;
            index_content = index_content.replace(
                "<!-- @style-placeholder -->",
                "<link rel=\"stylesheet\" href=\"/index.css\">",
            );
        }

        let js_path = templates_dir.join("os.js");
        if js_path.exists() {
            index_content = index_content.replace(
                "<!-- @script-placeholder -->",
                "<script src=\"/os.js\"></script>",
            );
        }

        self.template_cache
            .insert("index.html".to_string(), index_content);

        // Load JavaScript bundle
        let js_path = templates_dir.join("os.js");
        if js_path.exists() {
            let js_content = fs::read_to_string(&js_path)
                .map_err(|e| WasmrunError::from(format!("Failed to read JS bundle: {e}")))?;
            self.template_cache.insert("os.js".to_string(), js_content);
        }

        // Load CSS styles
        let css_path = templates_dir.join("index.css");
        if css_path.exists() {
            let css_content = fs::read_to_string(&css_path)
                .map_err(|e| WasmrunError::from(format!("Failed to read CSS bundle: {e}")))?;
            self.template_cache
                .insert("index.css".to_string(), css_content);
        }

        // Load logging module
        let logging_js_path = templates_dir.join("logging.js");
        if logging_js_path.exists() {
            let logging_js_content = fs::read_to_string(&logging_js_path)
                .map_err(|e| WasmrunError::from(format!("Failed to read logging.js: {e}")))?;
            self.template_cache
                .insert("logging.js".to_string(), logging_js_content);
        }

        // Load logs panel HTML
        let logs_path = templates_dir.join("logs.html");
        if logs_path.exists() {
            let logs_content = fs::read_to_string(&logs_path)
                .map_err(|e| WasmrunError::from(format!("Failed to read logs.html: {e}")))?;
            let logs_content =
                logs_content.replace("$PORT$", &self.config.port.unwrap_or(8420).to_string());
            self.template_cache
                .insert("logs.html".to_string(), logs_content);
        }

        self.log_system.log(LogEntry::info(
            LogSource::Kernel,
            "OS mode templates loaded",
        ));
        println!("✅ OS mode templates loaded");
        Ok(())
    }

    /// Detect the project language
    fn detect_project_language(&self) -> Result<String> {
        // Check for package.json (Node.js)
        if Path::new(&self.config.project_path)
            .join("package.json")
            .exists()
        {
            return Ok("nodejs".to_string());
        }

        // Check for Cargo.toml (Rust)
        if Path::new(&self.config.project_path)
            .join("Cargo.toml")
            .exists()
        {
            return Ok("rust".to_string());
        }

        // Check for go.mod (Go)
        if Path::new(&self.config.project_path).join("go.mod").exists() {
            return Ok("go".to_string());
        }

        // Check for requirements.txt or pyproject.toml (Python)
        let project_path = Path::new(&self.config.project_path);
        if project_path.join("requirements.txt").exists()
            || project_path.join("pyproject.toml").exists()
        {
            return Ok("python".to_string());
        }

        // Default to unknown
        Ok("unknown".to_string())
    }

    /// Start the OS server
    pub fn start(self, port: u16) -> Result<()> {
        let server = Server::http(format!("127.0.0.1:{port}"))
            .map_err(|e| WasmrunError::from(format!("Failed to start HTTP server: {e}")))?;

        self.log_system.log(LogEntry::info(
            LogSource::Kernel,
            format!("OS Mode server listening on http://127.0.0.1:{port}"),
        ));
        println!("🌐 OS Mode server listening on http://127.0.0.1:{port}");

        // Start the project in the kernel
        self.start_project()?;

        // Handle HTTP requests
        for request in server.incoming_requests() {
            match self.handle_request(request) {
                Ok(_) => {}
                Err(e) => eprintln!("Request handling error: {e}"),
            }
        }

        Ok(())
    }

    /// Start the project in the kernel
    fn start_project(&self) -> Result<()> {
        let mut kernel = self.kernel.write().unwrap();

        // Mount the project directory into WASI filesystem
        if let Err(e) = kernel.mount_project(&self.config.project_path) {
            eprintln!("⚠️ Failed to mount project directory: {e}");
        }

        match kernel.auto_detect_and_run(self.config.clone()) {
            Ok(pid) => {
                let mut project_pid = self.project_pid.write().unwrap();
                *project_pid = Some(pid);
                self.log_system.log(
                    LogEntry::info(
                        LogSource::Kernel,
                        format!("Project started with PID: {pid}"),
                    )
                    .with_pid(pid),
                );
                println!("✅ Project started with PID: {pid}");
                Ok(())
            }
            Err(e) => {
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Failed to start project in kernel: {e}"),
                ));
                eprintln!("⚠️ Failed to start project in kernel: {e}");
                // Continue serving the interface even if project fails
                Ok(())
            }
        }
    }

    /// Handle HTTP requests
    fn handle_request(&self, request: Request) -> Result<()> {
        let method = request.method().clone();
        let url = request.url().to_string();

        match (method, url.as_str()) {
            // Serve the main OS interface
            (Method::Get, "/") => {
                if let Some(content) = self.template_cache.get("index.html") {
                    let response = Response::from_string(content).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                            .unwrap(),
                    );
                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                } else {
                    self.send_404(request)?;
                }
            }

            // Serve JavaScript bundle
            (Method::Get, "/os.js") => {
                if let Some(content) = self.template_cache.get("os.js") {
                    let response = Response::from_string(content).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/javascript"[..])
                            .unwrap(),
                    );
                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                } else {
                    self.send_404(request)?;
                }
            }

            // Serve CSS styles
            (Method::Get, "/index.css") => {
                if let Some(content) = self.template_cache.get("index.css") {
                    let response = Response::from_string(content).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/css"[..]).unwrap(),
                    );
                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                } else {
                    self.send_404(request)?;
                }
            }

            // Serve logging module
            (Method::Get, "/logging.js") => {
                if let Some(content) = self.template_cache.get("logging.js") {
                    let response = Response::from_string(content).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/javascript"[..])
                            .unwrap(),
                    );
                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                } else {
                    self.send_404(request)?;
                }
            }

            // Serve logs panel
            (Method::Get, "/logs") => {
                if let Some(content) = self.template_cache.get("logs.html") {
                    let response = Response::from_string(content).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                            .unwrap(),
                    );
                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                } else {
                    self.send_404(request)?;
                }
            }

            (Method::Get, "/ws") => {
                // TODO: WebSocket upgrade for real-time communication
                let response = Response::from_string("WebSocket not implemented yet").with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap(),
                );
                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }

            // API endpoint for kernel statistics
            (Method::Get, "/api/kernel/stats") => {
                self.handle_kernel_stats_request(request)?;
            }

            // API endpoint for filesystem statistics
            (Method::Get, "/api/fs/stats") => {
                self.handle_fs_stats_request(request)?;
            }

            // API endpoint for reading files
            (Method::Get, path) if path.starts_with("/api/fs/read/") => {
                let file_path = &path[13..]; // Remove "/api/fs/read/"
                self.handle_fs_read_request(request, file_path)?;
            }

            // API endpoint for listing directory
            (Method::Get, path) if path.starts_with("/api/fs/list/") => {
                let dir_path = &path[13..]; // Remove "/api/fs/list/"
                self.handle_fs_list_request(request, dir_path)?;
            }

            // API endpoint for writing files
            (Method::Post, path) if path.starts_with("/api/fs/write/") => {
                let file_path = &path[14..]; // Remove "/api/fs/write/"
                self.handle_fs_write_request(request, file_path)?;
            }

            // API endpoint for creating directories
            (Method::Post, path) if path.starts_with("/api/fs/mkdir/") => {
                let dir_path = &path[14..]; // Remove "/api/fs/mkdir/"
                self.handle_fs_mkdir_request(request, dir_path)?;
            }

            // API endpoint for deleting files
            (Method::Post, path) if path.starts_with("/api/fs/delete/") => {
                let file_path = &path[15..]; // Remove "/api/fs/delete/"
                self.handle_fs_delete_request(request, file_path)?;
            }

            (Method::Post, "/api/kernel/start") => {
                self.handle_start_project(request)?;
            }

            (Method::Post, "/api/kernel/restart") => {
                self.handle_restart_project(request)?;
            }

            // API endpoints for port forwarding
            (Method::Get, path)
                if path.starts_with("/api/processes/") && path.ends_with("/ports") =>
            {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 4 {
                    if let Ok(pid) = parts[3].parse::<u32>() {
                        self.handle_list_ports_request(request, pid)?;
                    } else {
                        self.send_error(request, "Invalid PID")?;
                    }
                } else {
                    self.send_404(request)?;
                }
            }

            (Method::Post, path)
                if path.starts_with("/api/processes/") && path.contains("/forward") =>
            {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 4 {
                    if let Ok(pid) = parts[3].parse::<u32>() {
                        self.handle_create_port_forward_request(request, pid)?;
                    } else {
                        self.send_error(request, "Invalid PID")?;
                    }
                } else {
                    self.send_404(request)?;
                }
            }

            (Method::Delete, path)
                if path.starts_with("/api/processes/") && path.contains("/forward/") =>
            {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 6 {
                    if let (Ok(pid), Ok(guest_port)) =
                        (parts[3].parse::<u32>(), parts[5].parse::<u16>())
                    {
                        self.handle_delete_port_forward_request(request, pid, guest_port)?;
                    } else {
                        self.send_error(request, "Invalid PID or port")?;
                    }
                } else {
                    self.send_404(request)?;
                }
            }

            // API endpoint for logs
            (Method::Get, "/api/logs") => {
                self.handle_logs_request(request)?;
            }

            (Method::Get, "/api/logs/recent") => {
                self.handle_recent_logs_request(request)?;
            }

            // Serve static assets
            (Method::Get, path) if path.starts_with("/assets/") => {
                self.serve_asset(request, &path[8..])?; // Remove "/assets/" prefix
            }

            // Proxy requests to project dev server
            (Method::Get, path) if path.starts_with("/app/") => {
                let project_path = &path[5..]; // Remove "/app/" prefix
                self.proxy_to_dev_server(request, project_path)?;
            }

            // Default: serve 404
            _ => {
                self.send_404(request)?;
            }
        }

        Ok(())
    }

    /// Handle start project request
    fn handle_start_project(&self, request: Request) -> Result<()> {
        let project_pid = self.project_pid.read().unwrap();
        if project_pid.is_some() {
            let response_json = serde_json::json!({
                "success": false,
                "error": "Project is already running"
            });

            let response = Response::from_string(response_json.to_string())
                .with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                )
                .with_header(
                    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                );

            request
                .respond(response)
                .map_err(|e| WasmrunError::from(e.to_string()))?;
        } else {
            drop(project_pid); // Release read lock
            match self.start_project() {
                Ok(_) => {
                    let pid = *self.project_pid.read().unwrap();
                    let response_json = serde_json::json!({
                        "success": true,
                        "pid": pid
                    });

                    let response = Response::from_string(response_json.to_string())
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .unwrap(),
                        )
                        .with_header(
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        );

                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                }
                Err(e) => {
                    let response_json = serde_json::json!({
                        "success": false,
                        "error": e.to_string()
                    });

                    let response = Response::from_string(response_json.to_string())
                        .with_status_code(tiny_http::StatusCode(500))
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .unwrap(),
                        )
                        .with_header(
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        );

                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                }
            }
        }
        Ok(())
    }

    /// Handle restart project request
    fn handle_restart_project(&self, request: Request) -> Result<()> {
        // Stop the current project if running
        {
            let pid_opt = *self.project_pid.read().unwrap();
            if let Some(pid) = pid_opt {
                let mut kernel = self.kernel.write().unwrap();
                let _ = kernel.kill_process(pid);
                let mut project_pid = self.project_pid.write().unwrap();
                *project_pid = None;
            }
        }

        // Start the project again
        match self.start_project() {
            Ok(_) => {
                let pid = *self.project_pid.read().unwrap();
                let response_json = serde_json::json!({
                    "success": true,
                    "pid": pid
                });

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                let response_json = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });

                let response = Response::from_string(response_json.to_string())
                    .with_status_code(tiny_http::StatusCode(500))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Handle kernel statistics API request
    fn handle_kernel_stats_request(&self, request: Request) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let stats = kernel.get_statistics();

        let project_pid = *self.project_pid.read().unwrap();
        let stats_json = serde_json::json!({
            "status": "running",
            "active_processes": stats.active_processes,
            "total_memory_usage": stats.total_memory_usage,
            "active_runtimes": stats.active_runtimes,
            "active_dev_servers": stats.active_dev_servers,
            "project_pid": project_pid,
            // System information
            "os": stats.os,
            "arch": stats.arch,
            "kernel_version": stats.kernel_version,
            // WASI capabilities
            "wasi_capabilities": stats.wasi_capabilities,
            "filesystem_mounts": stats.filesystem_mounts,
            "supported_languages": stats.supported_languages,
        });

        let response = Response::from_string(stats_json.to_string())
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(
                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            );

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
    }

    fn handle_list_ports_request(&self, request: Request, pid: u32) -> Result<()> {
        let kernel = self.kernel.read().unwrap();

        let all_network_stats = kernel.get_network_stats();
        if let Some(network_stats) = all_network_stats.get(&pid) {
            let mappings = if let Some(ns) = kernel.get_network_namespace(pid) {
                ns.list_port_mappings()
            } else {
                vec![]
            };

            let response_json = serde_json::json!({
                "success": true,
                "pid": pid,
                "port_mappings": mappings.iter().map(|m| {
                    serde_json::json!({
                        "guest_port": m.guest_port,
                        "host_port": m.host_port,
                        "protocol": format!("{:?}", m.protocol),
                        "created_at": m.created_at.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default().as_secs()
                    })
                }).collect::<Vec<_>>(),
                "network_stats": {
                    "base_port": network_stats.base_port,
                    "allocated_ports": network_stats.allocated_ports,
                    "total_connections": network_stats.total_connections,
                    "active_connections": network_stats.active_connections,
                    "listening_sockets": network_stats.listening_sockets,
                }
            });

            let response = Response::from_string(response_json.to_string())
                .with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                )
                .with_header(
                    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                );

            request
                .respond(response)
                .map_err(|e| WasmrunError::from(e.to_string()))?;
        } else {
            self.send_error(request, &format!("Process with PID {pid} not found"))?;
        }
        Ok(())
    }

    fn handle_create_port_forward_request(&self, mut request: Request, pid: u32) -> Result<()> {
        let mut content = String::new();
        let mut reader = request.as_reader();
        if let Err(e) = std::io::Read::read_to_string(&mut reader, &mut content) {
            return self.send_error(request, &format!("Failed to read request body: {e}"));
        }

        let body: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => return self.send_error(request, &format!("Invalid JSON: {e}")),
        };

        let guest_port = match body.get("guest_port").and_then(|v| v.as_u64()) {
            Some(p) if p <= u16::MAX as u64 => p as u16,
            _ => return self.send_error(request, "Invalid guest_port"),
        };

        let protocol = match body.get("protocol").and_then(|v| v.as_str()) {
            Some("tcp") | Some("Tcp") => crate::runtime::network_namespace::SocketProtocol::Tcp,
            Some("udp") | Some("Udp") => crate::runtime::network_namespace::SocketProtocol::Udp,
            _ => return self.send_error(request, "Invalid protocol (must be 'tcp' or 'udp')"),
        };

        let kernel = self.kernel.read().unwrap();
        if let Some(ns) = kernel.get_network_namespace(pid) {
            match ns.allocate_port(guest_port, protocol) {
                Ok(host_port) => {
                    let response_json = serde_json::json!({
                        "success": true,
                        "pid": pid,
                        "guest_port": guest_port,
                        "host_port": host_port,
                        "protocol": format!("{:?}", protocol)
                    });

                    let response = Response::from_string(response_json.to_string())
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .unwrap(),
                        )
                        .with_header(
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        );

                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                }
                Err(e) => {
                    self.send_error(request, &format!("Failed to allocate port: {e}"))?;
                }
            }
        } else {
            self.send_error(request, &format!("Process with PID {pid} not found"))?;
        }
        Ok(())
    }

    fn handle_delete_port_forward_request(
        &self,
        request: Request,
        pid: u32,
        guest_port: u16,
    ) -> Result<()> {
        let kernel = self.kernel.read().unwrap();

        if let Some(ns) = kernel.get_network_namespace(pid) {
            match ns.deallocate_port(guest_port) {
                Ok(()) => {
                    let response_json = serde_json::json!({
                        "success": true,
                        "pid": pid,
                        "guest_port": guest_port,
                        "message": "Port mapping removed successfully"
                    });

                    let response = Response::from_string(response_json.to_string())
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .unwrap(),
                        )
                        .with_header(
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        );

                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                }
                Err(e) => {
                    self.send_error(request, &format!("Failed to remove port mapping: {e}"))?;
                }
            }
        } else {
            self.send_error(request, &format!("Process with PID {pid} not found"))?;
        }
        Ok(())
    }

    fn send_error(&self, request: Request, error_msg: &str) -> Result<()> {
        let response_json = serde_json::json!({
            "success": false,
            "error": error_msg
        });

        let response = Response::from_string(response_json.to_string())
            .with_status_code(tiny_http::StatusCode(400))
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(
                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            );

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
    }

    /// Proxy requests to the project's dev server
    fn proxy_to_dev_server(&self, request: Request, path: &str) -> Result<()> {
        // Get the dev server port for the project
        let project_pid = *self.project_pid.read().unwrap();
        if let Some(pid) = project_pid {
            let kernel = self.kernel.read().unwrap();
            let dev_server_port = kernel.get_dev_server_status(pid).and_then(|status| {
                if let crate::runtime::registry::DevServerStatus::Running(port) = status {
                    Some(port)
                } else {
                    None
                }
            });

            if let Some(port) = dev_server_port {
                // Forward the request to the dev server
                let target_url = format!(
                    "http://127.0.0.1:{}{}",
                    port,
                    if path.is_empty() { "/" } else { path }
                );

                match self.fetch_from_dev_server(&target_url) {
                    Ok((content, content_type)) => {
                        let response = Response::from_string(content).with_header(
                            Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                                .unwrap(),
                        );
                        request
                            .respond(response)
                            .map_err(|e| WasmrunError::from(e.to_string()))?;
                    }
                    Err(e) => {
                        let error_html = format!(
                            "<html><body><h1>Dev Server Error</h1><p>{e}</p></body></html>"
                        );
                        let response = Response::from_string(error_html).with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
                        );
                        request
                            .respond(response)
                            .map_err(|e| WasmrunError::from(e.to_string()))?;
                    }
                }
            } else {
                let error_html = format!(
                    "<html><body><h1>No Dev Server</h1><p>No dev server running for PID {pid}</p></body></html>"
                );
                let response = Response::from_string(error_html).with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
                );
                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        } else {
            let error_html = "<html><body><h1>No Project Running</h1><p>No project is currently running</p></body></html>";
            let response = Response::from_string(error_html)
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap());
            request
                .respond(response)
                .map_err(|e| WasmrunError::from(e.to_string()))?;
        }
        Ok(())
    }

    /// Fetch content from the dev server
    fn fetch_from_dev_server(&self, url: &str) -> Result<(String, String)> {
        use std::io::Read;
        use std::net::TcpStream;

        // Parse the URL to get host and path
        let url_without_scheme = url.strip_prefix("http://").unwrap_or(url);
        let parts: Vec<&str> = url_without_scheme.splitn(2, '/').collect();
        let host = parts[0];
        let path = if parts.len() > 1 {
            format!("/{}", parts[1])
        } else {
            "/".to_string()
        };

        // Connect to the dev server
        let mut stream = TcpStream::connect(host)
            .map_err(|e| WasmrunError::from(format!("Failed to connect to dev server: {e}")))?;

        // Send HTTP request
        let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        std::io::Write::write_all(&mut stream, request.as_bytes())
            .map_err(|e| WasmrunError::from(format!("Failed to send request: {e}")))?;

        // Read response
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|e| WasmrunError::from(format!("Failed to read response: {e}")))?;

        // Parse HTTP response
        if let Some(header_end) = response.find("\r\n\r\n") {
            let headers = &response[..header_end];
            let body = &response[header_end + 4..];

            // Extract content type from headers
            let content_type = headers
                .lines()
                .find(|line| line.to_lowercase().starts_with("content-type:"))
                .and_then(|line| line.split(':').nth(1))
                .map(|ct| ct.trim().to_string())
                .unwrap_or_else(|| "text/html".to_string());

            Ok((body.to_string(), content_type))
        } else {
            Err(WasmrunError::from("Invalid HTTP response"))
        }
    }

    /// Serve static assets
    fn serve_asset(&self, request: Request, asset_path: &str) -> Result<()> {
        let full_path = Path::new("templates/assets").join(asset_path);

        if !full_path.exists() {
            return self.send_404(request);
        }

        let content = fs::read(&full_path)
            .map_err(|e| WasmrunError::from(format!("Failed to read asset: {e}")))?;

        let content_type = match full_path.extension().and_then(|ext| ext.to_str()) {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("svg") => "image/svg+xml",
            Some("css") => "text/css",
            Some("js") => "application/javascript",
            _ => "application/octet-stream",
        };

        let response = Response::from_data(content).with_header(
            Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap(),
        );

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
    }

    /// Send 404 Not Found response
    fn send_404(&self, request: Request) -> Result<()> {
        let not_found = "
            <html>
                <head><title>404 - Not Found</title></head>
                <body>
                    <h1>404 - Not Found</h1>
                    <p>The requested resource was not found on this server.</p>
                </body>
            </html>
        ";

        let response = Response::from_string(not_found)
            .with_status_code(tiny_http::StatusCode(404))
            .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap());

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
    }

    /// Handle filesystem statistics request
    fn handle_fs_stats_request(&self, request: Request) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let wasi_fs = kernel.wasi_filesystem();
        let stats = wasi_fs.get_stats();

        let stats_json =
            serde_json::to_string(&stats).map_err(|e| WasmrunError::from(e.to_string()))?;

        let response = Response::from_string(stats_json)
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(
                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            );

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
    }

    /// Handle file read request
    fn handle_fs_read_request(&self, request: Request, file_path: &str) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let wasi_fs = kernel.wasi_filesystem();

        // Ensure path has leading slash
        let normalized_path = if file_path.starts_with('/') {
            file_path.to_string()
        } else {
            format!("/{file_path}")
        };

        match wasi_fs.read_file(&normalized_path) {
            Ok(content) => {
                // Try to detect if it's text or binary
                let is_text = content
                    .iter()
                    .all(|&b| b.is_ascii() || b == b'\n' || b == b'\r' || b == b'\t');

                let response_json = if is_text {
                    serde_json::json!({
                        "success": true,
                        "path": file_path,
                        "content": String::from_utf8_lossy(&content),
                        "size": content.len(),
                        "type": "text"
                    })
                } else {
                    // For binary files, return hex representation
                    let hex_content: String = content.iter().map(|b| format!("{b:02x}")).collect();
                    serde_json::json!({
                        "success": true,
                        "path": file_path,
                        "content": hex_content,
                        "size": content.len(),
                        "type": "binary"
                    })
                };

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(404))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Handle directory listing request
    fn handle_fs_list_request(&self, request: Request, dir_path: &str) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let wasi_fs = kernel.wasi_filesystem();

        // Ensure path has leading slash
        let normalized_path = if dir_path.starts_with('/') {
            dir_path.to_string()
        } else {
            format!("/{dir_path}")
        };

        match wasi_fs.path_readdir(&normalized_path) {
            Ok(entries) => {
                let response_json = serde_json::json!({
                    "success": true,
                    "path": dir_path,
                    "entries": entries
                });

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(404))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Handle file write request
    fn handle_fs_write_request(&self, mut request: Request, file_path: &str) -> Result<()> {
        // Read the request body
        let mut body = Vec::new();
        let mut reader = request.as_reader();
        std::io::Read::read_to_end(&mut reader, &mut body)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

        let kernel = self.kernel.read().unwrap();
        let wasi_fs = kernel.wasi_filesystem();

        // Ensure path has leading slash
        let normalized_path = if file_path.starts_with('/') {
            file_path.to_string()
        } else {
            format!("/{file_path}")
        };

        match wasi_fs.write_file(&normalized_path, &body) {
            Ok(_) => {
                let response_json = serde_json::json!({
                    "success": true,
                    "path": file_path,
                    "size": body.len()
                });

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(500))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Handle directory creation request
    fn handle_fs_mkdir_request(&self, request: Request, dir_path: &str) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let wasi_fs = kernel.wasi_filesystem();

        // Ensure path has leading slash
        let normalized_path = if dir_path.starts_with('/') {
            dir_path.to_string()
        } else {
            format!("/{dir_path}")
        };

        match wasi_fs.path_create_directory(&normalized_path) {
            Ok(_) => {
                let response_json = serde_json::json!({
                    "success": true,
                    "path": dir_path
                });

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(500))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Handle file deletion request
    fn handle_fs_delete_request(&self, request: Request, file_path: &str) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let wasi_fs = kernel.wasi_filesystem();

        // Ensure path has leading slash
        let normalized_path = if file_path.starts_with('/') {
            file_path.to_string()
        } else {
            format!("/{file_path}")
        };

        match wasi_fs.path_unlink_file(&normalized_path) {
            Ok(_) => {
                let response_json = serde_json::json!({
                    "success": true,
                    "path": file_path
                });

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(500))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }

        Ok(())
    }

    fn handle_logs_request(&self, request: Request) -> Result<()> {
        let logs = self.log_system.get_all();
        let response_json = serde_json::json!({
            "success": true,
            "count": logs.len(),
            "logs": logs
        });

        let response = Response::from_string(response_json.to_string())
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(
                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            );

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

        Ok(())
    }

    fn handle_recent_logs_request(&self, request: Request) -> Result<()> {
        let count = 100;
        let logs = self.log_system.get_recent(count);
        let response_json = serde_json::json!({
            "success": true,
            "count": logs.len(),
            "logs": logs
        });

        let response = Response::from_string(response_json.to_string())
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(
                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            );

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

        Ok(())
    }
}
