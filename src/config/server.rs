use std::path::Path;

use crate::compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel, TargetType};
use crate::error::{Result, ServerError, WasmrunError};
use crate::plugin::manager::PluginManager;
use crate::utils::PluginUtils;
use crate::utils::{ProjectAnalysis, WasmAnalysis};

use crate::server::utils::{find_wasm_files, is_port_available};
use crate::server::wasm;
use crate::server::{is_server_running, stop_existing_server, ServerUtils};

#[derive(Debug)]
#[allow(dead_code)] // TODO: Future server configuration system
pub struct ServerConfig {
    pub wasm_path: String,
    pub js_path: Option<String>,
    pub port: u16,
    pub watch_mode: bool,
    pub project_path: Option<String>,
    pub output_dir: Option<String>,
    pub serve: bool,
}

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

#[allow(dead_code)] // TODO: Use for advanced compilation setup
pub fn setup_project_compilation(
    project_path: &str,
    language_override: Option<String>,
    _watch: bool,
) -> Option<(crate::compiler::ProjectLanguage, String)> {
    ServerUtils::print_initial_project_detection(project_path);

    // project detection using plugin system
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
            println!("🔌 Using plugin: {}", plugin.info().name);

            // Check plugin dependencies
            let builder = plugin.get_builder();
            let missing_deps = builder.check_dependencies();
            if !missing_deps.is_empty() {
                println!(
                    "⚠️  Plugin dependencies missing: {}",
                    missing_deps.join(", ")
                );
                println!("💡 Consider installing: {}", missing_deps.join(", "));
                println!("🔄 Falling back to built-in language detection...");
            } else {
                // Map plugin to language type for compatibility with existing code
                let lang = PluginUtils::map_plugin_to_project_language(plugin, project_path);

                use crate::utils::PathResolver;

                // Clean up temp files from previous runs
                if let Err(e) = PathResolver::cleanup_temp_directory("wasmrun_temp") {
                    println!("⚠️  Warning: Failed to cleanup temporary directory: {e}");
                }

                let temp_output_dir = match PathResolver::create_temp_directory("wasmrun_temp") {
                    Ok(dir) => dir,
                    Err(e) => {
                        println!("❌ Failed to create temporary directory: {e}");
                        return None;
                    }
                };

                return Some((lang, temp_output_dir));
            }
        }
    }

    // Legacy language detection. Remove in future versions.
    let lang = if let Some(lang_override) = language_override {
        match lang_override.to_lowercase().as_str() {
            "rust" | "rs" => crate::compiler::ProjectLanguage::Rust,
            "c" | "cpp" | "c++" => crate::compiler::ProjectLanguage::C,
            "asc" | "assemblyscript" => crate::compiler::ProjectLanguage::Asc,
            "go" => crate::compiler::ProjectLanguage::Go,
            "python" | "py" => crate::compiler::ProjectLanguage::Python,
            _ => {
                println!("⚠️  Unknown language override: {lang_override}");
                crate::compiler::detect_project_language(project_path)
            }
        }
    } else {
        println!("🔍 Using built-in language detection...");
        crate::compiler::detect_project_language(project_path)
    };

    use crate::utils::PathResolver;

    // Clean up any stale files from previous runs
    if let Err(e) = PathResolver::cleanup_temp_directory("wasmrun_temp") {
        println!("⚠️  Warning: Failed to cleanup temporary directory: {e}");
    }

    let temp_output_dir = match PathResolver::create_temp_directory("wasmrun_temp") {
        Ok(dir) => dir,
        Err(e) => {
            println!("❌ Failed to create temporary directory: {e}");
            return None;
        }
    };

    Some((lang, temp_output_dir))
}

