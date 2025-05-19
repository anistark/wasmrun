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
pub fn run_wasm_file(path: &str, port: u16) {
    let path_obj = std::path::Path::new(path);

    // Check if file exists and has correct extension
    if !path_obj.extension().map_or(false, |ext| ext == "wasm") {
        // Handle JS files that might be associated with WASM
        if path_obj.extension().map_or(false, |ext| ext == "js") {
            handle_js_file(path, port);
            return;
        }

        utils::print_error(format!("Error: Not a WASM file: {}", path));
        println!("  \x1b[1;37mPlease specify a path to a .wasm file:\x1b[0m");
        println!("  \x1b[1;33mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
        return;
    }

    // Special handling for _bg.wasm files (wasm-bindgen output)
    if handle_wasm_bindgen_file(path_obj, path, port) {
        return;
    }

    // Regular WASM file handling
    let wasm_filename = Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if let Err(e) = wasm::serve_wasm_file(path, port, &wasm_filename) {
        utils::print_error(format!("Error serving WASM file: {}", e));
    }
}

/// Compile and run a project
pub fn run_project(path: &str, port: u16, language_override: Option<String>, watch: bool) {
    let path_obj = std::path::Path::new(path);

    // Handle WASM file for convenience
    if path_obj.is_file() && path_obj.extension().map_or(false, |ext| ext == "wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected WASM file: {}\x1b[0m", path);
        println!("  \x1b[0;37mRunning the WASM file directly...\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");

        run_wasm_file(path, port);
        return;
    }

    // Handle JS file that might be wasm-bindgen output
    if path_obj.is_file() && path_obj.extension().map_or(false, |ext| ext == "js") {
        handle_js_file(path, port);
        return;
    }

    // Handle project directory
    if !path_obj.is_dir() {
        utils::print_error(if !path_obj.exists() {
            format!("Error: Path not found: {}", path)
        } else {
            format!("Error: Not a WASM file or project directory: {}", path)
        });
        println!("  \x1b[1;37mPlease specify a path to a project directory or use --wasm for WASM files:\x1b[0m");
        println!("  \x1b[1;33mchakra --path /path/to/your/project/\x1b[0m");
        println!("  \x1b[1;33mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
        return;
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
        if let Err(e) = webapp::run_webapp(path, 3000, watch) {
            eprintln!("\n\x1b[1;34m╭\x1b[0m");
            eprintln!("  ❌ \x1b[1;31mError Running Web Application:\x1b[0m");
            eprintln!("  \x1b[0;91m{}\x1b[0m", e);
            eprintln!("\x1b[1;34m╰\x1b[0m");
        }

        return;
    }

    // Detect project and setup
    let (lang, temp_output_dir) =
        match config::setup_project_compilation(path, language_override, watch) {
            Some(result) => result,
            None => return, // Error already printed
        };

    // Compile the project
    let result = config::compile_project(path, &temp_output_dir, lang, watch);

    if let Some((wasm_path, is_wasm_bindgen, js_path)) = result {
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

        if let Err(e) = config::run_server(server_config) {
            utils::print_error(format!("Error Running Server: {}", e));
        }
    }
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
pub fn stop_existing_server() -> Result<(), String> {
    // Check if the server is running
    if !is_server_running() {
        // No server is running, clean up any stale PID file
        if std::path::Path::new(PID_FILE).exists() {
            if let Err(e) = std::fs::remove_file(PID_FILE) {
                return Err(format!(
                    "No server running, but failed to remove stale PID file: {e}"
                ));
            }
        }

        return Ok(());
    }

    let pid_str =
        std::fs::read_to_string(PID_FILE).map_err(|e| format!("Failed to read PID file: {}", e))?;

    let pid = pid_str
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse PID '{}': {}", pid_str.trim(), e))?;

    let kill_command = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to kill server process: {}", e))?;

    if kill_command.status.success() {
        std::fs::remove_file(PID_FILE).map_err(|e| format!("Failed to remove PID file: {e}"))?;
        println!("💀 Existing Chakra server terminated successfully.");
        Ok(())
    } else {
        // Failed to stop the server
        let error_msg = String::from_utf8_lossy(&kill_command.stderr);
        Err(format!("Failed to stop Chakra server: {}", error_msg))
    }
}

// Private helper functions

// Handle JS files that might be associated with WASM
fn handle_js_file(path: &str, port: u16) {
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
                let wasm_filename = wasm_path.file_name().unwrap().to_string_lossy().to_string();

                if let Err(e) = wasm::handle_wasm_bindgen_files(
                    path,
                    wasm_path.to_str().unwrap(),
                    port,
                    &wasm_filename,
                ) {
                    utils::print_error(format!("Error Running Chakra Server: {}", e));
                }

                return;
            }
        }

        // If not confirmed as wasm-bindgen, run as regular WASM
        run_wasm_file(wasm_path.to_str().unwrap(), port);
    }
}

// Handle wasm-bindgen files
fn handle_wasm_bindgen_file(path_obj: &std::path::Path, path: &str, port: u16) -> bool {
    let file_name = path_obj.file_name().unwrap().to_string_lossy();

    if file_name.ends_with("_bg.wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!(
            "  ℹ️  \x1b[1;34mDetected wasm-bindgen _bg.wasm file: {}\x1b[0m",
            path
        );

        // Remove _bg.wasm and add .js to find the JS file
        let js_base_name = file_name.replace("_bg.wasm", "");
        let js_file_name = format!("{}.js", js_base_name);
        let js_path = path_obj.parent().unwrap().join(&js_file_name);

        if js_path.exists() {
            println!(
                "  ✅ \x1b[1;32mFound corresponding JS file: {}\x1b[0m",
                js_path.display()
            );
            println!("  \x1b[0;37mRunning with wasm-bindgen support\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m\n");

            if let Err(e) = wasm::handle_wasm_bindgen_files(
                js_path.to_str().unwrap(),
                path,
                port,
                file_name.as_ref(),
            ) {
                utils::print_error(format!("Error Running Chakra Server: {}", e));
            }

            return true;
        } else {
            // Try to find other JS files
            return search_for_js_files(path_obj, path, port, file_name.as_ref());
        }
    }

    false
}

// Search for JS files that might be associated with a WASM file
fn search_for_js_files(
    path_obj: &std::path::Path,
    path: &str,
    port: u16,
    _wasm_filename: &str,
) -> bool {
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

                            // Run with this JS file
                            if let Err(e) = config::run_server(ServerConfig {
                                wasm_path: path.to_string(),
                                js_path: Some(entry_path.to_str().unwrap().to_string()),
                                port,
                                watch_mode: false,
                                project_path: None,
                                output_dir: None,
                            }) {
                                utils::print_error(format!("Error Running Chakra Server: {}", e));
                            }

                            return true;
                        }
                    }
                }
            }
        }
    }

    println!("  ⚠️ \x1b[1;33mNo suitable JS file found. This is likely a wasm-bindgen module without its JS counterpart.\x1b[0m");
    println!("  \x1b[0;37mTry running the .js file directly instead.\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");

    false
}
