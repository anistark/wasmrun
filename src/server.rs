use crate::compiler;
use crate::template::{generate_html, generate_html_wasm_bindgen};
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
        // Check if it's a JavaScript file that might be wasm-bindgen output
        if path_obj.extension().map_or(false, |ext| ext == "js") {
            // Look for a corresponding .wasm file
            let wasm_path = path_obj.with_extension("wasm");

            // Check for regular .js -> .wasm pattern
            if wasm_path.exists() {
                handle_wasm_bindgen_files(path, wasm_path.to_str().unwrap(), port);
                return;
            }

            // Check for .js -> _bg.wasm pattern
            let file_stem = path_obj.file_stem().unwrap().to_string_lossy();
            let bg_wasm_path = path_obj.with_file_name(format!("{}_bg.wasm", file_stem));
            if bg_wasm_path.exists() {
                handle_wasm_bindgen_files(path, bg_wasm_path.to_str().unwrap(), port);
                return;
            }
        }

        print_error(format!("Error: Not a WASM file: {}", path));
        println!("  \x1b[1;37mPlease specify a path to a .wasm file:\x1b[0m");
        println!("  \x1b[1;33mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
        return;
    }

    // WASM file handling enhancement for _bg.wasm files
    let file_name = path_obj.file_name().unwrap().to_string_lossy();
    if file_name.ends_with("_bg.wasm") {
        // This is a strong indicator of wasm-bindgen output
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!(
            "  ℹ️  \x1b[1;34mDetected wasm-bindgen _bg.wasm file: {}\x1b[0m",
            path
        );

        // Look for the corresponding .js file
        // Remove _bg.wasm and add .js
        let base_name = file_name.trim_end_matches("_bg.wasm");
        let js_file_name = format!("{}.js", base_name);
        let js_path = path_obj.with_file_name(&js_file_name);

        if js_path.exists() {
            println!(
                "  \x1b[0;37mFound corresponding JS file: {}\x1b[0m",
                js_path.display()
            );
            println!("  \x1b[1;32mRunning with wasm-bindgen support\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m\n");

            handle_wasm_bindgen_files(js_path.to_str().unwrap(), path, port);
            return;
        } else {
            println!(
                "  \x1b[1;33mWarning: Could not find corresponding JS file: {}\x1b[0m",
                js_path.display()
            );
            println!("  \x1b[0;37mLooking for other JS files in the same directory...\x1b[0m");

            // Look for any .js file in the same directory that might be the glue code
            let dir = path_obj.parent().unwrap_or_else(|| Path::new("."));
            let mut found_js = None;

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.extension().map_or(false, |ext| ext == "js") {
                        // Check if this JS file contains wasm-bindgen signatures
                        if let Ok(js_content) = fs::read_to_string(&entry_path) {
                            if js_content.contains("wasm_bindgen")
                                || js_content.contains("__wbindgen")
                                || js_content.contains("wbg")
                            {
                                found_js = Some(entry_path);
                                break;
                            }
                        }
                    }
                }
            }

            if let Some(js_file) = found_js {
                println!(
                    "  \x1b[1;32mFound potential wasm-bindgen JS file: {}\x1b[0m",
                    js_file.display()
                );
                println!("\x1b[1;34m╰\x1b[0m\n");

                handle_wasm_bindgen_files(js_file.to_str().unwrap(), path, port);
                return;
            }

            println!(
                "  \x1b[1;33mNo matching JS file found. Attempting to run WASM directly...\x1b[0m"
            );
            println!("  \x1b[0;37mNote: Running wasm-bindgen modules without JS glue code may fail.\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m\n");
        }
    }

    // Check for any wasm-bindgen JS file for this WASM file
    let js_path = path_obj.with_extension("js");
    if js_path.exists() {
        // This might be a wasm-bindgen output
        if let Ok(js_content) = fs::read_to_string(&js_path) {
            if js_content.contains("wasm_bindgen") || js_content.contains("__wbindgen") {
                handle_wasm_bindgen_files(js_path.to_str().unwrap(), path, port);
                return;
            }
        }
    }

    // Standard WASM file handling
    if let Err(e) = run_server(ServerConfig {
        wasm_path: path.to_string(),
        js_path: None,
        port,
        watch_mode: false,
        project_path: None,
        output_dir: None,
    }) {
        print_error(format!("Error Running Chakra Server: {}", e));
    }
}

