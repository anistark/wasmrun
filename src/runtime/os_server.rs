use crate::config::project::PROJECT_CONFIG_FILE;
use crate::config::ProjectConfig;
use crate::error::{Result, WasmrunError};
use crate::logging::{LogEntry, LogSource, LogTrailSystem};
use crate::runtime::multilang_kernel::{MultiLanguageKernel, OsRunConfig};
use crate::runtime::network::NetworkServer;
use crate::runtime::project_files::ProjectFilesCollector;
use crate::runtime::runtime_cache::RuntimeCache;
use crate::runtime::tunnel::bore::{self, BoreClient, BoreServer, TunnelStatus};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server};
use wasmnet::policy::{NetworkPolicy, Policy, PolicyConfig};

const TEMPLATE_INDEX_HTML: &str = include_str!("../../templates/os/index.html");
const TEMPLATE_OS_JS: &str = include_str!("../../templates/os/os.js");
const TEMPLATE_INDEX_CSS: &str = include_str!("../../templates/os/index.css");
const TEMPLATE_LOGGING_JS: &str = include_str!("../../templates/os/logging.js");
const TEMPLATE_LOGS_HTML: &str = include_str!("../../templates/os/logs.html");

/// The proxy binds loopback like the OS server does: the browser reaching it
/// is the same machine, and a socket bridge on a routable address would be an
/// open proxy.
const NETWORK_HOST: &str = "127.0.0.1";

const ASSET_LOGO_PNG: &[u8] = include_bytes!("../../templates/assets/logo.png");
const ASSET_LOGO_TEXT_PNG: &[u8] = include_bytes!("../../templates/assets/logo-text.png");

/// OS Mode server providing the browser-based development interface
pub struct OsServer {
    kernel: Arc<RwLock<MultiLanguageKernel>>,
    config: OsRunConfig,
    project_pid: Arc<RwLock<Option<u32>>>,
    template_cache: HashMap<String, String>,
    log_system: Arc<LogTrailSystem>,
    /// The public tunnel, while `--expose` has one open.
    tunnel_client: Arc<RwLock<Option<BoreClient>>>,
    /// The program's port as the page last reported it. Kept apart from the
    /// client so a tunnel restarted from the UI picks it up.
    tunnel_target: Arc<RwLock<Option<u16>>>,
    /// The port this server is listening on, set by `start`.
    os_port: u16,
    runtime_cache: RuntimeCache,
    cors_origin: String,
    /// The wasmnet proxy's port, once it is up. `None` until `start` runs it,
    /// and if it failed to start: OS mode still serves everything else.
    network_port: Arc<RwLock<Option<u16>>>,
    /// The policy's `bind_ports`, so the page can pick a port the proxy will
    /// accept before it asks
    network_bind_ports: Arc<RwLock<String>>,
}

impl OsServer {
    pub fn new(kernel: MultiLanguageKernel, config: OsRunConfig) -> Result<Self> {
        let log_system = kernel.log_system();
        let cors_origin = if config.allow_cors {
            "*".to_string()
        } else {
            format!("http://127.0.0.1:{}", config.port.unwrap_or(8420))
        };
        let runtime_cache = RuntimeCache::new()?;
        let os_port = config.port.unwrap_or(8420);
        let mut server = Self {
            kernel: Arc::new(RwLock::new(kernel)),
            config,
            project_pid: Arc::new(RwLock::new(None)),
            template_cache: HashMap::new(),
            log_system,
            tunnel_client: Arc::new(RwLock::new(None)),
            tunnel_target: Arc::new(RwLock::new(None)),
            os_port,
            runtime_cache,
            cors_origin,
            network_port: Arc::new(RwLock::new(None)),
            network_bind_ports: Arc::new(RwLock::new(String::new())),
        };

        // Load and process templates
        server.load_templates()?;

        Ok(server)
    }

