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
                &format!("<link rel=\"stylesheet\" href=\"/index.css\">"),
            );
        }

        let js_path = templates_dir.join("os.js");
        if js_path.exists() {
            index_content = index_content.replace(
                "<!-- @script-placeholder -->",
                &format!("<script src=\"/os.js\"></script>"),
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
        let server = Server::http(format!("127.0.0.1:{}", port))
            .map_err(|e| WasmrunError::from(format!("Failed to start HTTP server: {e}")))?;

        println!("🌐 OS Mode server listening on http://127.0.0.1:{}", port);

        // Start the project in the kernel
        self.start_project()?;

        // Handle HTTP requests
        for request in server.incoming_requests() {
            match self.handle_request(request) {
                Ok(_) => {}
                Err(e) => eprintln!("Request handling error: {}", e),
            }
        }

        Ok(())
    }

    /// Start the project in the kernel
    fn start_project(&mut self) -> Result<()> {
        let mut kernel = self.kernel.write().unwrap();

        match kernel.auto_detect_and_run(self.config.clone()) {
            Ok(pid) => {
                self.project_pid = Some(pid);
                println!("✅ Project started with PID: {}", pid);
                Ok(())
            }
            Err(e) => {
                eprintln!("⚠️ Failed to start project in kernel: {}", e);
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

            // Proxy requests to the user's project (running in kernel)
            (Method::Get, path) if path.starts_with("/project/") => {
                // Strip /project/ prefix and proxy to the actual project
                let project_path = &path[9..]; // Remove "/project/"
                self.proxy_to_project(request, project_path)?;
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
            "project_pid": self.project_pid
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
    fn proxy_to_project(&self, request: Request, path: &str) -> Result<()> {
        // For now, return a placeholder response
        // TODO: Implement actual proxying to the project's HTTP server
        let placeholder = format!(
            "
            <html>
                <head><title>Project Proxy</title></head>
                <body>
                    <h1>Project Running in Kernel</h1>
                    <p>Path: {}</p>
                    <p>PID: {:?}</p>
                    <p>This would proxy to your project's HTTP server.</p>
                    <p>Implementation in progress...</p>
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
}
