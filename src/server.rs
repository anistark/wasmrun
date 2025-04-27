use std::fs;
use std::path::Path;
use std::process::Command;
use tiny_http::{Server, Response, Request};
use crate::template::generate_html;
use crate::utils::content_type_header;
use std::net::TcpListener;

const PID_FILE: &str = "/tmp/chakra_server.pid";

/// Helper function to stop the existing server using the PID stored in the file
pub fn stop_existing_server() -> Result<(), String> {
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
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
                return Err("Failed to stop Chakra server.".to_string());
            }
        } else {
            return Err("Failed to parse PID.".to_string());
        }
    }

    Ok(())  // If no PID is stored, it's safe to proceed
}

/// Function to check if the given port is available (not in use)
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("0.0.0.0:{port}")).is_ok()
}

/// Run server with the given WASM file and port
pub fn run_server(path: &str, port: u16) -> Result<(), String> {
    // First, stop any running server
    if let Err(e) = stop_existing_server() {
        eprintln!("❗ Error stopping the server: {e}");
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

    println!("🚀 Chakra server running at http://localhost:{port}");
    println!("📦 Serving WASM file: {}", wasm_filename);

    if let Err(e) = webbrowser::open(&format!("http://localhost:{port}")) {
        println!("❗ Failed to open browser automatically: {e}");
    }

    // Store the current process PID in /tmp/
    let pid = std::process::id();
    fs::write(PID_FILE, pid.to_string()).map_err(|e| format!("Failed to write PID to {}: {}", PID_FILE, e))?;
    println!("📝 PID file stored at: {}", PID_FILE);

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
