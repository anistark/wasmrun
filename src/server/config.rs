use std::fs;
use std::net::TcpListener;
use std::path::Path;

use crate::compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel, TargetType};
use crate::error::{Result, ServerError, WasmrunError};
use crate::utils::wasm_analysis::{ProjectAnalysis, WasmAnalysis};
use crate::utils::CommandExecutor;

use super::wasm;

#[derive(Debug)]
#[allow(dead_code)]
pub struct ServerConfig {
    pub wasm_path: String,
    pub js_path: Option<String>,
    pub port: u16,
    pub watch_mode: bool,
    pub project_path: Option<String>,
    pub output_dir: Option<String>,
}

#[allow(dead_code)]
pub fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}

pub fn find_wasm_files(dir_path: &Path) -> Vec<String> {
    let mut wasm_files = Vec::new();

    if dir_path.is_dir() {
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension.to_string_lossy().to_lowercase() == "wasm" {
                            if let Some(file_name) = path.to_str() {
                                wasm_files.push(file_name.to_string());
                            }
                        }
                    }
                } else if path.is_dir() {
                    let mut sub_wasm_files = find_wasm_files(&path);
                    wasm_files.append(&mut sub_wasm_files);
                }
            }
        }
    }

    wasm_files
}

pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("0.0.0.0:{port}")).is_ok()
}

#[allow(dead_code)]
pub fn check_assets_directory() {
    if let Ok(metadata) = fs::metadata("./assets") {
        if metadata.is_dir() {
            eprintln!("📁 The assets directory exists, but the specific file wasn't found");
        } else {
            eprintln!("❌ Found 'assets' but it's not a directory!");
        }
    } else {
        eprintln!("❌ The assets directory doesn't exist at the expected location!");
    }
}

#[allow(dead_code)]
pub fn determine_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

pub struct ServerInfo {
    pub url: String,
    pub port: u16,
    pub server_pid: u32,
    pub watch_mode: bool,
    pub content_type: ContentType,
}

#[derive(Debug)]
pub enum ContentType {
    WasmFile(WasmAnalysis),
    #[allow(dead_code)]
    Project(ProjectAnalysis),
}

impl ServerInfo {
    pub fn for_wasm_file(wasm_path: &str, port: u16, watch_mode: bool) -> Result<Self> {
        let analysis = WasmAnalysis::analyze(wasm_path)?;

        Ok(Self {
            url: format!("http://localhost:{port}"),
            port,
            server_pid: std::process::id(),
            watch_mode,
            content_type: ContentType::WasmFile(analysis),
        })
    }

    #[allow(dead_code)]
    pub fn for_project(project_path: &str, port: u16, watch_mode: bool) -> Result<Self> {
        let analysis = ProjectAnalysis::analyze(project_path)?;
        let content_type = ContentType::Project(analysis);

        Ok(Self {
            url: format!("http://localhost:{port}"),
            port,
            server_pid: std::process::id(),
            watch_mode,
            content_type,
        })
    }

