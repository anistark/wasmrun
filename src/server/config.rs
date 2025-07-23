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
use crate::compiler::builder::{OptimizationLevel, TargetType};
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

/// Run server
pub fn run_server(config: ServerConfig) -> Result<(), String> {
    // Check if server is running
    if super::is_server_running() {
        match super::stop_existing_server() {
            Ok(_) => println!("💀 Existing server stopped successfully."),
            Err(e) => eprintln!("❗ Warning when stopping existing server: {e}"),
        }
    }

    // Check if port is available
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

    // If directory, look for WASM files
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
            println!("  \x1b[1;37mwasmrun --wasm --path <filename.wasm>\x1b[0m");
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

    // Setup for watch mode
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
                    if let Ok(server) = Server::http(format!("0.0.0.0:{port}")) {
                        let mut clients_to_reload = Vec::new();

                        for request in server.incoming_requests() {
                            if rx.try_recv().is_ok() {
                                break;
                            }

                            handle_request_with_reload_flag(
                                request,
                                js_filename.as_deref(),
                                &wasm_filename,
                                &wasm_path_clone,
                                true,
                                &mut clients_to_reload,
                                &reload_flag_clone,
                            );
                        }
                    }
                });

                thread::sleep(Duration::from_millis(500));
                println!("👀 Watching project directory for changes...");

                loop {
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

                    if server_thread.is_finished() {
                        println!("Server stopped. Exiting watch mode.");
                        break;
                    }

                    // Sleep to avoid high CPU usage
                    thread::sleep(Duration::from_millis(100));
                }

                // Stop server if still running
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

    if url == "/reload" && watch_mode {
        println!("🔄 Handling reload request in watch mode");

        // Check if there's a reload pending
        if reload_flag.load(Ordering::SeqCst) {
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

/// Set up project compilation environment
pub fn setup_project_compilation(
    path: &str,
    language_override: Option<String>,
    watch: bool,
) -> Option<(crate::compiler::ProjectLanguage, String)> {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🚀 \x1b[1;36mWasmrun: Compile and Run\x1b[0m\n");

    let detected_language = crate::compiler::detect_project_language(path);
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

    let lang = match language_override {
        Some(lang_str) => match lang_str.to_lowercase().as_str() {
            "rust" => crate::compiler::ProjectLanguage::Rust,
            "c" => crate::compiler::ProjectLanguage::C,
            "asc" => crate::compiler::ProjectLanguage::Asc,
            "python" => crate::compiler::ProjectLanguage::Python,
            _ => {
                println!(
                    "  ⚠️ \x1b[1;33mUnknown language '{}', using auto-detected\x1b[0m",
                    lang_str
                );
                detected_language
            }
        },
        None => detected_language,
    };

    if lang == crate::compiler::ProjectLanguage::Unknown {
        println!("\n  ❓ \x1b[1;33mNo recognizable project detected in this directory\x1b[0m");
        println!("\n  💡 \x1b[1;33mTo run a WASM file directly:\x1b[0m");
        println!("     \x1b[1;37mwasmrun --wasm --path /path/to/your/file.wasm\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m");
        return None;
    }

    match crate::plugin::PluginManager::new() {
        Ok(plugin_manager) => {
            if let Some(plugin) = plugin_manager.find_plugin_for_project(path) {
                println!(
                    "  🔌 \x1b[1;34mPlugin:\x1b[0m \x1b[1;32m{} v{}\x1b[0m",
                    plugin.info().name,
                    plugin.info().version
                );

                let builder = plugin.get_builder();
                let missing_deps = builder.check_dependencies();
                if !missing_deps.is_empty() {
                    println!("\n  ⚠️  \x1b[1;33mMissing Plugin Dependencies:\x1b[0m");
                    for dep in &missing_deps {
                        println!("     \x1b[1;31m• {}\x1b[0m", dep);
                    }

                    if lang == crate::compiler::ProjectLanguage::Rust {
                        println!("\n  💡 \x1b[1;33mTo install the Rust plugin:\x1b[0m");
                        println!("     \x1b[1;37mcargo install wasmrust\x1b[0m");
                    }

                    println!("\x1b[1;34m╰\x1b[0m\n");
                    return None;
                }
            } else {
                match lang {
                    crate::compiler::ProjectLanguage::Rust => {
                        println!("\n  ⚠️  \x1b[1;33mRust plugin not found\x1b[0m");
                        println!("  💡 \x1b[1;33mInstall the wasmrust plugin:\x1b[0m");
                        println!("     \x1b[1;37mwasmrun plugin install wasmrust\x1b[0m");
                        println!("\n  ℹ️  \x1b[1;34mAfter installation, wasmrust will be auto-detected\x1b[0m");
                        println!("\x1b[1;34m╰\x1b[0m\n");
                        return None;
                    }
                    crate::compiler::ProjectLanguage::C
                    | crate::compiler::ProjectLanguage::Asc
                    | crate::compiler::ProjectLanguage::Python => {
                        println!("  🔧 \x1b[1;34mUsing built-in plugin\x1b[0m");
                    }
                    _ => {}
                }
            }

            let (builtin_count, external_count) = plugin_manager.plugin_counts();
            if external_count > 0 {
                println!(
                    "  📊 \x1b[1;34mPlugins:\x1b[0m {} built-in, {} external",
                    builtin_count, external_count
                );
            }
        }
        Err(e) => {
            eprintln!("  ⚠️ Warning: Failed to initialize plugin manager: {}", e);
        }
    }

    let temp_dir = std::env::temp_dir().join("wasmrun_temp");
    let temp_output_dir = temp_dir.to_str().unwrap_or("/tmp").to_string();

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
                println!("     \x1b[1;31m• {}\x1b[0m", tool);
            }
            println!(
                "\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m"
            );
            println!("\x1b[1;34m╰\x1b[0m\n");
            return None;
        }
    }

    if lang == crate::compiler::ProjectLanguage::Python {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ⚠️  \x1b[1;33mPython WebAssembly compilation coming soon!\x1b[0m");
        println!("  📝 \x1b[0;37mImplementing py2wasm integration for Python-to-WASM compilation.\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");
        return None;
    }

    Some((lang, temp_output_dir))
}

/// Compile a project
pub fn compile_project(
    project_path: &str,
    output_dir: &str,
    _lang: crate::compiler::ProjectLanguage,
    _watch: bool,
) -> Option<(String, bool, Option<String>)> {
    if let Ok(plugin_manager) = crate::plugin::PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
            println!("🔌 Compiling with {}", plugin.info().name);

            let builder = plugin.get_builder();
            let config = crate::compiler::builder::BuildConfig {
                project_path: project_path.to_string(),
                output_dir: output_dir.to_string(),
                verbose: false,
                optimization_level: OptimizationLevel::Release,
                target_type: TargetType::Standard,
            };

            match builder.build(&config) {
                Ok(result) => {
                    let is_wasm_bindgen = result.js_path.is_some();
                    return Some((result.wasm_path, is_wasm_bindgen, result.js_path));
                }
                Err(e) => {
                    eprintln!("❌ Plugin compilation failed: {}", e);
                    return None;
                }
            }
        }
    }

    match crate::compiler::compile_for_execution(project_path, output_dir) {
        Ok(result) => {
            let is_wasm_bindgen = result.contains(".js");
            let js_path = if is_wasm_bindgen {
                Some(result.clone())
            } else {
                None
            };

            Some((result, is_wasm_bindgen, js_path))
        }
        Err(e) => {
            eprintln!("❌ Compilation failed: {}", e);
            None
        }
    }
}
