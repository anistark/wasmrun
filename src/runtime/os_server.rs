use crate::error::{Result, WasmrunError};
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
    project_pid: Option<u32>,
    template_cache: HashMap<String, String>,
}

impl OsServer {
    pub fn new(kernel: MultiLanguageKernel, config: OsRunConfig) -> Result<Self> {
        let mut server = Self {
            kernel: Arc::new(RwLock::new(kernel)),
            config,
            project_pid: None,
            template_cache: HashMap::new(),
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
    pub fn start(mut self, port: u16) -> Result<()> {
        let server = Server::http(format!("127.0.0.1:{port}"))
            .map_err(|e| WasmrunError::from(format!("Failed to start HTTP server: {e}")))?;

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
    fn start_project(&mut self) -> Result<()> {
        let mut kernel = self.kernel.write().unwrap();

        // Mount the project directory into WASI filesystem
        if let Err(e) = kernel.mount_project(&self.config.project_path) {
            eprintln!("⚠️ Failed to mount project directory: {e}");
        }

        match kernel.auto_detect_and_run(self.config.clone()) {
            Ok(pid) => {
                self.project_pid = Some(pid);
                println!("✅ Project started with PID: {pid}");
                Ok(())
            }
            Err(e) => {
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

            // WebSocket endpoint (simplified HTTP upgrade simulation)
            (Method::Get, "/ws") => {
                // For now, return a simple response
                // TODO: Implement proper WebSocket upgrade
                let response =
                    Response::from_string("WebSocket endpoint - upgrade not implemented yet")
                        .with_header(
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

            // API endpoint for process management
            (Method::Post, "/api/kernel/restart") => {
                // Note: This requires mutable access but we can't get it in this context
                // For now, return a placeholder response
                let response = Response::from_string(
                    r#"{"error": "Restart functionality not yet implemented"}"#,
                )
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

            // Serve static assets
            (Method::Get, path) if path.starts_with("/assets/") => {
                self.serve_asset(request, &path[8..])?; // Remove "/assets/" prefix
            }

            // Default: serve 404
            _ => {
                self.send_404(request)?;
            }
        }

        Ok(())
    }

    /// Handle kernel statistics API request
    fn handle_kernel_stats_request(&self, request: Request) -> Result<()> {
        let kernel = self.kernel.read().unwrap();
        let stats = kernel.get_statistics();

        let stats_json = serde_json::json!({
            "status": "running",
            "active_processes": stats.active_processes,
            "total_memory_usage": stats.total_memory_usage,
            "active_runtimes": stats.active_runtimes,
            "active_dev_servers": stats.active_dev_servers,
            "project_pid": self.project_pid,
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

    /// Proxy request to the user's project running in the kernel
    #[allow(dead_code)]
    fn proxy_to_project(&self, request: Request, path: &str) -> Result<()> {
        // For now, return a placeholder response
        // TODO: Implement actual proxying to the project's HTTP server
        let placeholder = format!(
            "
            <html>
                <head><title>Project Proxy</title></head>
                <body style=\"background-color: #000; color: white; font-family: system-ui, -apple-system, sans-serif; padding: 2rem;\">
                    <h1 style=\"color: #10b981;\">Project Running in Kernel</h1>
                    <p style=\"color: white;\">Path: {}</p>
                    <p style=\"color: white;\">PID: {:?}</p>
                    <p style=\"color: white;\">This would proxy to your project's HTTP server.</p>
                    <p style=\"color: #10b981;\">Implementation in progress...</p>
                </body>
            </html>
        ",
            path, self.project_pid
        );

        let response = Response::from_string(placeholder)
            .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap());

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
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

        let stats_json = serde_json::to_string(&stats)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

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
            format!("/{}", file_path)
        };

        match wasi_fs.read_file(&normalized_path) {
            Ok(content) => {
                // Try to detect if it's text or binary
                let is_text = content.iter().all(|&b| b.is_ascii() || b == b'\n' || b == b'\r' || b == b'\t');

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
                    let hex_content: String = content.iter()
                        .map(|b| format!("{:02x}", b))
                        .collect();
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
            format!("/{}", dir_path)
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
            format!("/{}", file_path)
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
            format!("/{}", dir_path)
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
            format!("/{}", file_path)
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
}
