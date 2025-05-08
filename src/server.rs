use crate::compiler;
use crate::template::generate_html;
use crate::utils::content_type_header;
use crate::watcher;
use std::fs;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::channel,
    Arc,
};
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Request, Response, Server};

const PID_FILE: &str = "/tmp/chakra_server.pid";

//
// Public API
//

/// Run a WebAssembly file directly
pub fn run_wasm_file(path: &str, port: u16) {
    let path_obj = Path::new(path);

    if !path_obj.extension().map_or(false, |ext| ext == "wasm") {
        print_error(format!("Error: Not a WASM file: {}", path));
        println!("  \x1b[1;37mPlease specify a path to a .wasm file:\x1b[0m");
        println!("  \x1b[1;33mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
        return;
    }

    if let Err(e) = run_server(ServerConfig {
        wasm_path: path.to_string(),
        port,
        watch_mode: false,
        project_path: None,
        output_dir: None,
    }) {
        print_error(format!("Error Running Chakra Server: {}", e));
    }
}

/// Compile and run a project
pub fn run_project(path: &str, port: u16, language_override: Option<String>, watch: bool) {
    let path_obj = Path::new(path);

    // Handle WASM file for convenience
    if path_obj.is_file() && path_obj.extension().map_or(false, |ext| ext == "wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected WASM file: {}\x1b[0m", path);
        println!("  \x1b[0;37mRunning the WASM file directly...\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");

        run_wasm_file(path, port);
        return;
    }

    // Handle project directory
    if !path_obj.is_dir() {
        print_error(if !path_obj.exists() {
            format!("Error: Path not found: {}", path)
        } else {
            format!("Error: Not a WASM file or project directory: {}", path)
        });
        println!("  \x1b[1;37mPlease specify a path to a project directory or use --wasm for WASM files:\x1b[0m");
        println!("  \x1b[1;33mchakra --path /path/to/your/project/\x1b[0m");
        println!("  \x1b[1;33mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
        return;
    }

    // Detect project and setup
    let (lang, temp_output_dir) = match setup_project_compilation(path, language_override, watch) {
        Some(result) => result,
        None => return, // Error already printed
    };

    // Compile the project
    let wasm_path = match compile_project(path, &temp_output_dir, lang, watch) {
        Some(path) => path,
        None => return, // Error already printed
    };

    // Run the server
    let server_config = ServerConfig {
        wasm_path,
        port,
        watch_mode: watch,
        project_path: if watch { Some(path.to_string()) } else { None },
        output_dir: if watch {
            Some(temp_output_dir.to_string())
        } else {
            None
        },
    };

    if let Err(e) = run_server(server_config) {
        print_error(format!("Error Running Server: {}", e));
    }
}

/// Check if a server is currently running
pub fn is_server_running() -> bool {
    if !Path::new(PID_FILE).exists() {
        return false;
    }

    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            // Checking if a process exists
            let ps_command = Command::new("ps").arg("-p").arg(pid.to_string()).output();

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
        if Path::new(PID_FILE).exists() {
            if let Err(e) = fs::remove_file(PID_FILE) {
                return Err(format!(
                    "No server running, but failed to remove stale PID file: {e}"
                ));
            }
        }

        return Ok(());
    }

    let pid_str =
        fs::read_to_string(PID_FILE).map_err(|e| format!("Failed to read PID file: {}", e))?;

    let pid = pid_str
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse PID '{}': {}", pid_str.trim(), e))?;

    let kill_command = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to kill server process: {}", e))?;

    if kill_command.status.success() {
        fs::remove_file(PID_FILE).map_err(|e| format!("Failed to remove PID file: {e}"))?;
        println!("💀 Existing Chakra server terminated successfully.");
        Ok(())
    } else {
        // Failed to stop the server
        let error_msg = String::from_utf8_lossy(&kill_command.stderr);
        Err(format!("Failed to stop Chakra server: {}", error_msg))
    }
}

//
// Server Implementation
//

// Configuration struct for server setup
struct ServerConfig {
    wasm_path: String,
    port: u16,
    watch_mode: bool,
    project_path: Option<String>,
    output_dir: Option<String>,
}