#[allow(dead_code)] // TODO: Use for standalone compilation API
pub fn compile_project(
    project_path: &str,
    output_dir: &str,
    lang: crate::compiler::ProjectLanguage,
    _watch: bool,
) -> Option<(String, bool, Option<String>)> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        optimization_level: OptimizationLevel::Release,
        verbose: false,
        watch: false,
        target_type: TargetType::Standard,
    };

    // First try plugin-based compilation
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
            let builder = plugin.get_builder();

            // Check dependencies first
            let missing_deps = builder.check_dependencies();
            if !missing_deps.is_empty() {
                println!(
                    "⚠️  Plugin dependencies missing: {}",
                    missing_deps.join(", ")
                );
                println!("💡 Install missing tools or use built-in compiler");
                println!("🔄 Falling back to built-in compilation...");
            } else {
                println!("🔌 Compiling with plugin: {}", plugin.info().name);
                match builder.build(&config) {
                    Ok(result) => {
                        println!("✅ Plugin compilation successful!");
                        println!("📦 WASM file: {}", result.wasm_path);
                        if let Some(ref js_path) = result.js_path {
                            println!("📦 JS file: {js_path}");
                        }
                        return Some((result.wasm_path, result.is_wasm_bindgen, result.js_path));
                    }
                    Err(e) => {
                        println!("❌ Plugin compilation failed: {e:?}");
                        println!("🔄 Falling back to built-in compilation...");
                    }
                }
            }
        }
    }

    // Fallback to built-in compilation system
    println!("🔧 Using built-in compiler for {lang:?}");
    let builder = BuilderFactory::create_builder(&lang);
    match builder.build(&config) {
        Ok(result) => {
            println!("✅ Built-in compilation successful!");
            println!("📦 WASM file: {}", result.wasm_path);
            if let Some(ref js_path) = result.js_path {
                println!("📦 JS file: {js_path}");
            }
            Some((result.wasm_path, result.is_wasm_bindgen, result.js_path))
        }
        Err(e) => {
            println!("❌ Built-in compilation failed: {e}");
            None
        }
    }
}

pub fn run_server(config: ServerConfig) -> Result<()> {
    if is_server_running() {
        match stop_existing_server() {
            Ok(_) => println!("💀 Existing server stopped successfully."),
            Err(e) => eprintln!("❗ Warning when stopping existing server: {e}"),
        }
    }

    if !is_port_available(config.port) {
        return Err(WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Port {} is already in use", config.port),
        }));
    }

    let path_obj = Path::new(&config.wasm_path);
    if !path_obj.exists() {
        return Err(WasmrunError::path(format!(
            "Path not found: {}",
            config.wasm_path
        )));
    }

    if path_obj.is_dir() {
        let wasm_files = find_wasm_files(path_obj);
        if wasm_files.is_empty() {
            return Err(WasmrunError::path(format!(
                "No WASM files found in directory: {}",
                config.wasm_path
            )));
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
            println!("  \x1b[1;37mwasmrun --wasm --path <filename.wasm>\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m");
            return Err(WasmrunError::path(
                "Please select a specific WASM file to run".to_string(),
            ));
        }
    }

    if !path_obj.is_file() {
        return Err(WasmrunError::path(format!(
            "Not a file: {}",
            config.wasm_path
        )));
    }
    if path_obj
        .extension()
        .map_or(true, |ext| ext.to_string_lossy().to_lowercase() != "wasm")
    {
        return Err(WasmrunError::path(format!(
            "Not a WASM file: {}",
            config.wasm_path
        )));
    }

    let wasm_filename = path_obj
        .file_name()
        .ok_or_else(|| WasmrunError::path("Invalid path".to_string()))?
        .to_string_lossy()
        .to_string();

    wasm::serve_wasm_file_with_project(
        &config.wasm_path,
        config.port,
        &wasm_filename,
        config.project_path.as_deref(),
        config.serve,
    )
    .map_err(|e| {
        WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Server startup failed: {e}"),
        })
    })
}

#[derive(Debug)]
#[allow(dead_code)] // TODO: Future file metadata system (duplicate of server/utils.rs)
pub struct FileInfo {
    pub filename: String,
    pub absolute_path: String,
    pub file_size: String,
    pub file_size_bytes: u64,
}

#[derive(Debug)]
pub enum PortStatus {
    Available,
    #[allow(dead_code)] // TODO: Future port status with alternatives
    Unavailable {
        alternative: Option<u16>,
    },
}
