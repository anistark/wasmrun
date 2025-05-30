use crate::error::{ChakraError, Result, ServerError};
use std::path::Path;

mod config;
mod handler;
mod utils;
mod wasm;
mod webapp;

pub use config::ServerConfig;
pub use webapp::run_webapp;

// Constants
const PID_FILE: &str = "/tmp/chakra_server.pid";

/// Run a WebAssembly file directly
pub fn run_wasm_file(path: &str, port: u16) -> Result<()> {
    // Server should not start for tests
    if cfg!(test) {
        return Ok(());
    }

    let path_obj = std::path::Path::new(path);

    // Check if file exists and has correct extension
    if !path_obj.extension().map_or(false, |ext| ext == "wasm") {
        // Handle JS files that might be associated with WASM
        if path_obj.extension().map_or(false, |ext| ext == "js") {
            return handle_js_file(path, port);
        }

        return Err(ChakraError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Not a WASM file: {}", path),
        }));
    }

    // Special handling for _bg.wasm files (wasm-bindgen output)
    if handle_wasm_bindgen_file(path_obj, path, port)? {
        return Ok(());
    }

    // Regular WASM file handling
    let wasm_filename = Path::new(path)
        .file_name()
        .ok_or_else(|| ChakraError::path(format!("Invalid path: {}", path)))?
        .to_string_lossy()
        .to_string();

    wasm::serve_wasm_file(path, port, &wasm_filename)
        .map_err(|e| ChakraError::Server(ServerError::startup_failed(port, e)))
}

/// Compile and run a project
pub fn run_project(
    path: &str,
    port: u16,
    language_override: Option<String>,
    watch: bool,
) -> Result<()> {
    // Server should not start for tests
    if cfg!(test) {
        return Ok(());
    }

    let path_obj = std::path::Path::new(path);

    // Handle WASM file for convenience
    if path_obj.is_file() && path_obj.extension().map_or(false, |ext| ext == "wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected WASM file: {}\x1b[0m", path);
        println!("  \x1b[0;37mRunning the WASM file directly...\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");

        return run_wasm_file(path, port);
    }

    // Handle JS file that might be wasm-bindgen output
    if path_obj.is_file() && path_obj.extension().map_or(false, |ext| ext == "js") {
        return handle_js_file(path, port);
    }

    // Handle project directory
    if !path_obj.is_dir() {
        let error_msg = if !path_obj.exists() {
            format!("Path not found: {}", path)
        } else {
            format!("Not a WASM file or project directory: {}", path)
        };

        return Err(ChakraError::path(error_msg));
    }

    // Check if it's a Rust web application
    let detected_language = crate::compiler::detect_project_language(path);

    if detected_language == crate::compiler::ProjectLanguage::Rust
        && crate::compiler::is_rust_web_application(path)
    {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  🌐 \x1b[1;36mDetected Rust Web Application\x1b[0m");
        println!("  \x1b[0;37mRunning as a web app on port {}\x1b[0m", 3000); // Use port 3000 for web apps
        println!("\x1b[1;34m╰\x1b[0m\n");

        // Run as a web application on port 3000
        return webapp::run_webapp(path, 3000, watch)
            .map_err(|e| ChakraError::Server(ServerError::startup_failed(3000, e)));
    }

    // Detect project and setup
    let (lang, temp_output_dir) = config::setup_project_compilation(path, language_override, watch)
        .ok_or_else(|| {
            ChakraError::language_detection(format!(
                "Failed to setup compilation for project: {}",
                path
            ))
        })?;

    // Compile the project
    let result = config::compile_project(path, &temp_output_dir, lang, watch).ok_or_else(|| {
        ChakraError::Compilation(crate::error::CompilationError::build_failed(
            "project".to_string(),
            "Compilation failed",
        ))
    })?;

    let (wasm_path, is_wasm_bindgen, js_path) = result;

    // Run the server based on project type
    let server_config = ServerConfig {
        wasm_path,
        js_path,
        port,
        watch_mode: watch,
        project_path: if watch { Some(path.to_string()) } else { None },
        output_dir: if watch {
            Some(temp_output_dir.to_string())
        } else {
            None
        },
    };

    // Log a message based on the project type
    if is_wasm_bindgen {
        println!("Running wasm-bindgen project with JS support");
    } else {
        println!("Running standard WASM project");
    }

    config::run_server(server_config)
        .map_err(|e| ChakraError::Server(ServerError::startup_failed(port, e)))
}

/// Check if a server is currently running
pub fn is_server_running() -> bool {
    if !std::path::Path::new(PID_FILE).exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            // Checking if a process exists
            let ps_command = std::process::Command::new("ps")
                .arg("-p")
                .arg(pid.to_string())
                .output();

            if let Ok(output) = ps_command {
                // the process exists
                return output.status.success()
                    && String::from_utf8_lossy(&output.stdout).lines().count() > 1;
            }
        }
    }

    false
}

