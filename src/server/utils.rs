use crate::error::Result;
use crate::utils::wasm_analysis::{ProjectAnalysis, WasmAnalysis};
use std::fs;
use std::net::TcpListener;
use std::path::Path;

/// Generate a Content-Type header
pub fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
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

/// Server information display with comprehensive analysis
pub struct ServerInfo {
    pub url: String,
    pub port: u16,
    pub server_pid: u32,
    pub watch_mode: bool,
    pub content_type: ContentType,
}

#[derive(Debug)]
pub enum ContentType {
    WasmFile(WasmAnalysis),
    Project(ProjectAnalysis),
    WebApp(ProjectAnalysis),
}

impl ServerInfo {
    pub fn for_wasm_file(wasm_path: &str, port: u16, watch_mode: bool) -> Result<Self> {
        let analysis = WasmAnalysis::analyze(wasm_path)?;

        Ok(Self {
            url: format!("http://localhost:{}", port),
            port,
            server_pid: std::process::id(),
            watch_mode,
            content_type: ContentType::WasmFile(analysis),
        })
    }

    pub fn for_project(project_path: &str, port: u16, watch_mode: bool) -> Result<Self> {
        let analysis = ProjectAnalysis::analyze(project_path)?;

        let content_type = if analysis.is_web_app {
            ContentType::WebApp(analysis)
        } else {
            ContentType::Project(analysis)
        };

        Ok(Self {
            url: format!("http://localhost:{}", port),
            port,
            server_pid: std::process::id(),
            watch_mode,
            content_type,
        })
    }

    /// Print comprehensive server startup information
    pub fn print_server_startup(&self) {
        // Clear screen for better presentation
        print!("\x1b[2J\x1b[H");

        // Print the main header
        self.print_header();

        // Print content-specific analysis
        match &self.content_type {
            ContentType::WasmFile(analysis) => {
                analysis.print_analysis();
                self.print_wasm_server_info();
            }
            ContentType::Project(analysis) => {
                analysis.print_analysis();
                self.print_project_server_info();
            }
            ContentType::WebApp(analysis) => {
                analysis.print_analysis();
                self.print_webapp_server_info();
            }
        }

        // Print server details
        self.print_server_details();

        // Open browser
        self.open_browser();
    }

    fn print_header(&self) {
        println!("\n\x1b[1;32m");
        println!("   ░█████╗░██╗░░██╗░█████╗░██╗░░██╗██████╗░░█████╗░");
        println!("   ██╔══██╗██║░░██║██╔══██╗██║░██╔╝██╔══██╗██╔══██╗");
        println!("   ██║░░╚═╝███████║███████║█████═╝░██████╔╝███████║");
        println!("   ██║░░██╗██╔══██║██╔══██║██╔═██╗░██╔══██╗██╔══██║");
        println!("   ╚█████╔╝██║░░██║██║░░██║██║░╚██╗██║░░██║██║░░██║");
        println!("   ░╚════╝░╚═╝░░╚═╝╚═╝░░╚═╝╚═╝░░╚═╝╚═╝░░╚═╝╚═╝░░╚═╝");
        println!("\x1b[0m");
        println!("   \x1b[1;34m🌟 WebAssembly Development Server\x1b[0m");

        let content_description = match &self.content_type {
            ContentType::WasmFile(analysis) => analysis.get_summary(),
            ContentType::Project(analysis) => analysis.get_summary(),
            ContentType::WebApp(analysis) => {
                format!("🌐 {} (Web Application)", analysis.get_summary())
            }
        };

        println!("   \x1b[0;37m{}\x1b[0m\n", content_description);
    }

