use std::fs;
use std::path::Path;
use std::process::Command;
use tiny_http::{Server, Response, Request};
use crate::template::generate_html;
use crate::utils::content_type_header;
use std::net::TcpListener;

const PID_FILE: &str = "/tmp/chakra_server.pid";

/// Helper function to check if a server is currently running
pub fn is_server_running() -> bool {
    if !Path::new(PID_FILE).exists() {
        return false;
    }
    
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            // On Unix-like systems, checking if a process exists
            let ps_command = Command::new("ps")
                .arg("-p")
                .arg(pid.to_string())
                .output();
                
            if let Ok(output) = ps_command {
                // If ps returns success and has more than just the header line,
                // the process exists
                return output.status.success() && 
                       String::from_utf8_lossy(&output.stdout).lines().count() > 1;
            }
        }
    }
    
    false
}

/// Helper function to stop the existing server using the PID stored in the file
pub fn stop_existing_server() -> Result<(), String> {
    // Check if the server is running first
    if !is_server_running() {
        // No server is running, clean up any stale PID file
        if Path::new(PID_FILE).exists() {
            if let Err(e) = fs::remove_file(PID_FILE) {
                return Err(format!("No server running, but failed to remove stale PID file: {e}"));
            }
        }
        
        // Not an error, just return success as there's no server to stop
        return Ok(());
    }
    
    // At this point, we know the server is running and the PID file exists
    let pid_str = fs::read_to_string(PID_FILE)
        .map_err(|e| format!("Failed to read PID file: {}", e))?;
    
    let pid = pid_str.trim().parse::<u32>()
        .map_err(|e| format!("Failed to parse PID '{}': {}", pid_str.trim(), e))?;
    
    let kill_command = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to kill server process: {}", e))?;

    if kill_command.status.success() {
        fs::remove_file(PID_FILE).map_err(|e| format!("Failed to remove PID file: {e}"))?;
        println!("💀 Existing Chakra server terminated successfully.");
        return Ok(());
    } else {
        // Get the error output from kill command
        let error_msg = String::from_utf8_lossy(&kill_command.stderr);
        return Err(format!("Failed to stop Chakra server: {}", error_msg));
    }
}

/// Function to check if the given port is available (not in use)
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("0.0.0.0:{port}")).is_ok()
}

/// Run server with the given WASM file and port
pub fn run_server(path: &str, port: u16) -> Result<(), String> {
    // First, try to stop any running server
    if is_server_running() {
        match stop_existing_server() {
            Ok(_) => println!("💀 Existing server stopped successfully."),
            Err(e) => eprintln!("❗ Warning when stopping existing server: {e}"),
        }
    }

    // Check if the port is available
    if !is_port_available(port) {
        return Err(format!("❗ Port {} is already in use, please choose a different port.", port));
    }

    // Verify the WASM file exists
    if !Path::new(path).exists() {
        return Err(format!("❗ WASM file not found at path: {}", path));
    }

    // Get the WASM filename
    let wasm_filename = Path::new(path)
        .file_name()
        .ok_or_else(|| "Invalid path".to_string())?
        .to_string_lossy()
        .to_string();
    
    // Get absolute path to display
    let absolute_path = fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
        
    // Get file size in a human-readable format
    let file_size = match fs::metadata(path) {
        Ok(metadata) => {
            let bytes = metadata.len();
            if bytes < 1024 {
                format!("{} bytes", bytes)
            } else if bytes < 1024 * 1024 {
                format!("{:.2} KB", bytes as f64 / 1024.0)
            } else {
                format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
            }
        },
        Err(_) => "unknown size".to_string(),
    };

    // Display a nice box with server info - corners only design
    let url = format!("http://localhost:{}", port);
    
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🌀 \x1b[1;36mChakra WASM Server\x1b[0m");
    println!();
    println!("  🚀 \x1b[1;34mServer URL:\x1b[0m \x1b[4;36m{}\x1b[0m", url);
    println!("  🔌 \x1b[1;34mListening on port:\x1b[0m \x1b[1;33m{}\x1b[0m", port);
    println!("  📦 \x1b[1;34mServing file:\x1b[0m \x1b[1;32m{}\x1b[0m", wasm_filename);
    println!("  💾 \x1b[1;34mFile size:\x1b[0m \x1b[0;37m{}\x1b[0m", file_size);
    println!("  🔍 \x1b[1;34mFull path:\x1b[0m \x1b[0;37m{:.45}\x1b[0m", absolute_path);
    println!("  🆔 \x1b[1;34mServer PID:\x1b[0m \x1b[0;37m{}\x1b[0m", std::process::id());
    println!();
    println!("  \x1b[0;90mPress Ctrl+C to stop the server\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
    println!("\n🌐 Opening browser...");

    if let Err(e) = webbrowser::open(&url) {
        println!("❗ Failed to open browser automatically: {e}");
    }

    // Store the current process PID in /tmp/
    let pid = std::process::id();
    fs::write(PID_FILE, pid.to_string()).map_err(|e| format!("Failed to write PID to {}: {}", PID_FILE, e))?;

    // Create the HTTP server
    let server = Server::http(format!("0.0.0.0:{port}"))
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // Monitor incoming requests
    for request in server.incoming_requests() {
        handle_request(request, &wasm_filename, path);
    }

    Ok(())
}

fn handle_request(request: Request, wasm_filename: &str, wasm_path: &str) {
    let url = request.url();
    
    println!("📝 Received request for: {}", url);

    if url == "/" {
        // Serve the main HTML page
        let html = generate_html(wasm_filename);
        let response = Response::from_string(html).with_header(content_type_header("text/html"));
        if let Err(e) = request.respond(response) {
            eprintln!("❗ Error sending HTML response: {}", e);
        }
    } else if url == format!("/{}", wasm_filename) {
        // Serve the WASM file
        match fs::read(wasm_path) {
            Ok(wasm_bytes) => {
                println!("🔄 Serving WASM file: {} ({} bytes)", wasm_filename, wasm_bytes.len());
                let response = Response::from_data(wasm_bytes)
                    .with_header(content_type_header("application/wasm"));
                if let Err(e) = request.respond(response) {
                    eprintln!("❗ Error sending WASM response: {}", e);
                }
            },
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