// Combined server function
fn run_server(config: ServerConfig) -> Result<(), String> {
    // Check if a server is already running
    if is_server_running() {
        match stop_existing_server() {
            Ok(_) => println!("💀 Existing server stopped successfully."),
            Err(e) => eprintln!("❗ Warning when stopping existing server: {e}"),
        }
    }

    // Check if the port is available
    if !is_port_available(config.port) {
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
        let wasm_files = find_wasm_files(path_obj);
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
    print_server_info(
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
    fs::write(PID_FILE, pid.to_string())
        .map_err(|e| format!("Failed to write PID to {}: {}", PID_FILE, e))?;

    // Setup for watch mode if needed
    if config.watch_mode && config.project_path.is_some() && config.output_dir.is_some() {
        let project_path = config.project_path.unwrap();
        let output_dir = config.output_dir.unwrap();

        match watcher::ProjectWatcher::new(&project_path) {
            Ok(watcher) => {
                // Create channels for communication
                let (tx, rx) = channel();
                let reload_flag = Arc::new(AtomicBool::new(false));

                // Start server in a new thread
                let wasm_path_clone = config.wasm_path.clone();
                let port = config.port;
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

                            // Handle the request
                            handle_request(
                                request,
                                &wasm_filename,
                                &wasm_path_clone,
                                true,
                                &mut clients_to_reload,
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

                                    // Send reload signal to the server
                                    if let Ok(mut stream) =
                                        TcpStream::connect(format!("127.0.0.1:{}", port))
                                    {
                                        let reload_request =
                                            "GET /reload HTTP/1.1\r\nHost: localhost\r\n\r\n";
                                        let _ = stream.write_all(reload_request.as_bytes());
                                    }
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
            }
            Err(e) => {
                eprintln!("Failed to set up file watcher: {}", e);

                // Fall back to regular server
                serve_wasm_file(&config.wasm_path, config.port, &wasm_filename)?;
            }
        }
    } else {
        // Standard server without watching
        serve_wasm_file(&config.wasm_path, config.port, &wasm_filename)?;
    }

    // Clean up PID file
    if Path::new(PID_FILE).exists() {
        let _ = fs::remove_file(PID_FILE);
    }

    Ok(())
}

// Simple server function for non-watching mode
fn serve_wasm_file(wasm_path: &str, port: u16, wasm_filename: &str) -> Result<(), String> {
    // Create HTTP server
    let server = Server::http(format!("0.0.0.0:{port}"))
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // Track connected clients for live reload
    let mut clients_to_reload = Vec::new();

    // Handle requests
    for request in server.incoming_requests() {
        handle_request(
            request,
            wasm_filename,
            wasm_path,
            false,
            &mut clients_to_reload,
        );
    }

    Ok(())
}

//
// Helper Functions
//

/// Handle an incoming HTTP request
fn handle_request(
    request: Request,
    wasm_filename: &str,
    wasm_path: &str,
    watch_mode: bool,
    clients_to_reload: &mut Vec<String>,
) {
    let url = request.url().to_string();
    // Get client address safely
    let client_addr = match request.remote_addr() {
        Some(addr) => addr.to_string(),
        None => "unknown".to_string(),
    };

    println!("📝 Received request for: {}", url);

    if url == "/" {
        // Serve the main HTML page
        let html = generate_html(wasm_filename);
        let response = Response::from_string(html).with_header(content_type_header("text/html"));
        if let Err(e) = request.respond(response) {
            eprintln!("❗ Error sending HTML response: {}", e);
        }

        // Track this client for reload notifications
        if watch_mode && !clients_to_reload.contains(&client_addr) {
            clients_to_reload.push(client_addr);
        }
    } else if url == format!("/{}", wasm_filename) {
        // Serve the WASM file
        match fs::read(wasm_path) {
            Ok(wasm_bytes) => {
                println!(
                    "🔄 Serving WASM file: {} ({} bytes)",
                    wasm_filename,
                    wasm_bytes.len()
                );
                let response = Response::from_data(wasm_bytes)
                    .with_header(content_type_header("application/wasm"));
                if let Err(e) = request.respond(response) {
                    eprintln!("❗ Error sending WASM response: {}", e);
                }
            }
            Err(e) => {
                eprintln!("❗ Error reading WASM file: {}", e);
                let response = Response::from_string(format!("Error: {}", e))
                    .with_status_code(500)
                    .with_header(content_type_header("text/plain"));
                if let Err(e) = request.respond(response) {
                    eprintln!("❗ Error sending error response: {}", e);
                }
            }
        }
    } else if url == "/reload" {
        // Special reload endpoint
        println!("🔄 Handling reload request");

        // Send a special response to tell the browser to refresh
        let response = Response::from_string("reload")
            .with_header(Header::from_bytes(&b"X-Reload"[..], &b"true"[..]).unwrap())
            .with_header(content_type_header("text/plain"));

        if let Err(e) = request.respond(response) {
            eprintln!("❗ Error sending reload response: {}", e);
        }
    } else if url.starts_with("/assets/") {
        serve_asset(request, &url);
    } else {
        // 404 for all other requests
        let response = Response::from_string("404 Not Found")
            .with_status_code(404)
            .with_header(content_type_header("text/plain"));
        if let Err(e) = request.respond(response) {
            eprintln!("❗ Error sending 404 response: {}", e);
        }
    }
}

/// Serve a static asset file
fn serve_asset(request: Request, url: &str) {
    let asset_filename = url.strip_prefix("/assets/").unwrap_or("");
    let asset_path = format!("./assets/{}", asset_filename);

    let content_type = if url.ends_with(".png") {
        "image/png"
    } else if url.ends_with(".jpg") || url.ends_with(".jpeg") {
        "image/jpeg"
    } else if url.ends_with(".svg") {
        "image/svg+xml"
    } else if url.ends_with(".gif") {
        "image/gif"
    } else if url.ends_with(".css") {
        "text/css"
    } else if url.ends_with(".js") {
        "application/javascript"
    } else {
        "application/octet-stream"
    };

    match fs::read(&asset_path) {
        Ok(asset_bytes) => {
            println!(
                "🖼️ Successfully serving asset: {} ({} bytes)",
                asset_path,
                asset_bytes.len()
            );
            let response =
                Response::from_data(asset_bytes).with_header(content_type_header(content_type));
            if let Err(e) = request.respond(response) {
                eprintln!("‼️ Error sending asset response: {}", e);
            }
        }
        Err(e) => {
            eprintln!(
                "‼️ Error reading asset file {}: {} (does the file exist?)",
                asset_path, e
            );

            check_assets_directory();

            let response = Response::from_string(format!("Asset not found: {}", e))
                .with_status_code(404)
                .with_header(content_type_header("text/plain"));
            if let Err(e) = request.respond(response) {
                eprintln!("‼️ Error sending asset error response: {}", e);
            }
        }
    }
}

/// Check if assets directory exists
fn check_assets_directory() {
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

/// Set up project compilation environment and detect language
fn setup_project_compilation(
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
fn compile_project(
    path: &str,
    output_dir: &str,
    _lang: compiler::ProjectLanguage,
    watch: bool,
) -> Option<String> {
    // Compile the project
    match compiler::compile_for_execution(path, output_dir) {
        Ok(wasm_path) => {
            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  ✅ \x1b[1;36mCompilation Successful\x1b[0m\n");
            println!(
                "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                wasm_path
            );

            if watch {
                println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m (Press Ctrl+C to stop)");
            }

            println!("\x1b[1;34m╰\x1b[0m\n");

            Some(wasm_path)
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

/// Print server information
fn print_server_info(
    url: &str,
    port: u16,
    wasm_filename: &str,
    file_size: &str,
    absolute_path: &str,
    watch_mode: bool,
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🌀 \x1b[1;36mChakra WASM Server\x1b[0m\n");
    println!("  🚀 \x1b[1;34mServer URL:\x1b[0m \x1b[4;36m{}\x1b[0m", url);
    println!(
        "  🔌 \x1b[1;34mListening on port:\x1b[0m \x1b[1;33m{}\x1b[0m",
        port
    );
    println!(
        "  📦 \x1b[1;34mServing file:\x1b[0m \x1b[1;32m{}\x1b[0m",
        wasm_filename
    );
    println!(
        "  💾 \x1b[1;34mFile size:\x1b[0m \x1b[0;37m{}\x1b[0m",
        file_size
    );
    println!(
        "  🔍 \x1b[1;34mFull path:\x1b[0m \x1b[0;37m{:.45}\x1b[0m",
        absolute_path
    );
    println!(
        "  🆔 \x1b[1;34mServer PID:\x1b[0m \x1b[0;37m{}\x1b[0m",
        std::process::id()
    );

    if watch_mode {
        println!("\n  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m");
    }

    println!("\n  \x1b[0;90mPress Ctrl+C to stop the server\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
    println!("\n🌐 Opening browser...");
}

/// Print an error message in a formatted box
fn print_error(message: String) {
    eprintln!("\n\x1b[1;34m╭\x1b[0m");
    eprintln!("  ❌ \x1b[1;31m{}\x1b[0m", message);
    eprintln!("\x1b[1;34m╰\x1b[0m");
}

/// Find WASM files in a directory
fn find_wasm_files(dir_path: &Path) -> Vec<String> {
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
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("0.0.0.0:{port}")).is_ok()
}