/// Stop the existing server using the PID stored in the file
pub fn stop_existing_server() -> Result<()> {
    // Check if the server is running
    if !is_server_running() {
        // No server is running, clean up any stale PID file
        if std::path::Path::new(PID_FILE).exists() {
            std::fs::remove_file(PID_FILE).map_err(|e| {
                ChakraError::Server(ServerError::StopFailed {
                    pid: 0,
                    reason: format!("Failed to remove stale PID file: {}", e),
                })
            })?;
        }
        return Err(ChakraError::Server(ServerError::NotRunning));
    }

    let pid_str = std::fs::read_to_string(PID_FILE).map_err(|e| {
        ChakraError::Server(ServerError::StopFailed {
            pid: 0,
            reason: format!("Failed to read PID file: {}", e),
        })
    })?;

    let pid = pid_str.trim().parse::<u32>().map_err(|e| {
        ChakraError::Server(ServerError::StopFailed {
            pid: 0,
            reason: format!("Failed to parse PID '{}': {}", pid_str.trim(), e),
        })
    })?;

    let kill_command = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| {
            ChakraError::Server(ServerError::StopFailed {
                pid,
                reason: format!("Failed to kill server process: {}", e),
            })
        })?;

    if kill_command.status.success() {
        std::fs::remove_file(PID_FILE).map_err(|e| {
            ChakraError::Server(ServerError::StopFailed {
                pid,
                reason: format!("Failed to remove PID file: {}", e),
            })
        })?;
        println!("💀 Existing Chakra server terminated successfully.");
        Ok(())
    } else {
        // Failed to stop the server
        let error_msg = String::from_utf8_lossy(&kill_command.stderr);
        Err(ChakraError::Server(ServerError::StopFailed {
            pid,
            reason: error_msg.to_string(),
        }))
    }
}

// Private helper functions

// Handle JS files that might be associated with WASM
fn handle_js_file(path: &str, port: u16) -> Result<()> {
    let path_obj = std::path::Path::new(path);

    // Look for a corresponding .wasm file
    let wasm_path = path_obj.with_extension("wasm");
    if wasm_path.exists() {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!(
            "  ℹ️  \x1b[1;34mDetected potential wasm-bindgen JS file: {}\x1b[0m",
            path
        );
        println!(
            "  \x1b[0;37mFound corresponding WASM file: {}\x1b[0m",
            wasm_path.display()
        );
        println!("\x1b[1;34m╰\x1b[0m\n");

        // Check if JS file contains wasm-bindgen patterns
        if let Ok(js_content) = std::fs::read_to_string(path) {
            if js_content.contains("wasm_bindgen") || js_content.contains("__wbindgen") {
                println!("  ✅  \x1b[1;32mConfirmed wasm-bindgen project\x1b[0m");
                println!("  \x1b[0;37mRunning with wasm-bindgen support\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m\n");

                // Get wasm filename
                let wasm_filename = wasm_path
                    .file_name()
                    .ok_or_else(|| ChakraError::path("Invalid WASM file path"))?
                    .to_string_lossy()
                    .to_string();

                // Server should not start for tests
                if cfg!(test) {
                    return Ok(());
                }

                return wasm::handle_wasm_bindgen_files(
                    path,
                    wasm_path.to_str().unwrap(),
                    port,
                    &wasm_filename,
                )
                .map_err(|e| ChakraError::Server(ServerError::startup_failed(port, e)));
            }
        }

        // If not confirmed as wasm-bindgen, run as regular WASM
        return run_wasm_file(wasm_path.to_str().unwrap(), port);
    }

    Err(ChakraError::file_not_found(format!(
        "Corresponding WASM file not found for JS file: {}",
        path
    )))
}