    pub fn print_server_startup(&self) {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  🌐 \x1b[1;36mWasmrun Server Started\x1b[0m                               \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );

        match &self.content_type {
            ContentType::WasmFile(analysis) => {
                analysis.print_analysis();
            }
            ContentType::Project(analysis) => {
                analysis.print_analysis();
            }
        }

        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🌐 \x1b[1;34mURL:\x1b[0m \x1b[1;32m{:<53}\x1b[0m \x1b[1;34m│\x1b[0m", self.url);
        println!("\x1b[1;34m│\x1b[0m  🔌 \x1b[1;34mPort:\x1b[0m \x1b[1;33m{:<52}\x1b[0m \x1b[1;34m│\x1b[0m", self.port);
        println!("\x1b[1;34m│\x1b[0m  🆔 \x1b[1;34mPID:\x1b[0m \x1b[1;37m{:<53}\x1b[0m \x1b[1;34m│\x1b[0m", self.server_pid);

        if self.watch_mode {
            println!("\x1b[1;34m│\x1b[0m  👁️  \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mEnabled\x1b[0m                              \x1b[1;34m│\x1b[0m");
        }

        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  💡 \x1b[1;37mPress Ctrl+C to stop the server\x1b[0m                        \x1b[1;34m│\x1b[0m");
        println!("\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m\n");
    }
}

pub struct ServerUtils;

impl ServerUtils {
    #[allow(dead_code)]
    pub fn handle_port_conflict(requested_port: u16) -> Result<u16> {
        if is_port_available(requested_port) {
            return Ok(requested_port);
        }

        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ⚠️  \x1b[1;33mPort {requested_port} is already in use\x1b[0m");
        println!("  🔍 \x1b[0;37mSearching for available port...\x1b[0m");

        for port in (requested_port + 1)..=(requested_port + 100) {
            if is_port_available(port) {
                println!("  ✅ \x1b[1;32mUsing port {port} instead\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m\n");
                return Ok(port);
            }
        }

        Err(WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!(
                "No available ports in range {}-{}",
                requested_port,
                requested_port + 100
            ),
        }))
    }

    pub fn print_initial_project_detection(project_path: &str) {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  🔍 \x1b[1;34mAnalyzing project:\x1b[0m \x1b[1;33m{project_path}\x1b[0m");

        let lang = crate::compiler::detect_project_language(project_path);

        match crate::plugin::manager::PluginManager::new() {
            Ok(plugin_manager) => {
                if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
                    println!(
                        "  🔌 \x1b[1;34mPlugin:\x1b[0m \x1b[1;32m{} v{}\x1b[0m",
                        plugin.info().name,
                        plugin.info().version
                    );

                    if matches!(
                        plugin.info().plugin_type,
                        crate::plugin::PluginType::External
                    ) {
                        println!("  📦 \x1b[1;34mType:\x1b[0m \x1b[1;36mExternal Plugin\x1b[0m");
                    } else {
                        println!("  📦 \x1b[1;34mType:\x1b[0m \x1b[1;35mBuilt-in Plugin\x1b[0m");
                    }
                } else {
                    match lang {
                        crate::compiler::ProjectLanguage::Rust => {
                            println!("\n  ⚠️  \x1b[1;33mRust plugin not found\x1b[0m");
                            println!("  💡 \x1b[1;33mInstall the wasmrust plugin:\x1b[0m");
                            println!("     \x1b[1;37mwasmrun plugin install wasmrust\x1b[0m");
                            println!("\n  ℹ️  \x1b[1;34mAfter installation, wasmrust will be auto-detected\x1b[0m");
                            println!("\x1b[1;34m╰\x1b[0m\n");
                            return;
                        }
                        crate::compiler::ProjectLanguage::C
                        | crate::compiler::ProjectLanguage::Asc
                        | crate::compiler::ProjectLanguage::Python => {
                            println!("  🔧 \x1b[1;34mUsing built-in plugin\x1b[0m");
                        }
                        _ => {}
                    }
                }

                let (builtin_count, external_count, _enabled_count) =
                    plugin_manager.plugin_counts();
                if external_count > 0 {
                    println!(
                        "  📊 \x1b[1;34mPlugins:\x1b[0m {builtin_count} built-in, {external_count} external"
                    );
                }
            }
            Err(e) => {
                eprintln!("  ⚠️ Warning: Failed to initialize plugin manager: {e}");
            }
        }

        let temp_dir = std::env::temp_dir().join("wasmrun_temp");
        let temp_output_dir = temp_dir.to_str().unwrap_or("/tmp").to_string();

        if !temp_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&temp_dir) {
                println!("  ❌ \x1b[1;31mFailed to create temporary directory: {e}\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m");
                return;
            }
        }

        println!("  📁 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{temp_output_dir}\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");

        if matches!(
            lang,
            crate::compiler::ProjectLanguage::C
                | crate::compiler::ProjectLanguage::Asc
                | crate::compiler::ProjectLanguage::Python
        ) {
            crate::compiler::print_system_info();
            let os = crate::compiler::detect_operating_system();
            let missing_tools = crate::compiler::get_missing_tools(&lang, &os);
            if !missing_tools.is_empty() {
                println!("\n\x1b[1;34m╭\x1b[0m");
                println!("  ⚠️  \x1b[1;33mMissing Required Tools:\x1b[0m");
                for tool in &missing_tools {
                    println!("     \x1b[1;31m• {tool}\x1b[0m");
                }
                println!(
                    "\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m"
                );
                println!("\x1b[1;34m╰\x1b[0m\n");
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_file_info(path: &str) -> Result<FileInfo> {
        let path_obj = Path::new(path);
        let metadata = fs::metadata(path)?;

        let filename = path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let absolute_path = fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        let file_size_bytes = metadata.len();
        let file_size = CommandExecutor::format_file_size(file_size_bytes);

        Ok(FileInfo {
            filename,
            absolute_path,
            file_size,
            file_size_bytes,
        })
    }

    #[allow(dead_code)]
    pub fn check_port_availability(port: u16) -> PortStatus {
        if is_port_available(port) {
            PortStatus::Available
        } else {
            let alternatives = (port + 1..port + 10).find(|&p| is_port_available(p));
            PortStatus::Unavailable {
                alternative: alternatives,
            }
        }
    }
}

pub fn setup_project_compilation(
    project_path: &str,
    language_override: Option<String>,
    _watch: bool,
) -> Option<(crate::compiler::ProjectLanguage, String)> {
    ServerUtils::print_initial_project_detection(project_path);

    let lang = if let Some(lang_override) = language_override {
        match lang_override.to_lowercase().as_str() {
            "rust" | "rs" => crate::compiler::ProjectLanguage::Rust,
            "c" | "cpp" | "c++" => crate::compiler::ProjectLanguage::C,
            "asc" | "assemblyscript" => crate::compiler::ProjectLanguage::Asc,
            "python" | "py" => crate::compiler::ProjectLanguage::Python,
            _ => {
                println!("⚠️  Unknown language override: {lang_override}");
                crate::compiler::detect_project_language(project_path)
            }
        }
    } else {
        crate::compiler::detect_project_language(project_path)
    };

    let temp_dir = std::env::temp_dir().join("wasmrun_temp");
    let temp_output_dir = temp_dir.to_str().unwrap_or("/tmp").to_string();

    if !temp_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            println!("❌ Failed to create temporary directory: {e}");
            return None;
        }
    }

    Some((lang, temp_output_dir))
}

pub fn compile_project(
    project_path: &str,
    output_dir: &str,
    lang: crate::compiler::ProjectLanguage,
    _watch: bool,
) -> Option<(String, bool, Option<String>)> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        optimization_level: OptimizationLevel::Release,
        verbose: false,
        watch: false,
        target_type: TargetType::Standard,
    };

    let builder = BuilderFactory::create_builder(&lang);
    match builder.build(&config) {
        Ok(result) => {
            println!("✅ Compilation successful!");
            println!("📦 WASM file: {}", result.wasm_path);
            if let Some(ref js_path) = result.js_path {
                println!("📦 JS file: {js_path}");
            }
            Some((result.wasm_path, result.is_wasm_bindgen, result.js_path))
        }
        Err(e) => {
            println!("❌ Compilation failed: {e}");
            None
        }
    }
}

pub fn run_server(config: ServerConfig) -> Result<()> {
    if super::is_server_running() {
        match super::stop_existing_server() {
            Ok(_) => println!("💀 Existing server stopped successfully."),
            Err(e) => eprintln!("❗ Warning when stopping existing server: {e}"),
        }
    }

    if !is_port_available(config.port) {
        return Err(WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Port {} is already in use", config.port),
        }));
    }

    let path_obj = Path::new(&config.wasm_path);
    if !path_obj.exists() {
        return Err(WasmrunError::path(format!(
            "Path not found: {}",
            config.wasm_path
        )));
    }

    if path_obj.is_dir() {
        let wasm_files = find_wasm_files(path_obj);
        if wasm_files.is_empty() {
            return Err(WasmrunError::path(format!(
                "No WASM files found in directory: {}",
                config.wasm_path
            )));
        }
        if wasm_files.len() == 1 {
            println!("🔍 Found a single WASM file: {}", wasm_files[0]);
            let mut new_config = config;
            new_config.wasm_path = wasm_files[0].clone();
            return run_server(new_config);
        } else {
            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  🔍 \x1b[1;36mMultiple WASM files found:\x1b[0m\n");
            for (i, file) in wasm_files.iter().enumerate() {
                println!("  {}. \x1b[1;33m{}\x1b[0m", i + 1, file);
            }
            println!("\n  \x1b[1;34mPlease specify which file to run:\x1b[0m");
            println!("  \x1b[1;37mwasmrun --wasm --path <filename.wasm>\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m");
            return Err(WasmrunError::path(
                "Please select a specific WASM file to run".to_string(),
            ));
        }
    }

    if !path_obj.is_file() {
        return Err(WasmrunError::path(format!(
            "Not a file: {}",
            config.wasm_path
        )));
    }
    if path_obj
        .extension()
        .map_or(true, |ext| ext.to_string_lossy().to_lowercase() != "wasm")
    {
        return Err(WasmrunError::path(format!(
            "Not a WASM file: {}",
            config.wasm_path
        )));
    }

    let wasm_filename = path_obj
        .file_name()
        .ok_or_else(|| WasmrunError::path("Invalid path".to_string()))?
        .to_string_lossy()
        .to_string();

    let server_info = ServerInfo::for_wasm_file(&config.wasm_path, config.port, config.watch_mode)?;
    server_info.print_server_startup();

    wasm::serve_wasm_file(&config.wasm_path, config.port, &wasm_filename).map_err(|e| {
        WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Server startup failed: {e}"),
        })
    })
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct FileInfo {
    pub filename: String,
    pub absolute_path: String,
    pub file_size: String,
    pub file_size_bytes: u64,
}

#[derive(Debug)]
pub enum PortStatus {
    Available,
    #[allow(dead_code)]
    Unavailable {
        alternative: Option<u16>,
    },
}