    /// Load OS mode templates from embedded data and process variables
    fn load_templates(&mut self) -> Result<()> {
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

        let port_str = self.config.port.unwrap_or(8420).to_string();

        let index_content = TEMPLATE_INDEX_HTML
            .replace("$PROJECT_NAME$", &project_name)
            .replace("$LANGUAGE$", language)
            .replace("$PROJECT_PATH$", &self.config.project_path)
            .replace("$PORT$", &port_str)
            .replace(
                "<!-- @style-placeholder -->",
                "<link rel=\"stylesheet\" href=\"/index.css\">",
            )
            .replace(
                "<!-- @script-placeholder -->",
                "<script src=\"/os.js\"></script>",
            );

        self.template_cache
            .insert("index.html".to_string(), index_content);
        self.template_cache
            .insert("os.js".to_string(), TEMPLATE_OS_JS.to_string());
        self.template_cache
            .insert("index.css".to_string(), TEMPLATE_INDEX_CSS.to_string());
        self.template_cache
            .insert("logging.js".to_string(), TEMPLATE_LOGGING_JS.to_string());
        self.template_cache.insert(
            "logs.html".to_string(),
            TEMPLATE_LOGS_HTML.replace("$PORT$", &port_str),
        );

        self.log_system.log(LogEntry::info(
            LogSource::Kernel,
            "OS mode templates loaded",
        ));
        println!("✅ OS mode templates loaded");
        Ok(())
    }

    fn cors_header(&self) -> Header {
        Header::from_bytes(
            &b"Access-Control-Allow-Origin"[..],
            self.cors_origin.as_bytes(),
        )
        .unwrap()
    }

