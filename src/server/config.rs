use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::channel,
    Arc,
};
use std::thread;
use std::time::Duration;
use tiny_http::Server;

use crate::compiler;
use crate::watcher;

use super::handler;
use super::utils;
use super::wasm;

// Configuration struct for server setup
pub struct ServerConfig {
    pub wasm_path: String,
    pub js_path: Option<String>,
    pub port: u16,
    pub watch_mode: bool,
    pub project_path: Option<String>,
    pub output_dir: Option<String>,
}

/// Combined server function
pub fn run_server(config: ServerConfig) -> Result<(), String> {
    // Check if a server is already running
    if super::is_server_running() {
        match super::stop_existing_server() {
            Ok(_) => println!("💀 Existing server stopped successfully."),
            Err(e) => eprintln!("❗ Warning when stopping existing server: {e}"),
        }
    }

    // Check if the port is available
    if !utils::is_port_available(config.port) {
        return Err(format!(
            "❗ Port {} is already in use, please choose a different port.",
            config.port
        ));
    }

    let path_obj = Path::new(&config.wasm_path);
    if !path_obj.exists() {
        return Err(format!("❗ Path not found: {}", config.wasm_path));
    }

    // If it's a directory, look for WASM files
    if path_obj.is_dir() {
        let wasm_files = utils::find_wasm_files(path_obj);
        if wasm_files.is_empty() {
            return Err(format!(
                "❗ No WASM files found in directory: {}",
                config.wasm_path
            ));
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
            println!("  \x1b[1;37mchakra --wasm --path <filename.wasm>\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m");
            return Err("Please select a specific WASM file to run".to_string());
        }
    }

    // Verify the WASM file
    if !path_obj.is_file() {
        return Err(format!("❗ Not a file: {}", config.wasm_path));
    }
    if path_obj
        .extension()
        .map_or(true, |ext| ext.to_string_lossy().to_lowercase() != "wasm")
    {
        return Err(format!("❗ Not a WASM file: {}", config.wasm_path));
    }

    // Get file information
    let wasm_filename = path_obj
        .file_name()
        .ok_or_else(|| "Invalid path".to_string())?
        .to_string_lossy()
        .to_string();

    let js_filename = if let Some(js_path) = &config.js_path {
        let js_path_obj = Path::new(js_path);
        if js_path_obj.exists() && js_path_obj.is_file() {
            Some(
                js_path_obj
                    .file_name()
                    .ok_or_else(|| "Invalid JS path".to_string())?
                    .to_string_lossy()
                    .to_string(),
            )
        } else {
            None
        }
    } else {
        None
    };

    let absolute_path = fs::canonicalize(&config.wasm_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| config.wasm_path.clone());

    let file_size = match fs::metadata(&config.wasm_path) {
        Ok(metadata) => {
            let bytes = metadata.len();
            if bytes < 1024 {
                format!("{} bytes", bytes)
            } else if bytes < 1024 * 1024 {
                format!("{:.2} KB", bytes as f64 / 1024.0)
            } else {
                format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
            }
        }
        Err(_) => "unknown size".to_string(),
    };

    let url = format!("http://localhost:{}", config.port);

    // Display welcome message
    utils::print_server_info(
        &url,
        config.port,
        &wasm_filename,
        &file_size,
        &absolute_path,
        config.watch_mode,
    );

    // Open browser
    if let Err(e) = webbrowser::open(&url) {
        println!("❗ Failed to open browser automatically: {e}");
    }

    // Store PID
    let pid = std::process::id();
    fs::write(super::PID_FILE, pid.to_string())
        .map_err(|e| format!("Failed to write PID to {}: {}", super::PID_FILE, e))?;

    // Setup for watch mode if needed
    if config.watch_mode && config.project_path.is_some() && config.output_dir.is_some() {
        // Create channels for communication
        let (tx, rx) = channel();
        let reload_flag = Arc::new(AtomicBool::new(false));

        // Setup file watcher
        match watcher::ProjectWatcher::new(config.project_path.as_ref().unwrap()) {
            Ok(watcher) => {
                // Start server in a new thread
                let wasm_path_clone = config.wasm_path.clone();
                let port = config.port;
                let project_path = config.project_path.unwrap();
                let output_dir = config.output_dir.unwrap();
                let reload_flag_clone = Arc::clone(&reload_flag);

                let server_thread = thread::spawn(move || {
                    // Create HTTP server
                    if let Ok(server) = Server::http(format!("0.0.0.0:{port}")) {
                        // Track connected clients for live reload
                        let mut clients_to_reload = Vec::new();

                        // Handle requests
                        for request in server.incoming_requests() {
                            // Check for shutdown signal
                            if rx.try_recv().is_ok() {
                                break;
                            }

                            // Handle the request with proper reload flag checking
                            handle_request_with_reload_flag(
                                request,
                                js_filename.as_deref(),
                                &wasm_filename,
                                &wasm_path_clone,
                                true, // watch_mode is true
                                &mut clients_to_reload,
                                &reload_flag_clone,
                            );
                        }
                    }
                });

                // Let the server start up
                thread::sleep(Duration::from_millis(500));

                // Watch for file changes in the main thread
                println!("👀 Watching project directory for changes...");

                loop {
                    // Wait for file changes
                    if let Some(Ok(events)) = watcher.wait_for_change() {
                        if watcher.should_recompile(&events) {
                            println!("\n📝 File change detected. Recompiling...");

                            // Recompile the project
                            match compiler::compile_for_execution(&project_path, &output_dir) {
                                Ok(_) => {
                                    println!("✅ Recompilation successful!");
                                    println!("🔄 Reloading in browser...");

                                    // Set the reload flag
                                    reload_flag.store(true, Ordering::SeqCst);
                                }
                                Err(e) => {
                                    println!("❌ Recompilation failed: {}", e);
                                }
                            }
                        }
                    }

                    // Check for server exit
                    if server_thread.is_finished() {
                        println!("Server stopped. Exiting watch mode.");
                        break;
                    }

                    // Sleep to avoid high CPU usage
                    thread::sleep(Duration::from_millis(100));
                }

                // Signal the server to stop if still running
                let _ = tx.send(());

                // Wait for server thread to finish
                if let Err(e) = server_thread.join() {
                    eprintln!("Error joining server thread: {:?}", e);
                }

                return Ok(());
            }
            Err(e) => {
                eprintln!("Failed to set up file watcher: {}", e);

                // Fall back to standard non-watching mode
                if config.js_path.is_some() {
                    wasm::serve_wasm_bindgen_files(
                        &config.wasm_path,
                        config.js_path.as_ref().unwrap(),
                        config.port,
                        &wasm_filename,
                    )?;
                } else {
                    wasm::serve_wasm_file(&config.wasm_path, config.port, &wasm_filename)?
                }
            }
        }
    } else {
        // Standard server without watching
        if config.js_path.is_some() {
            wasm::serve_wasm_bindgen_files(
                &config.wasm_path,
                config.js_path.as_ref().unwrap(),
                config.port,
                &wasm_filename,
            )?;
        } else {
            wasm::serve_wasm_file(&config.wasm_path, config.port, &wasm_filename)?;
        }

        return Ok(());
    }

    // Clean up PID file
    if Path::new(super::PID_FILE).exists() {
        let _ = fs::remove_file(super::PID_FILE);
    }

    Ok(())
}

