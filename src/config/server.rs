use crate::error::Result;
use crate::utils::{ProjectAnalysis, WasmAnalysis};

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
    #[allow(dead_code)] // TODO: Future project-based content serving
    Project(ProjectAnalysis),
}

impl ServerInfo {
    pub fn for_wasm_file(wasm_path: &str, port: u16, watch_mode: bool) -> Result<Self> {
        let analysis = WasmAnalysis::analyze(wasm_path)?;

        Ok(Self {
            url: format!("http://localhost:{port}"),
            port,
            server_pid: std::process::id(),
            watch_mode,
            content_type: ContentType::WasmFile(analysis),
        })
    }

    #[allow(dead_code)] // TODO: Future project-based content serving
    pub fn for_project(project_path: &str, port: u16, watch_mode: bool) -> Result<Self> {
        let analysis = ProjectAnalysis::analyze(project_path)?;
        let content_type = ContentType::Project(analysis);

        Ok(Self {
            url: format!("http://localhost:{port}"),
            port,
            server_pid: std::process::id(),
            watch_mode,
            content_type,
        })
    }

    /// Print comprehensive server startup details
    pub fn print_server_startup(&self) {
        print!("\x1b[2J\x1b[H");
        self.print_header();

        match &self.content_type {
            ContentType::WasmFile(analysis) => {
                analysis.print_analysis();
                self.print_wasm_server_info();
            }
            ContentType::Project(analysis) => {
                analysis.print_analysis();
                self.print_project_server_info();
            }
        }

        // Print server details
        self.print_server_details();
    }

    fn print_header(&self) {
        println!("\n\x1b[1;32m");
        println!("   ██╗    ██╗ █████╗ ███████╗███╗   ███╗██████╗ ██╗   ██╗███╗   ██╗");
        println!("   ██║    ██║██╔══██╗██╔════╝████╗ ████║██╔══██╗██║   ██║████╗  ██║");
        println!("   ██║ █╗ ██║███████║███████╗██╔████╔██║██████╔╝██║   ██║██╔██╗ ██║");
        println!("   ██║███╗██║██╔══██║╚════██║██║╚██╔╝██║██╔══██╗██║   ██║██║╚██╗██║");
        println!("   ╚███╔███╔╝██║  ██║███████║██║ ╚═╝ ██║██║  ██║╚██████╔╝██║ ╚████║");
        println!("    ╚══╝╚══╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═══╝");
        println!("\x1b[0m");
        println!("   \x1b[1;34m🌟 WebAssembly Development Server\x1b[0m");

        let content_description = match &self.content_type {
            ContentType::WasmFile(analysis) => analysis.get_summary(),
            ContentType::Project(analysis) => analysis.get_summary(),
        };

        println!("   \x1b[0;37m{content_description}\x1b[0m\n");
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

    fn print_server_details(&self) {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  🅦 \x1b[1;36mWasmrun Server\x1b[0m                                     \x1b[1;34m│\x1b[0m");
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
        println!("\x1b[1;34m│\x1b[0m  ⚫️ \x1b[1;34mStatus:\x1b[0m {status:<47} \x1b[1;34m│\x1b[0m");

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }
}

#[derive(Debug)]
#[allow(dead_code)] // TODO: Future file metadata system (duplicate of server/utils.rs)
pub struct FileInfo {
    pub filename: String,
    pub absolute_path: String,
    pub file_size: String,
    pub file_size_bytes: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    pub allowed_destinations: Vec<String>,
    pub allowed_ports: Vec<u16>,
    pub bindable_port_range: (u16, u16),
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allowed_destinations: vec!["*".to_string()],
            allowed_ports: vec![],
            bindable_port_range: (10000, 65535),
        }
    }
}

#[allow(dead_code)]
impl NetworkPolicy {
    pub fn is_destination_allowed(&self, host: &str) -> bool {
        if self.allowed_destinations.contains(&"*".to_string()) {
            return true;
        }
        self.allowed_destinations.iter().any(|dest| {
            if let Some(stripped) = dest.strip_prefix('*') {
                host.ends_with(stripped)
            } else {
                dest == host
            }
        })
    }

    pub fn is_port_allowed(&self, port: u16) -> bool {
        if self.allowed_ports.is_empty() {
            return true;
        }
        self.allowed_ports.contains(&port)
    }

    pub fn is_bindable_port(&self, port: u16) -> bool {
        port >= self.bindable_port_range.0 && port <= self.bindable_port_range.1
    }
}
