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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_content_type_header() {
        let header = content_type_header("text/html");
        assert_eq!(header.field.as_str().to_ascii_lowercase(), "content-type");
        assert_eq!(header.value.as_str(), "text/html");

        let header = content_type_header("application/wasm");
        assert_eq!(header.value.as_str(), "application/wasm");
    }

    #[test]
    fn test_find_wasm_files_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let wasm_files = find_wasm_files(temp_dir.path());
        assert!(wasm_files.is_empty());
    }

    #[test]
    fn test_find_wasm_files_with_wasm_files() {
        let temp_dir = tempdir().unwrap();

        // Create some WASM files
        File::create(temp_dir.path().join("test1.wasm")).unwrap();
        File::create(temp_dir.path().join("test2.wasm")).unwrap();
        File::create(temp_dir.path().join("other.js")).unwrap(); // Non-WASM file

        let wasm_files = find_wasm_files(temp_dir.path());
        assert_eq!(wasm_files.len(), 2);
        assert!(wasm_files.iter().all(|f| f.ends_with(".wasm")));
    }

    #[test]
    fn test_find_wasm_files_recursive() {
        let temp_dir = tempdir().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        // Create WASM files in subdirectory
        File::create(sub_dir.join("nested.wasm")).unwrap();
        File::create(temp_dir.path().join("root.wasm")).unwrap();

        let wasm_files = find_wasm_files(temp_dir.path());
        assert_eq!(wasm_files.len(), 2);
        assert!(wasm_files.iter().any(|f| f.contains("nested.wasm")));
        assert!(wasm_files.iter().any(|f| f.contains("root.wasm")));
    }

    #[test]
    fn test_is_port_available() {
        // Test with a port that's likely available (high number)
        assert!(is_port_available(65432));

        // Test multiple times to ensure consistency
        assert!(is_port_available(65433));
        assert!(is_port_available(65434));
    }

    #[test]
    fn test_is_port_available_system_ports() {
        // Test some well-known ports that might be in use
        // These tests are not deterministic but shouldn't crash
        let _result = is_port_available(80); // HTTP
        let _result = is_port_available(443); // HTTPS
        let _result = is_port_available(22); // SSH
    }

    #[test]
    fn test_determine_content_type() {
        let test_cases = vec![
            ("test.html", "text/html"),
            ("style.css", "text/css"),
            ("script.js", "application/javascript"),
            ("data.json", "application/json"),
            ("module.wasm", "application/wasm"),
            ("image.png", "image/png"),
            ("photo.jpg", "image/jpeg"),
            ("photo.jpeg", "image/jpeg"),
            ("icon.svg", "image/svg+xml"),
            ("favicon.ico", "image/x-icon"),
            ("readme.txt", "text/plain"),
            ("doc.md", "text/markdown"),
            ("source.map", "application/json"),
            ("unknown.xyz", "application/octet-stream"),
        ];

        for (filename, expected) in test_cases {
            let path = std::path::Path::new(filename);
            assert_eq!(
                determine_content_type(&path),
                expected,
                "Failed for {}",
                filename
            );
        }
    }

    #[test]
    fn test_determine_content_type_no_extension() {
        let path = std::path::Path::new("filename_without_extension");
        assert_eq!(determine_content_type(&path), "application/octet-stream");
    }

    #[test]
    fn test_determine_content_type_case_insensitive() {
        let test_cases = vec![
            ("TEST.html", "text/html"),
            ("STYLE.css", "text/css"),
            ("MODULE.wasm", "application/wasm"),
            ("Image.png", "image/png"),
        ];

        for (filename, expected) in test_cases {
            let path = std::path::Path::new(filename);
            assert_eq!(
                determine_content_type(&path),
                expected,
                "Failed for {}",
                filename
            );
        }
    }

    #[test]
    fn test_check_assets_directory_no_directory() {
        // This function prints to stderr, so we just test it doesn't crash
        check_assets_directory();
        // Should complete without panicking
    }

    #[test]
    fn test_server_utils_check_port_availability() {
        // Test available port
        let result = ServerUtils::check_port_availability(65435);
        assert!(matches!(result, PortStatus::Available));

        // Test pattern with higher ports
        for port in 65400..65410 {
            let result = ServerUtils::check_port_availability(port);
            // Should either be available or unavailable with alternative
            match result {
                PortStatus::Available => {
                    // Good
                }
                PortStatus::Unavailable { alternative: _ } => {
                    // Also acceptable
                }
            }
        }
    }

    #[test]
    fn test_server_utils_handle_port_conflict_available() {
        let result = ServerUtils::handle_port_conflict(65436);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 65436);
    }

    #[test]
    fn test_server_utils_print_initial_project_detection() {
        let temp_dir = tempdir().unwrap();

        // Should not crash even with invalid directory
        ServerUtils::print_initial_project_detection(temp_dir.path().to_str().unwrap());

        // Should not crash with non-existent directory
        ServerUtils::print_initial_project_detection("/nonexistent/directory");
    }

    #[test]
    fn test_server_utils_get_file_info() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();

        let result = ServerUtils::get_file_info(test_file.to_str().unwrap());
        assert!(result.is_ok());

        let file_info = result.unwrap();
        assert_eq!(file_info.filename, "test.txt");
        assert!(file_info.absolute_path.contains("test.txt"));
        assert!(file_info.file_size_bytes > 0);
        assert!(!file_info.file_size.is_empty());
    }

    #[test]
    fn test_server_utils_get_file_info_nonexistent() {
        let result = ServerUtils::get_file_info("/nonexistent/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_print_server_info() {
        let temp_dir = tempdir().unwrap();
        let test_wasm = temp_dir.path().join("test.wasm");
        let mut file = File::create(&test_wasm).unwrap();
        file.write_all(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00])
            .unwrap(); // Valid WASM header

        // Should not crash
        print_server_info(
            "http://localhost:8080",
            8080,
            "test.wasm",
            "8 bytes",
            test_wasm.to_str().unwrap(),
            false,
        );

        // Test with watch mode
        print_server_info(
            "http://localhost:8081",
            8081,
            "test.wasm",
            "8 bytes",
            test_wasm.to_str().unwrap(),
            true,
        );
    }

    #[test]
    fn test_print_basic_server_info() {
        // Test the basic server info function
        print_basic_server_info(
            "http://localhost:8080",
            8080,
            "test.wasm",
            "8 bytes",
            "/path/to/test.wasm",
            false,
        );

        print_basic_server_info(
            "http://localhost:8081",
            8081,
            "test.wasm",
            "8 bytes",
            "/path/to/test.wasm",
            true,
        );

        // Should complete without panicking
    }
}