    /// A response for anything the browser treats as a document: the OS page,
    /// the logs page, and whatever the dev server hands the Application
    /// panel's iframe. All of them carry the cross-origin isolation headers,
    /// since browsers only enable `SharedArrayBuffer` on an isolated page and
    /// the VM worker blocks on one. An iframe has to send them too, even
    /// same-origin, or the isolated parent refuses to load it. Scripts,
    /// styles and API responses need nothing: they are all same-origin.
    fn document_response(body: String, content_type: &str) -> Response<Cursor<Vec<u8>>> {
        Response::from_string(body)
            .with_header(Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap())
            .with_header(
                Header::from_bytes(&b"Cross-Origin-Opener-Policy"[..], &b"same-origin"[..])
                    .unwrap(),
            )
            .with_header(
                Header::from_bytes(&b"Cross-Origin-Embedder-Policy"[..], &b"require-corp"[..])
                    .unwrap(),
            )
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
        self.os_port = port;

        // Read the policy before anything binds: a project that asked for one
        // and wrote it wrong should hear about it instead of watching a server
        // come up.
        let policy = self.network_policy()?;

        let server = Server::http(format!("127.0.0.1:{port}"))
            .map_err(|e| WasmrunError::from(format!("Failed to start HTTP server: {e}")))?;

        self.log_system.log(LogEntry::info(
            LogSource::Kernel,
            format!("OS Mode server listening on http://127.0.0.1:{port}"),
        ));
        println!("🌐 OS Mode server listening on http://127.0.0.1:{port}");

        // The proxy lives exactly as long as this call: dropping the handle
        // when the request loop ends stops it and joins its thread.
        let _network = self.start_network(port, policy);

        if self.config.expose {
            self.start_tunnel_at_boot();
        }

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

    /// Read the project's network policy from `wasmrun.toml`.
    ///
    /// This is the one part of starting the proxy that can fail the whole
    /// server: the file is the project saying which egress it wants, so a file
    /// that cannot be read is a policy that cannot be honored, and starting
    /// anyway would run the project under settings nobody chose.
    fn network_policy(&self) -> Result<PolicyConfig> {
        let config = ProjectConfig::load(&self.config.project_path)?;
        let policy = config.network_policy()?;

        if config.has_network_config() {
            self.log_system.log(LogEntry::info(
                LogSource::Kernel,
                format!("Network policy loaded from {PROJECT_CONFIG_FILE}"),
            ));
            println!("🔒 Network policy loaded from {PROJECT_CONFIG_FILE}");
        }

        Ok(policy)
    }

    /// Start the wasmnet proxy alongside the OS server.
    ///
    /// A proxy that will not start is reported and then left alone: everything
    /// in OS mode except sockets works without it, and failing the whole server
    /// over an unavailable port would be the worse trade. A policy that will
    /// not load is a different matter and is raised by the caller, since a
    /// project that asked for a policy must not run under a different one.
    fn start_network(&self, os_port: u16, policy: PolicyConfig) -> Option<NetworkServer> {
        let bind_ports = policy.network.bind_ports.clone();
        match NetworkServer::start(NETWORK_HOST, os_port, policy) {
            Ok(server) => {
                let network_port = server.port();
                *self.network_port.write().unwrap() = Some(network_port);
                *self.network_bind_ports.write().unwrap() = bind_ports;
                self.log_system.log(LogEntry::info(
                    LogSource::Kernel,
                    format!("Network proxy listening on {}", server.url(NETWORK_HOST)),
                ));
                println!("🔌 Network proxy listening on {}", server.url(NETWORK_HOST));
                Some(server)
            }
            Err(e) => {
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Network proxy unavailable: {e}"),
                ));
                eprintln!("⚠️ Network proxy unavailable, sockets will not work: {e}");
                None
            }
        }
    }

    /// Start the project in the kernel.
    /// Called once during server boot — uses a write lock on project_pid internally.
    fn start_project(&self) -> Result<()> {
        let mut project_pid = self.project_pid.write().unwrap();
        match self.run_project_in_kernel() {
            Ok(pid) => {
                *project_pid = Some(pid);
                Ok(())
            }
            Err(e) => {
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Failed to start project in kernel: {e}"),
                ));
                eprintln!("⚠️ Failed to start project in kernel: {e}");
                Ok(())
            }
        }
    }

    /// Core project startup logic. Acquires the kernel write lock, mounts the
    /// project, and runs it. Returns the new PID on success.
    /// Does NOT touch project_pid — callers are responsible for that.
    fn run_project_in_kernel(&self) -> Result<u32> {
        let mut kernel = self.kernel.write().unwrap();

        if let Err(e) = kernel.mount_project(&self.config.project_path) {
            eprintln!("⚠️ Failed to mount project directory: {e}");
        }

        match kernel.auto_detect_and_run(self.config.clone()) {
            Ok(pid) => {
                self.log_system.log(
                    LogEntry::info(
                        LogSource::Kernel,
                        format!("Project started with PID: {pid}"),
                    )
                    .with_pid(pid),
                );
                println!("✅ Project started with PID: {pid}");
                Ok(pid)
            }
            Err(e) => Err(WasmrunError::from(e.to_string())),
        }
    }

    /// Handle HTTP requests
    fn handle_request(&self, request: Request) -> Result<()> {
        let method = request.method().clone();
        let url = request.url().to_string();

        match (method, url.as_str()) {
            (Method::Options, _) => {
                let response = Response::from_string("")
                    .with_header(self.cors_header())
                    .with_header(
                        Header::from_bytes(
                            &b"Access-Control-Allow-Methods"[..],
                            &b"GET, POST, DELETE, OPTIONS"[..],
                        )
                        .unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(
                            &b"Access-Control-Allow-Headers"[..],
                            &b"Content-Type"[..],
                        )
                        .unwrap(),
                    )
                    .with_status_code(tiny_http::StatusCode(204));
                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }

            // Serve the main OS interface
            (Method::Get, "/") => {
                if let Some(content) = self.template_cache.get("index.html") {
                    let response =
                        Self::document_response(content.clone(), "text/html; charset=utf-8");
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
                    let response =
                        Self::document_response(content.clone(), "text/html; charset=utf-8");
                    request
                        .respond(response)
                        .map_err(|e| WasmrunError::from(e.to_string()))?;
                } else {
                    self.send_404(request)?;
                }
            }

            // Sockets do not go through this server: tiny_http has no upgrade
            // path, so the browser shim talks to the wasmnet proxy on its own
            // port. `/api/network/status` says where.
            (Method::Get, "/ws") => {
                let response = Response::from_string(
                    "Sockets are proxied by wasmnet on its own port. \
                     GET /api/network/status for the URL.",
                )
                .with_status_code(tiny_http::StatusCode(410))
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap())
                .with_header(self.cors_header());
                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }

            // Where the browser shim connects its sockets
            (Method::Get, "/api/network/status") => {
                self.handle_network_status_request(request)?;
            }

            // API endpoint for runtime binary (serves cached wasmhub runtime)
            (Method::Get, path) if path.starts_with("/api/runtime/") => {
                let language = &path[13..]; // Remove "/api/runtime/"
                self.handle_runtime_request(request, language)?;
            }

            // API endpoint for available runtimes manifest
            (Method::Get, "/api/runtimes") => {
                self.handle_runtimes_list_request(request)?;
            }

            // API endpoint for project files bundle (base64-encoded)
            (Method::Get, "/api/project/files") => {
                self.handle_project_files_request(request)?;
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

            // Tunnel API endpoints. `--expose` opens the tunnel; the page
            // points it at the program's port with `target`
            (Method::Post, "/api/tunnel/target") => {
                self.handle_tunnel_target_request(request)?;
            }

            (Method::Post, "/api/tunnel/start") => {
                self.handle_tunnel_start_request(request)?;
            }

            (Method::Get, "/api/tunnel/status") => {
                self.handle_tunnel_status_request(request)?;
            }

            (Method::Post, "/api/tunnel/stop") => {
                self.handle_tunnel_stop_request(request)?;
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

    /// Handle start project request.
    /// Check-and-start is atomic under a single project_pid write lock.
    fn handle_start_project(&self, request: Request) -> Result<()> {
        let mut project_pid = self.project_pid.write().unwrap();

        let response_json = if project_pid.is_some() {
            serde_json::json!({ "success": false, "error": "Project is already running" })
        } else {
            match self.run_project_in_kernel() {
                Ok(pid) => {
                    *project_pid = Some(pid);
                    serde_json::json!({ "success": true, "pid": pid })
                }
                Err(e) => {
                    serde_json::json!({ "success": false, "error": e.to_string() })
                }
            }
        };

        let status = if response_json["success"].as_bool() == Some(true) {
            200
        } else if project_pid.is_some() {
            409
        } else {
            500
        };

        let response = Response::from_string(response_json.to_string())
            .with_status_code(tiny_http::StatusCode(status))
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(self.cors_header());

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
        Ok(())
    }

    /// Handle restart project request.
    /// Kill-and-restart is atomic under a single project_pid write lock.
    fn handle_restart_project(&self, request: Request) -> Result<()> {
        let mut project_pid = self.project_pid.write().unwrap();

        if let Some(pid) = *project_pid {
            let mut kernel = self.kernel.write().unwrap();
            let _ = kernel.kill_process(pid);
        }
        *project_pid = None;

        let response_json = match self.run_project_in_kernel() {
            Ok(pid) => {
                *project_pid = Some(pid);
                serde_json::json!({ "success": true, "pid": pid })
            }
            Err(e) => {
                serde_json::json!({ "success": false, "error": e.to_string() })
            }
        };

        let status = if response_json["success"].as_bool() == Some(true) {
            200
        } else {
            500
        };

        let response = Response::from_string(response_json.to_string())
            .with_status_code(tiny_http::StatusCode(status))
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(self.cors_header());

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;
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
            .with_header(self.cors_header());

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
                .with_header(self.cors_header());

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
                        .with_header(self.cors_header());

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
                        .with_header(self.cors_header());

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

    fn handle_runtime_request(&self, request: Request, language: &str) -> Result<()> {
        if language.is_empty() || language.contains("..") || language.contains('/') {
            return self.send_error(request, "Invalid language identifier");
        }

        let wasmhub_lang = crate::runtime::runtime_cache::wasmhub_language(language);

        match self.runtime_cache.get_runtime(wasmhub_lang) {
            Ok(wasm_bytes) => {
                self.log_system.log(LogEntry::info(
                    LogSource::Kernel,
                    format!("Serving {language} runtime ({} bytes)", wasm_bytes.len()),
                ));

                let response = Response::from_data(wasm_bytes)
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/wasm"[..]).unwrap(),
                    )
                    .with_header(
                        Header::from_bytes(&b"Cache-Control"[..], &b"public, max-age=86400"[..])
                            .unwrap(),
                    )
                    .with_header(self.cors_header());

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Failed to fetch {language} runtime: {e}"),
                ));

                let error_json = serde_json::json!({
                    "success": false,
                    "error": format!("Runtime not available for '{language}': {e}")
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(404))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(self.cors_header());

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        }

        Ok(())
    }

    fn handle_runtimes_list_request(&self, request: Request) -> Result<()> {
        let detected_language = self.detect_project_language()?;
        let wasmhub_lang = crate::runtime::runtime_cache::wasmhub_language(&detected_language);

        let mut response_json = serde_json::json!({
            "detected_language": detected_language,
            "wasmhub_runtime": wasmhub_lang,
            "cached": self.runtime_cache.is_cached(wasmhub_lang),
            "cached_version": self.runtime_cache.cached_version(wasmhub_lang),
        });

        if let Ok(manifest) = self.runtime_cache.fetch_manifest() {
            response_json["wasmhub_version"] = serde_json::Value::String(manifest.version);
            response_json["available_languages"] =
                serde_json::json!(manifest.languages.keys().collect::<Vec<_>>());
        }

        let response = Response::from_string(response_json.to_string())
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(self.cors_header());

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

        Ok(())
    }

    fn handle_project_files_request(&self, request: Request) -> Result<()> {
        let collector = match ProjectFilesCollector::new(&self.config.project_path) {
            Ok(c) => c,
            Err(e) => {
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Failed to read project files: {e}"),
                ));
                return self.send_error(request, &format!("Failed to read project files: {e}"));
            }
        };

        match collector.collect() {
            Ok(bundle) => {
                self.log_system.log(LogEntry::info(
                    LogSource::Kernel,
                    format!(
                        "Serving {} project files ({} bytes)",
                        bundle.file_count, bundle.total_size
                    ),
                ));

                let response_json = serde_json::json!({
                    "success": true,
                    "files": bundle.files,
                    "file_count": bundle.file_count,
                    "total_size": bundle.total_size,
                    "project_path": bundle.project_path,
                    "skipped": bundle.skipped,
                });

                let response = Response::from_string(response_json.to_string())
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(self.cors_header());

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
            Err(e) => {
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Failed to collect project files: {e}"),
                ));

                let error_json = serde_json::json!({
                    "success": false,
                    "error": format!("Failed to collect project files: {e}")
                });

                let response = Response::from_string(error_json.to_string())
                    .with_status_code(tiny_http::StatusCode(500))
                    .with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                    )
                    .with_header(self.cors_header());

                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
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
            .with_header(self.cors_header());

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
                        let response = Self::document_response(content, &content_type);
                        request
                            .respond(response)
                            .map_err(|e| WasmrunError::from(e.to_string()))?;
                    }
                    Err(e) => {
                        let error_html = format!(
                            "<html><body><h1>Dev Server Error</h1><p>{e}</p></body></html>"
                        );
                        let response = Self::document_response(error_html, "text/html");
                        request
                            .respond(response)
                            .map_err(|e| WasmrunError::from(e.to_string()))?;
                    }
                }
            } else {
                let error_html = format!(
                    "<html><body><h1>No Dev Server</h1><p>No dev server running for PID {pid}</p></body></html>"
                );
                let response = Self::document_response(error_html, "text/html");
                request
                    .respond(response)
                    .map_err(|e| WasmrunError::from(e.to_string()))?;
            }
        } else {
            let error_html = "<html><body><h1>No Project Running</h1><p>No project is currently running</p></body></html>";
            let response = Self::document_response(error_html.to_string(), "text/html");
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

    /// Serve static assets from embedded data
    fn serve_asset(&self, request: Request, asset_path: &str) -> Result<()> {
        let (content, content_type): (&[u8], &str) = match asset_path {
            "logo.png" => (ASSET_LOGO_PNG, "image/png"),
            "logo-text.png" => (ASSET_LOGO_TEXT_PNG, "image/png"),
            _ => return self.send_404(request),
        };

        let response = Response::from_data(content.to_vec()).with_header(
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
            .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
                    .with_header(self.cors_header());

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
            .with_header(self.cors_header());

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
            .with_header(self.cors_header());

        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

        Ok(())
    }

    /// Report the wasmnet proxy's URL, or that it is not running.
    ///
    /// The browser shim reads this before it boots a module, since the proxy
    /// picks its port at startup and is not always on the default.
    fn handle_network_status_request(&self, request: Request) -> Result<()> {
        let port = *self.network_port.read().unwrap();

        let response_json = match port {
            Some(port) => serde_json::json!({
                "success": true,
                "enabled": true,
                "host": NETWORK_HOST,
                "port": port,
                "url": format!("ws://{NETWORK_HOST}:{port}"),
                "bind_ports": *self.network_bind_ports.read().unwrap(),
                "tunnel": self.config.expose,
            }),
            None => serde_json::json!({
                "success": true,
                "enabled": false,
                "reason": "the network proxy is not running",
            }),
        };

        let response = Response::from_string(response_json.to_string())
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(self.cors_header());
        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

        Ok(())
    }

    /// Open the tunnel `--expose` asked for, pointed at whatever port the
    /// program has reported so far. Returns the client's server address so
    /// the caller can say where it is connecting.
    fn start_tunnel(&self) -> std::result::Result<String, String> {
        let server = BoreServer::parse(
            self.config
                .tunnel_server
                .as_deref()
                .unwrap_or(bore::DEFAULT_SERVER),
        )?;
        let target = *self.tunnel_target.read().unwrap();
        let client = BoreClient::start(server, self.config.tunnel_secret.as_deref(), target)
            .map_err(|e| format!("could not start the tunnel thread: {e}"))?;
        let address = client.server().to_string();
        *self.tunnel_client.write().unwrap() = Some(client);
        Ok(address)
    }

    /// Open the tunnel at startup and say how it went. Waiting a few seconds
    /// here is what lets the public URL be printed next to the local one;
    /// past that the tunnel keeps trying on its own and the UI reports it.
    fn start_tunnel_at_boot(&self) {
        let address = match self.start_tunnel() {
            Ok(address) => address,
            Err(e) => {
                eprintln!("⚠️ Public tunnel not started: {e}");
                return;
            }
        };
        println!("🌍 Opening a public tunnel through {address}…");

        let guard = self.tunnel_client.read().unwrap();
        let Some(client) = guard.as_ref() else {
            return;
        };
        match client.wait_connected(Duration::from_secs(8)) {
            TunnelStatus::Connected => {
                let url = client.public_url().unwrap_or_default();
                self.log_system.log(LogEntry::info(
                    LogSource::Kernel,
                    format!("Public tunnel open at {url}"),
                ));
                println!("🌍 Public URL: {url}");
                println!("   It forwards to the program's port once the program listens");
            }
            status => {
                let reason = client
                    .last_error()
                    .unwrap_or_else(|| status.as_str().to_lowercase());
                self.log_system.log(LogEntry::error(
                    LogSource::Kernel,
                    format!("Public tunnel not up yet: {reason}"),
                ));
                eprintln!("⚠️ Public tunnel not up yet ({reason}); still retrying");
            }
        }
    }

    fn send_json(&self, request: Request, status: u16, body: serde_json::Value) -> Result<()> {
        let response = Response::from_string(body.to_string())
            .with_status_code(tiny_http::StatusCode(status))
            .with_header(
                Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
            )
            .with_header(self.cors_header());
        request
            .respond(response)
            .map_err(|e| WasmrunError::from(e.to_string()))
    }

    /// The tunnel endpoints change what the internet can reach, so only the
    /// OS page itself may call them. A browser always sends `Origin` on a
    /// cross-origin POST, which is what stops another site the user has open
    /// from asking this server to publish a port.
    fn tunnel_request_allowed(&self, request: &Request) -> bool {
        let origin = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Origin"))
            .map(|h| h.value.as_str().to_string());
        origin_allowed(origin.as_deref(), self.os_port)
    }

    fn tunnel_disabled(&self, request: Request) -> Result<()> {
        self.send_json(
            request,
            409,
            serde_json::json!({
                "success": false,
                "error": "the tunnel is off: start wasmrun os with --expose",
            }),
        )
    }

    fn tunnel_forbidden(&self, request: Request) -> Result<()> {
        self.send_json(
            request,
            403,
            serde_json::json!({
                "success": false,
                "error": "tunnel requests are only accepted from the OS mode page",
            }),
        )
    }

    fn tunnel_status_json(&self) -> serde_json::Value {
        if !self.config.expose {
            return serde_json::json!({
                "success": true,
                "enabled": false,
                "status": "Disabled",
            });
        }

        let target = *self.tunnel_target.read().unwrap();
        let guard = self.tunnel_client.read().unwrap();
        match guard.as_ref() {
            Some(client) => serde_json::json!({
                "success": true,
                "enabled": true,
                "status": client.status().as_str(),
                "server": client.server().to_string(),
                "public_url": client.public_url(),
                "public_port": client.public_port(),
                "target_port": target,
                "error": client.last_error(),
            }),
            None => serde_json::json!({
                "success": true,
                "enabled": true,
                "status": "Not started",
                "target_port": target,
            }),
        }
    }

    fn handle_tunnel_status_request(&self, request: Request) -> Result<()> {
        let body = self.tunnel_status_json();
        self.send_json(request, 200, body)
    }

    fn handle_tunnel_start_request(&self, request: Request) -> Result<()> {
        if !self.config.expose {
            return self.tunnel_disabled(request);
        }
        if !self.tunnel_request_allowed(&request) {
            return self.tunnel_forbidden(request);
        }
        if self.tunnel_client.read().unwrap().is_none() {
            if let Err(e) = self.start_tunnel() {
                return self.send_json(
                    request,
                    500,
                    serde_json::json!({ "success": false, "error": e }),
                );
            }
        }
        let body = self.tunnel_status_json();
        self.send_json(request, 200, body)
    }

    fn handle_tunnel_stop_request(&self, request: Request) -> Result<()> {
        if !self.config.expose {
            return self.tunnel_disabled(request);
        }
        if !self.tunnel_request_allowed(&request) {
            return self.tunnel_forbidden(request);
        }
        // Dropping the client closes the control connection, which is what
        // releases the public port on the server
        let client = self.tunnel_client.write().unwrap().take();
        drop(client);
        let body = self.tunnel_status_json();
        self.send_json(request, 200, body)
    }

    /// Where the tunnel forwards: the port the page bound for the program,
    /// or `null` when the program stops. The page is the only party that
    /// knows, since wasmnet binds the port on the page's behalf.
    fn handle_tunnel_target_request(&self, mut request: Request) -> Result<()> {
        if !self.config.expose {
            return self.tunnel_disabled(request);
        }
        if !self.tunnel_request_allowed(&request) {
            return self.tunnel_forbidden(request);
        }

        let mut content = String::new();
        if let Err(e) = std::io::Read::read_to_string(request.as_reader(), &mut content) {
            return self.send_error(request, &format!("Failed to read request body: {e}"));
        }
        let body: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => return self.send_error(request, &format!("Invalid JSON: {e}")),
        };

        let port = match body.get("port") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => match v.as_u64().and_then(|p| u16::try_from(p).ok()) {
                Some(p) => Some(p),
                None => return self.send_error(request, "port must be a number or null"),
            },
        };

        if let Some(port) = port {
            let network_port = *self.network_port.read().unwrap();
            let bind_ports = self.network_bind_ports.read().unwrap().clone();
            let checked = check_tunnel_target(port, self.os_port, network_port, &bind_ports)
                .and_then(|_| {
                    // The page says it bound this port; make sure something is
                    // there before sending the internet at it
                    std::net::TcpStream::connect_timeout(
                        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                        Duration::from_secs(1),
                    )
                    .map(|_| ())
                    .map_err(|e| format!("nothing is listening on 127.0.0.1:{port}: {e}"))
                });
            if let Err(e) = checked {
                return self.send_error(request, &e);
            }
        }

        *self.tunnel_target.write().unwrap() = port;
        if let Some(client) = self.tunnel_client.read().unwrap().as_ref() {
            client.set_target(port);
        }
        if let Some(port) = port {
            self.log_system.log(LogEntry::info(
                LogSource::Kernel,
                format!("Public tunnel now forwards to 127.0.0.1:{port}"),
            ));
        }

        let body = self.tunnel_status_json();
        self.send_json(request, 200, body)
    }
}

