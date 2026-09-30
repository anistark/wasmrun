use crate::config::{FileInfo, ServerInfo};
use crate::error::Result;
use crate::utils::CommandExecutor;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Generate a Content-Type header
pub fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}

/// Find WASM files in a directory
#[allow(dead_code)] // TODO: Future WASM file discovery system
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

/// Wait for server to be ready and then open browser
pub fn open_browser_when_ready(port: u16) {
    let url = format!("http://localhost:{port}");

    thread::spawn(move || {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(30); // 30 second timeout
        let check_interval = Duration::from_millis(100);

        println!("\n🌐 \x1b[1;36mWaiting for server to be ready...\x1b[0m");

        loop {
            // Check if we can connect to the server
            if let Ok(stream) = std::net::TcpStream::connect(format!("localhost:{port}")) {
                drop(stream);

                // Server is ready, open browser
                println!("✅ \x1b[1;32mServer is ready! Opening browser...\x1b[0m");

                if let Err(e) = webbrowser::open(&url) {
                    println!("❗ \x1b[1;33mFailed to open browser automatically: {e}\x1b[0m");
                    println!("🔗 \x1b[1;34mManually open:\x1b[0m \x1b[4;36m{url}\x1b[0m");
                } else {
                    println!("✅ \x1b[1;32mBrowser opened successfully!\x1b[0m");
                }
                break;
            }

            // Check timeout
            if start_time.elapsed() > timeout {
                println!("⏰ \x1b[1;33mTimeout waiting for server. Please open manually:\x1b[0m \x1b[4;36m{url}\x1b[0m");
                break;
            }

            thread::sleep(check_interval);
        }
    });
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

/// Utility functions for server operations
pub struct ServerUtils;

impl ServerUtils {
    #[allow(dead_code)] // TODO: Future file metadata system
    pub fn get_file_info(path: &str) -> Result<FileInfo> {
        let path_obj = Path::new(path);
        let metadata = fs::metadata(path)?;

        let filename = path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let absolute_path = fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        let file_size_bytes = metadata.len();
        let file_size = CommandExecutor::format_file_size(file_size_bytes);

        Ok(FileInfo {
            filename,
            absolute_path,
            file_size,
            file_size_bytes,
        })
    }
}

/// Get Server Info
#[allow(dead_code)] // TODO: Future server information display
pub fn print_server_info(
    url: &str,
    port: u16,
    wasm_filename: &str,
    file_size: &str,
    absolute_path: &str,
    watch_mode: bool,
) {
    if let Ok(server_info) = ServerInfo::for_wasm_file(absolute_path, port, watch_mode) {
        server_info.print_server_startup();
    } else {
        // Basic output if analysis fails
        print_basic_server_info(
            url,
            port,
            wasm_filename,
            file_size,
            absolute_path,
            watch_mode,
        );
    }
}

/// Basic server info printing
#[allow(dead_code)] // TODO: Future basic server info display
fn print_basic_server_info(
    url: &str,
    port: u16,
    wasm_filename: &str,
    file_size: &str,
    absolute_path: &str,
    watch_mode: bool,
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🅦 \x1b[1;36mWasmrun WASM Server\x1b[0m\n");
    println!("  🚀 \x1b[1;34mServer URL:\x1b[0m \x1b[4;36m{url}\x1b[0m");
    println!("  🔌 \x1b[1;34mListening on port:\x1b[0m \x1b[1;33m{port}\x1b[0m");
    println!("  📦 \x1b[1;34mServing file:\x1b[0m \x1b[1;32m{wasm_filename}\x1b[0m");
    println!("  💾 \x1b[1;34mFile size:\x1b[0m \x1b[0;37m{file_size}\x1b[0m");
    println!("  🔍 \x1b[1;34mFull path:\x1b[0m \x1b[0;37m{absolute_path:.45}\x1b[0m");
    println!(
        "  ℹ️ \x1b[1;34mServer PID:\x1b[0m \x1b[0;37m{}\x1b[0m",
        std::process::id()
    );

    if watch_mode {
        println!("\n  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m");
    }

    println!("\n  \x1b[0;90mPress Ctrl+C to stop the server\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
    println!("\n🌐 Opening browser...");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_content_type_header() {
        let header = content_type_header("text/html");
        assert_eq!(header.field.as_str().to_ascii_lowercase(), "content-type");
        assert_eq!(header.value.as_str(), "text/html");

        let header = content_type_header("application/wasm");
        assert_eq!(header.value.as_str(), "application/wasm");
    }

    #[test]
    fn test_find_wasm_files_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let wasm_files = find_wasm_files(temp_dir.path());
        assert!(wasm_files.is_empty());
    }

    #[test]
    fn test_find_wasm_files_with_wasm_files() {
        let temp_dir = tempdir().unwrap();

        // Create some WASM files
        File::create(temp_dir.path().join("test1.wasm")).unwrap();
        File::create(temp_dir.path().join("test2.wasm")).unwrap();
        File::create(temp_dir.path().join("other.js")).unwrap(); // Non-WASM file

        let wasm_files = find_wasm_files(temp_dir.path());
        assert_eq!(wasm_files.len(), 2);
        assert!(wasm_files.iter().all(|f| f.ends_with(".wasm")));
    }

    #[test]
    fn test_find_wasm_files_recursive() {
        let temp_dir = tempdir().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        // Create WASM files in subdirectory
        File::create(sub_dir.join("nested.wasm")).unwrap();
        File::create(temp_dir.path().join("root.wasm")).unwrap();

        let wasm_files = find_wasm_files(temp_dir.path());
        assert_eq!(wasm_files.len(), 2);
        assert!(wasm_files.iter().any(|f| f.contains("nested.wasm")));
        assert!(wasm_files.iter().any(|f| f.contains("root.wasm")));
    }

    #[test]
    fn test_determine_content_type() {
        let test_cases = vec![
            ("test.html", "text/html"),
            ("style.css", "text/css"),
            ("script.js", "application/javascript"),
            ("data.json", "application/json"),
            ("module.wasm", "application/wasm"),
            ("image.png", "image/png"),
            ("photo.jpg", "image/jpeg"),
            ("photo.jpeg", "image/jpeg"),
            ("icon.svg", "image/svg+xml"),
            ("favicon.ico", "image/x-icon"),
            ("readme.txt", "text/plain"),
            ("doc.md", "text/markdown"),
            ("source.map", "application/json"),
            ("unknown.xyz", "application/octet-stream"),
        ];

        for (filename, expected) in test_cases {
            let path = std::path::Path::new(filename);
            assert_eq!(
                determine_content_type(path),
                expected,
                "Failed for {filename}"
            );
        }
    }

    #[test]
    fn test_determine_content_type_no_extension() {
        let path = std::path::Path::new("filename_without_extension");
        assert_eq!(determine_content_type(path), "application/octet-stream");
    }

    #[test]
    fn test_determine_content_type_case_insensitive() {
        let test_cases = vec![
            ("TEST.html", "text/html"),
            ("STYLE.css", "text/css"),
            ("MODULE.wasm", "application/wasm"),
            ("Image.png", "image/png"),
        ];

        for (filename, expected) in test_cases {
            let path = std::path::Path::new(filename);
            assert_eq!(
                determine_content_type(path),
                expected,
                "Failed for {filename}"
            );
        }
    }

    #[test]
    fn test_server_utils_get_file_info() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();

        let result = ServerUtils::get_file_info(test_file.to_str().unwrap());
        assert!(result.is_ok());

        let file_info = result.unwrap();
        assert_eq!(file_info.filename, "test.txt");
        assert!(file_info.absolute_path.contains("test.txt"));
        assert!(file_info.file_size_bytes > 0);
        assert!(!file_info.file_size.is_empty());
    }

    #[test]
    fn test_server_utils_get_file_info_nonexistent() {
        let result = ServerUtils::get_file_info("/nonexistent/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_print_server_info() {
        let temp_dir = tempdir().unwrap();
        let test_wasm = temp_dir.path().join("test.wasm");
        let mut file = File::create(&test_wasm).unwrap();
        file.write_all(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00])
            .unwrap(); // Valid WASM header

        // Should not crash
        print_server_info(
            "http://localhost:8080",
            8080,
            "test.wasm",
            "8 bytes",
            test_wasm.to_str().unwrap(),
            false,
        );

        // Test with watch mode
        print_server_info(
            "http://localhost:8081",
            8081,
            "test.wasm",
            "8 bytes",
            test_wasm.to_str().unwrap(),
            true,
        );
    }

    #[test]
    fn test_print_basic_server_info() {
        // Test the basic server info function
        print_basic_server_info(
            "http://localhost:8080",
            8080,
            "test.wasm",
            "8 bytes",
            "/path/to/test.wasm",
            false,
        );

        print_basic_server_info(
            "http://localhost:8081",
            8081,
            "test.wasm",
            "8 bytes",
            "/path/to/test.wasm",
            true,
        );

        // Should complete without panicking
    }
}
