use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Generate a Content-Type header
pub fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_header() {
        let header = content_type_header("text/html");
        assert_eq!(header.field.as_str().to_ascii_lowercase(), "content-type");
        assert_eq!(header.value.as_str(), "text/html");

        let header = content_type_header("application/wasm");
        assert_eq!(header.value.as_str(), "application/wasm");
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
}