// Helper function to handle wasm-bindgen files
fn handle_wasm_bindgen_files(js_path: &str, wasm_path: &str, port: u16) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅  \x1b[1;32mRunning wasm-bindgen project\x1b[0m");
    println!("  \x1b[0;37mJS File: {}\x1b[0m", js_path);
    println!("  \x1b[0;37mWASM File: {}\x1b[0m", wasm_path);
    println!("\x1b[1;34m╰\x1b[0m\n");

    // Run with wasm-bindgen support
    if let Err(e) = run_server(ServerConfig {
        wasm_path: wasm_path.to_string(),
        js_path: Some(js_path.to_string()),
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

    // Handle JS file that might be wasm-bindgen output
    if path_obj.is_file() && path_obj.extension().map_or(false, |ext| ext == "js") {
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
            if let Ok(js_content) = fs::read_to_string(path) {
                if js_content.contains("wasm_bindgen") || js_content.contains("__wbindgen") {
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ✅  \x1b[1;32mConfirmed wasm-bindgen project\x1b[0m");
                    println!("  \x1b[0;37mRunning with wasm-bindgen support\x1b[0m");
                    println!("\x1b[1;34m╰\x1b[0m\n");

                    // Run with wasm-bindgen support
                    if let Err(e) = run_server(ServerConfig {
                        wasm_path: wasm_path.to_str().unwrap().to_string(),
                        js_path: Some(path.to_string()),
                        port,
                        watch_mode: watch,
                        project_path: None,
                        output_dir: None,
                    }) {
                        print_error(format!("Error Running Chakra Server: {}", e));
                    }

                    return;
                }
            }

            // If not confirmed as wasm-bindgen, run as regular WASM
            run_wasm_file(wasm_path.to_str().unwrap(), port);
            return;
        }
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
    let result = compile_project(path, &temp_output_dir, lang, watch);

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

        if let Err(e) = run_server(server_config) {
            print_error(format!("Error Running Server: {}", e));
        }
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
    js_path: Option<String>,
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
                                js_filename.as_deref(),
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

                if config.js_path.is_some() {
                    serve_wasm_bindgen_files(
                        &config.wasm_path,
                        config.js_path.as_ref().unwrap(),
                        config.port,
                        &wasm_filename,
                    )?;
                } else {
                    serve_wasm_file(&config.wasm_path, config.port, &wasm_filename)?;
                }
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
            None, // No JS file for standard WASM
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
    js_filename: Option<&str>,
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
        let html = if let Some(js_file) = js_filename {
            // This is a wasm-bindgen project, use the wasm-bindgen template
            generate_html_wasm_bindgen(js_file, wasm_filename)
        } else {
            // Regular wasm file, use standard template
            generate_html(wasm_filename)
        };

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
        serve_file(request, wasm_path, "application/wasm");
    } else if let Some(js_file) = js_filename {
        // Handle JS file requests for wasm-bindgen
        if url == format!("/{}", js_file) {
            let js_path = Path::new(wasm_path).parent().unwrap().join(js_file);
            serve_file(request, js_path.to_str().unwrap(), "application/javascript");
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
        // For any other files, try to serve them from the same directory as the wasm file
        let base_dir = Path::new(wasm_path).parent().unwrap();
        let requested_file = base_dir.join(url.trim_start_matches('/'));

        if requested_file.exists() && requested_file.is_file() {
            // Determine content type based on extension
            let content_type = determine_content_type(&requested_file);
            serve_file(request, requested_file.to_str().unwrap(), content_type);
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
}

// Helper function to serve a file
fn serve_file(request: Request, file_path: &str, content_type: &str) {
    match fs::read(file_path) {
        Ok(file_bytes) => {
            println!(
                "🔄 Serving file: {} ({} bytes, content-type: {})",
                file_path,
                file_bytes.len(),
                content_type
            );
            let response =
                Response::from_data(file_bytes).with_header(content_type_header(content_type));
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending file response: {}", e);
            }
        }
        Err(e) => {
            eprintln!("❗ Error reading file {}: {}", file_path, e);
            let response = Response::from_string(format!("Error: {}", e))
                .with_status_code(500)
                .with_header(content_type_header("text/plain"));
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending error response: {}", e);
            }
        }
    }
}

// Function to determine content type based on file extension
fn determine_content_type(path: &Path) -> &'static str {
    if let Some(extension) = path.extension() {
        match extension.to_string_lossy().to_lowercase().as_str() {
            "html" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "wasm" => "application/wasm",
            _ => "application/octet-stream",
        }
    } else {
        "application/octet-stream"
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

/// Serve WASM files with wasm-bindgen support
fn serve_wasm_bindgen_files(
    wasm_path: &str,
    js_path: &str,
    port: u16,
    wasm_filename: &str,
) -> Result<(), String> {
    // Create HTTP server
    let server = Server::http(format!("0.0.0.0:{port}"))
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // Get the JS filename
    let js_path_obj = Path::new(js_path);
    let js_filename = js_path_obj
        .file_name()
        .ok_or_else(|| "Invalid JS path".to_string())?
        .to_string_lossy()
        .to_string();

    // Track connected clients for live reload
    let mut clients_to_reload = Vec::new();

    // Handle requests
    for request in server.incoming_requests() {
        handle_request(
            request,
            Some(&js_filename),
            wasm_filename,
            wasm_path,
            false,
            &mut clients_to_reload,
        );
    }

    Ok(())
}