    fn print_wasm_server_info(&self) {
        println!(
            "\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🚀 \x1b[1;36mWASM Server Configuration\x1b[0m                              \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mServer Mode:\x1b[0m \x1b[1;32mWASM File Execution\x1b[0m                     \x1b[1;34m│\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mRuntime:\x1b[0m \x1b[1;33mBrowser-based with full WASI support\x1b[0m         \x1b[1;34m│\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mFeatures:\x1b[0m \x1b[1;32mVirtual filesystem, Console I/O, Debugging\x1b[0m   \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    fn print_project_server_info(&self) {
        println!(
            "\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🚀 \x1b[1;36mProject Development Server\x1b[0m                             \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mServer Mode:\x1b[0m \x1b[1;32mCompile & Run\x1b[0m                              \x1b[1;34m│\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mBuild System:\x1b[0m \x1b[1;33mAutomatic compilation to WASM\x1b[0m           \x1b[1;34m│\x1b[0m");

        if self.watch_mode {
            println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32m✓ Live reload on file changes\x1b[0m             \x1b[1;34m│\x1b[0m");
        } else {
            println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mWatch Mode:\x1b[0m \x1b[0;37mDisabled\x1b[0m                                 \x1b[1;34m│\x1b[0m");
        }

        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mFeatures:\x1b[0m \x1b[1;32mFull WASI support, Debug console, Hot reload\x1b[0m \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    fn print_webapp_server_info(&self) {
        println!(
            "\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🌐 \x1b[1;36mWeb Application Server\x1b[0m                                 \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mServer Mode:\x1b[0m \x1b[1;32mWeb Application\x1b[0m                           \x1b[1;34m│\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mFramework:\x1b[0m \x1b[1;33mRust → WASM (wasm-bindgen)\x1b[0m                \x1b[1;34m│\x1b[0m");

        if self.watch_mode {
            println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32m✓ Hot reload on source changes\x1b[0m           \x1b[1;34m│\x1b[0m");
        } else {
            println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mWatch Mode:\x1b[0m \x1b[0;37mDisabled\x1b[0m                                 \x1b[1;34m│\x1b[0m");
        }

        println!("\x1b[1;34m│\x1b[0m  \x1b[1;34mFeatures:\x1b[0m \x1b[1;32mSPA routing, Asset serving, Dev tools\x1b[0m       \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    fn print_server_details(&self) {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  🌀 \x1b[1;36mChakra Server\x1b[0m                                     \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🚀 \x1b[1;34mServer URL:\x1b[0m \x1b[4;36m{:<47}\x1b[0m \x1b[1;34m│\x1b[0m", self.url);
        println!("\x1b[1;34m│\x1b[0m  🔌 \x1b[1;34mPort:\x1b[0m \x1b[1;33m{:<55}\x1b[0m \x1b[1;34m│\x1b[0m", self.port);
        println!("\x1b[1;34m│\x1b[0m  ℹ️ \x1b[1;34mProcess ID:\x1b[0m \x1b[1;33m{:<47}\x1b[0m \x1b[1;34m│\x1b[0m", self.server_pid);

        let status = if self.watch_mode {
            "\x1b[1;32m🔄 Active (watching for changes)\x1b[0m"
        } else {
            "\x1b[1;32m✓ Running\x1b[0m"
        };
        println!(
            "\x1b[1;34m│\x1b[0m  ⚫️ \x1b[1;34mStatus:\x1b[0m {:<47} \x1b[1;34m│\x1b[0m",
            status
        );

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    fn open_browser(&self) {
        println!("\n🌐 \x1b[1;36mOpening browser...\x1b[0m");

        if let Err(e) = webbrowser::open(&self.url) {
            println!(
                "❗ \x1b[1;33mFailed to open browser automatically: {}\x1b[0m",
                e
            );
            println!(
                "🔗 \x1b[1;34mManually open:\x1b[0m \x1b[4;36m{}\x1b[0m",
                self.url
            );
        } else {
            println!("✅ \x1b[1;32mBrowser opened successfully!\x1b[0m");
        }

        // Add some breathing room before server logs
        // println!("\n\x1b[1;34m" + "─".repeat(67).as_str() + "\x1b[0m");
        // println!("📝 \x1b[1;36mServer logs will appear below:\x1b[0m");
        // println!("\x1b[1;34m" + "─".repeat(67).as_str() + "\x1b[0m\n");
    }
}

/// Utility functions for server operations
pub struct ServerUtils;

impl ServerUtils {
    /// Get comprehensive file information for display
    #[allow(dead_code)]
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
        let file_size = format_file_size(file_size_bytes);

        Ok(FileInfo {
            filename,
            absolute_path,
            file_size,
            file_size_bytes,
        })
    }

    /// Check if a port is available and suggest alternatives if not
    pub fn check_port_availability(port: u16) -> PortStatus {
        if is_port_available(port) {
            PortStatus::Available
        } else {
            // Suggest alternative ports
            let alternatives = (port + 1..port + 10).find(|&p| is_port_available(p));

            PortStatus::Unavailable {
                alternative: alternatives,
            }
        }
    }

    /// Print a warning if the port is not available
    pub fn handle_port_conflict(port: u16) -> Result<u16> {
        match Self::check_port_availability(port) {
            PortStatus::Available => Ok(port),
            PortStatus::Unavailable { alternative } => {
                println!("\n⚠️  \x1b[1;33mPort {} is already in use\x1b[0m", port);

                if let Some(alt_port) = alternative {
                    println!("🔄 \x1b[1;34mTrying alternative port: {}\x1b[0m", alt_port);
                    Ok(alt_port)
                } else {
                    println!(
                        "❌ \x1b[1;31mNo alternative ports available in range {}-{}\x1b[0m",
                        port,
                        port + 10
                    );
                    Err(crate::error::ChakraError::Server(
                        crate::error::ServerError::startup_failed(
                            port,
                            format!("Port {} is in use and no alternatives found", port),
                        ),
                    ))
                }
            }
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct FileInfo {
    pub filename: String,
    pub absolute_path: String,
    pub file_size: String,
    pub file_size_bytes: u64,
}

#[derive(Debug)]
pub enum PortStatus {
    Available,
    Unavailable { alternative: Option<u16> },
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} bytes", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Legacy function for backward compatibility
pub fn print_server_info(
    url: &str,
    port: u16,
    wasm_filename: &str,
    file_size: &str,
    absolute_path: &str,
    watch_mode: bool,
) {
    // Server info and fall back to basic if needed
    if let Ok(server_info) = ServerInfo::for_wasm_file(absolute_path, port, watch_mode) {
        server_info.print_server_startup();
    } else {
        // Fallback to basic output if analysis fails
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

/// Basic server info printing (fallback)
fn print_basic_server_info(
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

/// Print an error message in a formatted box
#[allow(dead_code)]
pub fn print_error(message: String) {
    eprintln!("\n\x1b[1;34m╭\x1b[0m");
    eprintln!("  ❌ \x1b[1;31m{}\x1b[0m", message);
    eprintln!("\x1b[1;34m╰\x1b[0m");
}