/// Handle request with reload flag support for watch mode
fn handle_request_with_reload_flag(
    request: tiny_http::Request,
    js_filename: Option<&str>,
    wasm_filename: &str,
    wasm_path: &str,
    watch_mode: bool,
    clients_to_reload: &mut Vec<String>,
    reload_flag: &Arc<AtomicBool>,
) {
    let url = request.url().to_string();

    // Handle the special reload endpoint for watch mode
    if url == "/reload" && watch_mode {
        println!("🔄 Handling reload request in watch mode");

        // Check if there's a reload pending
        if reload_flag.load(Ordering::SeqCst) {
            // Send reload signal
            let response = tiny_http::Response::from_string("reload")
                .with_header(tiny_http::Header::from_bytes(&b"X-Reload"[..], &b"true"[..]).unwrap())
                .with_header(utils::content_type_header("text/plain"));

            // Reset the flag after sending the reload signal
            reload_flag.store(false, Ordering::SeqCst);
            println!("🔄 Sent reload signal to browser");

            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending reload response: {}", e);
            }
        } else {
            // No reload needed
            let response = tiny_http::Response::from_string("no-reload")
                .with_header(utils::content_type_header("text/plain"));

            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending reload response: {}", e);
            }
        }
        return;
    }

    // For all other requests, use the standard handler
    handler::handle_request(
        request,
        js_filename,
        wasm_filename,
        wasm_path,
        watch_mode,
        clients_to_reload,
    );
}