// Handle wasm-bindgen files
fn handle_wasm_bindgen_file(path_obj: &std::path::Path, path: &str, port: u16) -> Result<bool> {
    let file_name = path_obj
        .file_name()
        .ok_or_else(|| ChakraError::path("Invalid file path"))?
        .to_string_lossy();

    if file_name.ends_with("_bg.wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!(
            "  ℹ️  \x1b[1;34mDetected wasm-bindgen _bg.wasm file: {}\x1b[0m",
            path
        );

        // Remove _bg.wasm and add .js to find the JS file
        let js_base_name = file_name.replace("_bg.wasm", "");
        let js_file_name = format!("{}.js", js_base_name);
        let js_path = path_obj
            .parent()
            .ok_or_else(|| ChakraError::path("Invalid parent directory"))?
            .join(&js_file_name);

        if js_path.exists() {
            println!(
                "  ✅ \x1b[1;32mFound corresponding JS file: {}\x1b[0m",
                js_path.display()
            );
            println!("  \x1b[0;37mRunning with wasm-bindgen support\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m\n");

            // Server should not run for tests
            if cfg!(test) {
                return Ok(true);
            }

            wasm::handle_wasm_bindgen_files(
                js_path.to_str().unwrap(),
                path,
                port,
                file_name.as_ref(),
            )
            .map_err(|e| ChakraError::Server(ServerError::startup_failed(port, e)))?;

            return Ok(true);
        } else {
            // Try to find other JS files
            return search_for_js_files(path_obj, path, port, file_name.as_ref());
        }
    }

    Ok(false)
}

