use super::config::{FileInfo, PortStatus, ServerInfo};
use crate::error::Result;
use crate::utils::CommandExecutor;
use std::fs;
use std::net::TcpListener;
use std::path::Path;

/// Generate a Content-Type header
pub fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}

/// Find WASM files in a directory
#[allow(dead_code)] // TODO: Future WASM file discovery system
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
                    // Recursively check subdirectories
                    let mut sub_wasm_files = find_wasm_files(&path);
                    wasm_files.append(&mut sub_wasm_files);
                }
            }
        }
    }

    wasm_files
}

/// Check if the given port is available
pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("0.0.0.0:{port}")).is_ok()
}

/// Check if assets directory exists
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

/// Function to determine content type based on file extension
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

/// Utility functions for server operations
pub struct ServerUtils;

impl ServerUtils {
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

        use crate::utils::PathResolver;

        let temp_output_dir = match PathResolver::create_temp_directory("wasmrun_temp") {
            Ok(dir) => dir,
            Err(e) => {
                println!("  ❌ \x1b[1;31mFailed to create temporary directory: {e}\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m");
                return;
            }
        };

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

    #[allow(dead_code)] // TODO: Future file metadata system
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

    /// Check if a port is available
    pub fn check_port_availability(port: u16) -> PortStatus {
        if is_port_available(port) {
            PortStatus::Available
        } else {
            // Suggest alternative ports
            let alternatives = (port + 1..port + 10).find(|&p| is_port_available(p));

            PortStatus::Unavailable {
                alternative: alternatives,
            }
        }
    }

    /// Print a warning if the port is not available
    pub fn handle_port_conflict(port: u16) -> Result<u16> {
        match Self::check_port_availability(port) {
            PortStatus::Available => Ok(port),
            PortStatus::Unavailable { alternative } => {
                println!("\n⚠️  \x1b[1;33mPort {port} is already in use\x1b[0m");

                if let Some(alt_port) = alternative {
                    println!("🔄 \x1b[1;34mTrying alternative port: {alt_port}\x1b[0m");
                    Ok(alt_port)
                } else {
                    println!(
                        "❌ \x1b[1;31mNo alternative ports available in range {}-{}\x1b[0m",
                        port,
                        port + 10
                    );
                    Err(crate::error::WasmrunError::Server(
                        crate::error::ServerError::startup_failed(
                            port,
                            format!("Port {port} is in use and no alternatives found"),
                        ),
                    ))
                }
            }
        }
    }
}

/// Get Server Info
#[allow(dead_code)] // TODO: Future server information display
pub fn print_server_info(
    url: &str,
    port: u16,
    wasm_filename: &str,
    file_size: &str,
    absolute_path: &str,
    watch_mode: bool,
) {
    if let Ok(server_info) = ServerInfo::for_wasm_file(absolute_path, port, watch_mode) {
        server_info.print_server_startup();
    } else {
        // Basic output if analysis fails
        print_basic_server_info(
            url,
            port,
            wasm_filename,
            file_size,
            absolute_path,
            watch_mode,
        );
    }
}

/// Basic server info printing
#[allow(dead_code)] // TODO: Future basic server info display
fn print_basic_server_info(
    url: &str,
    port: u16,
    wasm_filename: &str,
    file_size: &str,
    absolute_path: &str,
    watch_mode: bool,
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🅦 \x1b[1;36mWasmrun WASM Server\x1b[0m\n");
    println!("  🚀 \x1b[1;34mServer URL:\x1b[0m \x1b[4;36m{url}\x1b[0m");
    println!("  🔌 \x1b[1;34mListening on port:\x1b[0m \x1b[1;33m{port}\x1b[0m");
    println!("  📦 \x1b[1;34mServing file:\x1b[0m \x1b[1;32m{wasm_filename}\x1b[0m");
    println!("  💾 \x1b[1;34mFile size:\x1b[0m \x1b[0;37m{file_size}\x1b[0m");
    println!("  🔍 \x1b[1;34mFull path:\x1b[0m \x1b[0;37m{absolute_path:.45}\x1b[0m");
    println!(
        "  ℹ️ \x1b[1;34mServer PID:\x1b[0m \x1b[0;37m{}\x1b[0m",
        std::process::id()
    );

    if watch_mode {
        println!("\n  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m");
    }

    println!("\n  \x1b[0;90mPress Ctrl+C to stop the server\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
    println!("\n🌐 Opening browser...");
}