/// Set up project compilation environment and detect language
pub fn setup_project_compilation(
    path: &str,
    language_override: Option<String>,
    watch: bool,
) -> Option<(compiler::ProjectLanguage, String)> {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🚀 \x1b[1;36mChakra: Compile and Run\x1b[0m\n");

    // Detect project language
    let detected_language = compiler::detect_project_language(path);
    println!(
        "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
        path
    );
    println!(
        "  🔍 \x1b[1;34mDetected Language:\x1b[0m \x1b[1;32m{:?}\x1b[0m",
        detected_language
    );

    if watch {
        println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mEnabled\x1b[0m");
    }

    // Use language from command if provided
    let lang = match language_override {
        Some(lang_str) => {
            // Convert string to ProjectLanguage enum
            match lang_str.to_lowercase().as_str() {
                "rust" => compiler::ProjectLanguage::Rust,
                "go" => compiler::ProjectLanguage::Go,
                "c" => compiler::ProjectLanguage::C,
                "assemblyscript" => compiler::ProjectLanguage::AssemblyScript,
                "python" => compiler::ProjectLanguage::Python,
                _ => {
                    println!(
                        "  ⚠️ \x1b[1;33mUnknown language '{}', using auto-detected\x1b[0m",
                        lang_str
                    );
                    detected_language
                }
            }
        }
        None => detected_language,
    };

    if lang == compiler::ProjectLanguage::Unknown {
        println!("\n  ❓ \x1b[1;33mNo recognizable project detected in this directory\x1b[0m");
        println!("\n  💡 \x1b[1;33mTo run a WASM file directly:\x1b[0m");
        println!("     \x1b[1;37mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m");
        return None;
    }

    // Create a temporary directory for output
    let temp_dir = std::env::temp_dir().join("chakra_temp");
    let temp_output_dir = temp_dir.to_str().unwrap_or("/tmp").to_string();

    // Create temp directory if it doesn't exist
    if !temp_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            println!(
                "  ❌ \x1b[1;31mFailed to create temporary directory: {}\x1b[0m",
                e
            );
            println!("\x1b[1;34m╰\x1b[0m");
            return None;
        }
    }

    println!(
        "  📁 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{}\x1b[0m",
        temp_output_dir
    );
    println!("\x1b[1;34m╰\x1b[0m\n");

    // Get system information
    compiler::print_system_info();

    // Check for missing tools
    let os = compiler::detect_operating_system();
    let missing_tools = compiler::get_missing_tools(&lang, &os);
    if !missing_tools.is_empty() {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ⚠️  \x1b[1;33mMissing Required Tools:\x1b[0m");
        for tool in &missing_tools {
            println!("     \x1b[1;31m• {}\x1b[0m", tool);
        }
        println!("\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");
        return None;
    }

    // Check if this language is supported for compilation
    if lang == compiler::ProjectLanguage::Python {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ⚠️  \x1b[1;33mPython WebAssembly compilation coming soon!\x1b[0m");
        println!("  \x1b[0;37mSupport for compiling Python projects is under development.\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");
        return None;
    }

    Some((lang, temp_output_dir))
}

/// Compile a project and return the path to the WASM file
pub fn compile_project(
    path: &str,
    output_dir: &str,
    _lang: compiler::ProjectLanguage,
    watch: bool,
) -> Option<(String, bool, Option<String>)> {
    // Compile the project
    match compiler::compile_for_execution(path, output_dir) {
        Ok(output_path) => {
            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  ✅ \x1b[1;36mCompilation Successful\x1b[0m\n");

            // Check if this is a JS file (indicating wasm-bindgen output)
            let is_wasm_bindgen = output_path.ends_with(".js");

            if is_wasm_bindgen {
                // For wasm-bindgen, output_path is the JS file path
                println!(
                    "  📦 \x1b[1;34mJS File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                    output_path
                );

                // Find the corresponding WASM file
                let wasm_path = Path::new(&output_path).with_extension("wasm");

                if !wasm_path.exists() {
                    // Check for the _bg.wasm file pattern
                    let js_stem = Path::new(&output_path)
                        .file_stem()
                        .unwrap()
                        .to_string_lossy();
                    let dir = Path::new(&output_path).parent().unwrap();
                    let bg_wasm_filename = format!("{}_bg.wasm", js_stem);
                    let bg_wasm_path = dir.join(&bg_wasm_filename);

                    if bg_wasm_path.exists() {
                        println!(
                            "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                            bg_wasm_path.display()
                        );

                        if watch {
                            println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m (Press Ctrl+C to stop)");
                        }

                        println!("\x1b[1;34m╰\x1b[0m\n");

                        // Return the WASM path, indicate it's a wasm-bindgen project, and provide JS path
                        return Some((
                            bg_wasm_path.to_string_lossy().to_string(),
                            true,
                            Some(output_path),
                        ));
                    } else {
                        // Try to find any .wasm file in the same directory
                        let mut found_wasm = None;
                        if let Ok(entries) = fs::read_dir(dir) {
                            for entry in entries.flatten() {
                                if let Some(ext) = entry.path().extension() {
                                    if ext == "wasm" {
                                        found_wasm = Some(entry.path());
                                        break;
                                    }
                                }
                            }
                        }

                        if let Some(found_wasm_path) = found_wasm {
                            println!(
                                "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                                found_wasm_path.display()
                            );

                            if watch {
                                println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m (Press Ctrl+C to stop)");
                            }

                            println!("\x1b[1;34m╰\x1b[0m\n");

                            return Some((
                                found_wasm_path.to_string_lossy().to_string(),
                                true,
                                Some(output_path),
                            ));
                        } else {
                            eprintln!("\n\x1b[1;34m╭\x1b[0m");
                            eprintln!("  ❌ \x1b[1;31mWASM File Not Found\x1b[0m\n");
                            eprintln!(
                                "  \x1b[0;91mExpected WASM file at: {}\x1b[0m",
                                wasm_path.display()
                            );
                            eprintln!("\x1b[1;34m╰\x1b[0m");
                            return None;
                        }
                    }
                }

                println!(
                    "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                    wasm_path.display()
                );

                if watch {
                    println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m (Press Ctrl+C to stop)");
                }

                println!("\x1b[1;34m╰\x1b[0m\n");

                // Return the WASM path, indicate it's a wasm-bindgen project, and provide JS path
                Some((
                    wasm_path.to_string_lossy().to_string(),
                    true,
                    Some(output_path),
                ))
            } else {
                // Standard WASM file
                println!(
                    "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                    output_path
                );

                if watch {
                    println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m (Press Ctrl+C to stop)");
                }

                println!("\x1b[1;34m╰\x1b[0m\n");

                // Return the WASM path and indicate it's not a wasm-bindgen project
                Some((output_path, false, None))
            }
        }
        Err(e) => {
            eprintln!("\n\x1b[1;34m╭\x1b[0m");
            eprintln!("  ❌ \x1b[1;31mCompilation Failed\x1b[0m\n");
            eprintln!("  \x1b[0;91m{}\x1b[0m", e);
            eprintln!("\x1b[1;34m╰\x1b[0m");
            None
        }
    }
}