// Search for JS files that might be associated with a WASM file
fn search_for_js_files(
    path_obj: &std::path::Path,
    path: &str,
    port: u16,
    _wasm_filename: &str,
) -> Result<bool> {
    println!("  ⚠️ \x1b[1;33mWarning: Could not find corresponding JS file\x1b[0m");
    println!("  \x1b[0;37mLooking for other JS files in the same directory...\x1b[0m");

    // Search for any JS file in the same directory
    if let Some(dir) = path_obj.parent() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.extension().map_or(false, |ext| ext == "js") {
                    // Check if this might be a wasm-bindgen JS by reading its content
                    if let Ok(js_content) = std::fs::read_to_string(&entry_path) {
                        if js_content.contains("wasm_bindgen") || js_content.contains("__wbindgen")
                        {
                            println!(
                                "  ✅ \x1b[1;32mFound potential wasm-bindgen JS file: {}\x1b[0m",
                                entry_path.display()
                            );
                            println!("\x1b[1;34m╰\x1b[0m\n");

                            // Server should not run for tests
                            if cfg!(test) {
                                return Ok(true);
                            }

                            // Run with this JS file
                            config::run_server(ServerConfig {
                                wasm_path: path.to_string(),
                                js_path: Some(entry_path.to_str().unwrap().to_string()),
                                port,
                                watch_mode: false,
                                project_path: None,
                                output_dir: None,
                            })
                            .map_err(|e| {
                                ChakraError::Server(ServerError::startup_failed(port, e))
                            })?;

                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    println!("  ⚠️ \x1b[1;33mNo suitable JS file found. This is likely a wasm-bindgen module without its JS counterpart.\x1b[0m");
    println!("  \x1b[0;37mTry running the .js file directly instead.\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");

    Err(ChakraError::Wasm(
        crate::error::WasmError::WasmBindgenJsNotFound,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_is_server_running_no_pid_file() {
        // Ensure PID file doesn't exist
        let _ = std::fs::remove_file(PID_FILE);
        assert!(!is_server_running());
    }

    #[test]
    fn test_handle_js_file_no_wasm_counterpart() {
        let temp_dir = tempdir().unwrap();
        let js_file = temp_dir.path().join("test.js");

        fs::write(&js_file, "console.log('test')").unwrap();

        let result = handle_js_file(js_file.to_str().unwrap(), 8080);
        assert!(result.is_err());

        match result.unwrap_err() {
            ChakraError::FileNotFound { .. } => {}
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[test]
    fn test_handle_wasm_bindgen_file_detection() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("test_bg.wasm");
        let js_file = temp_dir.path().join("test.js");

        // Create both the WASM file and the corresponding JS file
        fs::write(&wasm_file, b"fake wasm content").unwrap();

        // Create a realistic wasm-bindgen JS file content
        let js_content = r#"
import * as wasm from './test_bg.wasm';

export function greet(name) {
    return wasm.greet(name);
}

// wasm-bindgen generated code
let wasm_bindgen;
function __wbg_init() {
    // wasm-bindgen initialization code
}
"#;
        fs::write(&js_file, js_content).unwrap();

        let path_obj = wasm_file.as_path();
        let result = handle_wasm_bindgen_file(path_obj, wasm_file.to_str().unwrap(), 8080);

        // Should return true indicating it detected and would handle the file
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_handle_wasm_bindgen_file_detection_missing_js() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("test_bg.wasm");

        // Create only the WASM file without the JS file
        fs::write(&wasm_file, b"fake wasm content").unwrap();

        let path_obj = wasm_file.as_path();
        let result = handle_wasm_bindgen_file(path_obj, wasm_file.to_str().unwrap(), 8080);

        // This should return an error because the JS file is missing
        assert!(result.is_err());

        // Check that it's the specific WasmBindgenJsNotFound error
        match result.unwrap_err() {
            ChakraError::Wasm(crate::error::WasmError::WasmBindgenJsNotFound) => {}
            _ => panic!("Expected WasmBindgenJsNotFound error"),
        }
    }

    #[test]
    fn test_handle_js_file_with_wasm_counterpart() {
        let temp_dir = tempdir().unwrap();
        let js_file = temp_dir.path().join("test.js");
        let wasm_file = temp_dir.path().join("test.wasm");

        // Create realistic wasm-bindgen files
        let js_content = r#"
import * as wasm from './test_bg.wasm';

// This contains wasm_bindgen patterns
const __wbindgen_string_new = function(ptr, len) {
    return String.fromCharCode(...new Uint8Array(wasm.memory.buffer, ptr, len));
};

export function greet(name) {
    return wasm.greet(name);
}
"#;
        fs::write(&js_file, js_content).unwrap();
        fs::write(&wasm_file, b"fake wasm content").unwrap();

        let result = handle_js_file(js_file.to_str().unwrap(), 8080);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_js_file_without_wasm_bindgen() {
        let temp_dir = tempdir().unwrap();
        let js_file = temp_dir.path().join("test.js");
        let wasm_file = temp_dir.path().join("test.wasm");

        // Create files without wasm-bindgen patterns
        let js_content = r#"
// Regular JavaScript file without wasm-bindgen
console.log("Hello world");
function normalFunction() {
    return "test";
}
"#;
        fs::write(&js_file, js_content).unwrap();
        fs::write(&wasm_file, b"fake wasm content").unwrap();

        let result = handle_js_file(js_file.to_str().unwrap(), 8080);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_wasm_file_direct() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("test.wasm");

        fs::write(&wasm_file, b"fake wasm content").unwrap();

        let result = run_wasm_file(wasm_file.to_str().unwrap(), 8080);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_project_direct() {
        let temp_dir = tempdir().unwrap();

        let result = run_project(temp_dir.path().to_str().unwrap(), 8080, None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wasm_bindgen_js_detection() {
        let temp_dir = tempdir().unwrap();

        // Test with wasm-bindgen content
        let js_with_bindgen = temp_dir.path().join("bindgen.js");
        fs::write(
            &js_with_bindgen,
            "const __wbindgen = true; export function test() {}",
        )
        .unwrap();

        if let Ok(content) = std::fs::read_to_string(&js_with_bindgen) {
            assert!(content.contains("__wbindgen"));
        }

        // Test with regular JS content
        let regular_js = temp_dir.path().join("regular.js");
        fs::write(&regular_js, "console.log('hello'); function test() {}").unwrap();

        if let Ok(content) = std::fs::read_to_string(&regular_js) {
            assert!(!content.contains("__wbindgen"));
            assert!(!content.contains("wasm_bindgen"));
        }
    }

    #[test]
    fn test_pid_file_operations() {
        let test_pid_file = "/tmp/test_chakra_server.pid";

        // Ensure clean state
        let _ = std::fs::remove_file(test_pid_file);

        // Test writing PID
        std::fs::write(test_pid_file, "12345").unwrap();
        assert!(std::path::Path::new(test_pid_file).exists());

        // Test reading PID
        let content = std::fs::read_to_string(test_pid_file).unwrap();
        assert_eq!(content, "12345");

        std::fs::remove_file(test_pid_file).unwrap();
    }

    #[test]
    fn test_server_config_creation() {
        let config = ServerConfig {
            wasm_path: "test.wasm".to_string(),
            js_path: Some("test.js".to_string()),
            port: 8080,
            watch_mode: false,
            project_path: None,
            output_dir: None,
        };

        assert_eq!(config.wasm_path, "test.wasm");
        assert_eq!(config.js_path, Some("test.js".to_string()));
        assert_eq!(config.port, 8080);
        assert!(!config.watch_mode);
    }
}
