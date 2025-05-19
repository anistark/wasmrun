use std::fs;
use std::path::Path;
use tiny_http::Server;

use super::handler;

/// Simple server for non-watching mode
pub fn serve_wasm_file(wasm_path: &str, port: u16, wasm_filename: &str) -> Result<(), String> {
    // Create HTTP server
    let server = Server::http(format!("0.0.0.0:{port}"))
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // Track connected clients for live reload
    let mut clients_to_reload = Vec::new();

    // Handle requests
    for request in server.incoming_requests() {
        handler::handle_request(
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

/// Server for wasm-bindgen files
pub fn serve_wasm_bindgen_files(
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
        handler::handle_request(
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

/// Helper function to handle wasm-bindgen files
pub fn handle_wasm_bindgen_files(
    js_path: &str,
    wasm_path: &str,
    port: u16,
    wasm_filename: &str,
) -> Result<(), String> {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅  \x1b[1;32mRunning wasm-bindgen project\x1b[0m");
    println!("  \x1b[0;37mJS File: {}\x1b[0m", js_path);
    println!("  \x1b[0;37mWASM File: {}\x1b[0m", wasm_path);
    println!("\x1b[1;34m╰\x1b[0m\n");

    // Run with wasm-bindgen support
    serve_wasm_bindgen_files(wasm_path, js_path, port, wasm_filename)
}

/// Inspect a WebAssembly file for wasm-bindgen patterns
#[allow(dead_code)]
fn check_for_wasm_bindgen_patterns(wasm_bytes: &[u8]) -> bool {
    // Convert the binary to a string for pattern matching
    let wasm_content = String::from_utf8_lossy(wasm_bytes);

    // Check for common wasm-bindgen patterns
    wasm_content.contains("wasm-bindgen")
        || wasm_content.contains("__wbindgen")
        || wasm_content.contains("wbg")
}

/// Look for a corresponding JS file for a WASM file
#[allow(dead_code)]
fn find_corresponding_js_file(wasm_path: &Path) -> Option<String> {
    // First try with the same base name
    let js_path = wasm_path.with_extension("js");
    if js_path.exists() {
        return Some(js_path.to_string_lossy().to_string());
    }

    // For _bg.wasm files, try finding the matching JS
    let file_name = wasm_path.file_name()?.to_string_lossy();
    if file_name.ends_with("_bg.wasm") {
        let stem = file_name.replace("_bg.wasm", "");
        let js_name = format!("{}.js", stem);
        let parent = wasm_path.parent()?;
        let js_path = parent.join(&js_name);

        if js_path.exists() {
            return Some(js_path.to_string_lossy().to_string());
        }
    }

    // Check directory for any JS files that might be related
    if let Some(dir) = wasm_path.parent() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "js") {
                    // Read the file and check if it references the WASM file
                    if let Ok(content) = fs::read_to_string(&path) {
                        if content.contains("wasm_bindgen") || content.contains("__wbindgen") {
                            return Some(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    None
}