/// Whether a request carrying this `Origin` came from the OS page. No header
/// means no browser (curl, a script), which is the user at their own machine.
fn origin_allowed(origin: Option<&str>, os_port: u16) -> bool {
    match origin {
        None => true,
        Some(origin) => {
            origin == format!("http://127.0.0.1:{os_port}")
                || origin == format!("http://localhost:{os_port}")
        }
    }
}

/// Whether the tunnel may forward to `port`: one the policy lets the VM bind,
/// and never the OS server or the proxy, both of which sit inside the
/// default `bind_ports` range and would publish the dev environment itself.
fn check_tunnel_target(
    port: u16,
    os_port: u16,
    network_port: Option<u16>,
    bind_ports: &str,
) -> std::result::Result<(), String> {
    if port == os_port {
        return Err(format!(
            "port {port} is the OS mode server; the tunnel only carries the program's port"
        ));
    }
    if network_port == Some(port) {
        return Err(format!(
            "port {port} is the network proxy; the tunnel only carries the program's port"
        ));
    }
    if network_port.is_none() {
        return Err("the network proxy is not running, so the program has no port".to_string());
    }
    let policy = Policy::new(&NetworkPolicy {
        bind_ports: bind_ports.to_string(),
        ..NetworkPolicy::default()
    });
    policy.check_bind(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_os_page_may_steer_the_tunnel() {
        assert!(origin_allowed(None, 8420));
        assert!(origin_allowed(Some("http://127.0.0.1:8420"), 8420));
        assert!(origin_allowed(Some("http://localhost:8420"), 8420));
        assert!(!origin_allowed(Some("http://127.0.0.1:9999"), 8420));
        assert!(!origin_allowed(Some("https://example.com"), 8420));
        assert!(!origin_allowed(Some("null"), 8420));
    }

    #[test]
    fn tunnel_target_must_be_a_program_port() {
        let proxy = Some(8440);
        assert!(check_tunnel_target(3000, 8420, proxy, "3000-9999").is_ok());

        // The OS server and the proxy are inside the default range
        assert!(check_tunnel_target(8420, 8420, proxy, "3000-9999").is_err());
        assert!(check_tunnel_target(8440, 8420, proxy, "3000-9999").is_err());

        // Outside what the policy lets the VM bind
        assert!(check_tunnel_target(22, 8420, proxy, "3000-9999").is_err());
        assert!(check_tunnel_target(3001, 8420, proxy, "3000,8080").is_err());

        // No proxy, no program port
        assert!(check_tunnel_target(3000, 8420, None, "3000-9999").is_err());
    }
}
