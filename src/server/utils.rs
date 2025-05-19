use std::fs;
use std::net::TcpListener;
use std::path::Path;

/// Generate a Content-Type header
pub fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}

/// Print server information
pub fn print_server_info(
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
pub fn print_error(message: String) {
    eprintln!("\n\x1b[1;34m╭\x1b[0m");
    eprintln!("  ❌ \x1b[1;31m{}\x1b[0m", message);
    eprintln!("\x1b[1;34m╰\x1b[0m");
}

/// Find WASM files in a directory
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
