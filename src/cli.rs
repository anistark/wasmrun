use crate::error::{Result, WasmrunError};
use crate::utils::PathResolver;
use clap::{Parser, Subcommand};

/// Wasmrun - WebAssembly project compiler and runtime 🌟
#[derive(Parser, Debug)]
#[command(
    name = "wasmrun",
    author,
    version = get_version_string(),
    about = "A lightweight WebAssembly runner",
    long_about = "Wasmrun is a CLI tool for compiling, running, and debugging WebAssembly modules with full WASI support.",
    after_help = "If you find Wasmrun useful, please consider starring the repository on GitHub! ✨\nhttps://github.com/anistark/wasmrun"
)]
pub struct Args {
    /// Subcommands to control Wasmrun server
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to project directory or WASM file (default: current directory)
    #[arg(
        short = 'p',
        long,
        default_value = "./",
        value_hint = clap::ValueHint::AnyPath,
        help = "Project directory or WASM file path"
    )]
    pub path: String,

    /// Project directory or WASM file path (positional argument)
    #[arg(index = 1, value_hint = clap::ValueHint::AnyPath)]
    pub positional_path: Option<String>,

    /// Port to serve (default: 8420)
    // TODO: Apply to web server as well if provided.
    #[arg(
        short = 'P',
        long,
        default_value_t = 8420,
        value_parser = clap::value_parser!(u16).range(1..=65535),
        help = "Server port number"
    )]
    pub port: u16,

    /// Interpret path as a WebAssembly file (instead of a project directory)
    #[arg(short = 'w', long, help = "Run WASM file directly")]
    pub wasm: bool,

    /// Enable watch mode for live-reloading on file changes
    #[arg(short = 'W', long, help = "Watch for file changes and reload")]
    pub watch: bool,

    /// Enable debug output with detailed information
    #[arg(long, global = true, help = "Show detailed debug information")]
    pub debug: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Stop any running Wasmrun server instance
    #[command(alias = "kill")]
    Stop,

    /// Compile a project to WebAssembly with optimization options
    #[command(aliases = ["build", "c"])]
    Compile {
        /// Path to the project directory
        #[arg(
            short = 'p',
            long,
            value_hint = clap::ValueHint::DirPath,
            help = "Project directory to compile"
        )]
        path: Option<String>,

        /// Project directory path (positional argument)
        #[arg(index = 1, value_hint = clap::ValueHint::DirPath)]
        positional_path: Option<String>,

        /// Output directory for the WASM file (default: current directory)
        #[arg(
            short = 'o',
            long,
            value_hint = clap::ValueHint::DirPath,
            help = "Output directory for compiled files"
        )]
        output: Option<String>,

        /// Enable verbose output
        #[arg(short = 'v', long, help = "Show detailed compilation output")]
        verbose: bool,

        /// Optimization level: debug, release, size
        #[arg(
            long,
            default_value = "release",
            value_parser = ["debug", "release", "size"],
            help = "Compilation optimization level"
        )]
        optimization: String,
    },

    /// Verify WebAssembly file format and structure
    Verify {
        /// Path to the WASM file
        #[arg(
            short = 'p',
            long,
            value_hint = clap::ValueHint::FilePath,
            help = "WASM file to verify"
        )]
        path: Option<String>,

        /// WASM file path (positional argument)
        #[arg(index = 1, value_hint = clap::ValueHint::FilePath)]
        positional_path: Option<String>,

        /// Show detailed information about the WASM module
        #[arg(short = 'd', long, help = "Show detailed verification results")]
        detailed: bool,
    },

    /// Perform detailed inspection on a WebAssembly file
    Inspect {
        /// Path to the WASM file
        #[arg(
            short = 'p',
            long,
            value_hint = clap::ValueHint::FilePath,
            help = "WASM file to inspect"
        )]
        path: Option<String>,

        /// WASM file path (positional argument)
        #[arg(index = 1, value_hint = clap::ValueHint::FilePath)]
        positional_path: Option<String>,
    },

    /// Compile and run a project with live development server
    #[command(aliases = ["dev", "serve"])]
    Run {
        /// Path to the project
        #[arg(
            short = 'p',
            long,
            value_hint = clap::ValueHint::DirPath,
            help = "Project directory to run"
        )]
        path: Option<String>,

        /// Project path (positional argument)
        #[arg(index = 1, value_hint = clap::ValueHint::DirPath)]
        positional_path: Option<String>,

        /// Port to serve (default: 8420)
        #[arg(
            short = 'P',
            long,
            default_value_t = 8420,
            value_parser = clap::value_parser!(u16).range(1..=65535),
            help = "Development server port"
        )]
        port: u16,

        /// Language to use for compilation (auto-detect if not specified)
        #[arg(
            short = 'l',
            long,
            value_parser = ["rust", "go", "c", "asc", "python"],
            help = "Force specific language for compilation"
        )]
        language: Option<String>,

        /// Enable watch mode for live-reloading on file changes
        #[arg(long, help = "Watch for changes and auto-reload")]
        watch: bool,

        /// Enable verbose output
        #[arg(short = 'v', long, help = "Show detailed build output")]
        verbose: bool,
    },

    /// Plugin management commands
    #[command(subcommand)]
    Plugin(PluginSubcommands),

    // TODO: Implement project initialization command
    // This will create new WebAssembly projects from templates (rust, go, c, asc, python)
    // /// Initialize a new Wasmrun project from template
    // #[command(alias = "new")]
    // Init {
    //     /// Project name
    //     #[arg(index = 1, help = "Name of the new project")]
    //     name: Option<String>,

    //     /// Template to use (rust, go, c, asc)
    //     #[arg(
    //         short = 't',
    //         long,
    //         default_value = "rust",
    //         value_parser = ["rust", "go", "c", "asc", "python"],
    //         help = "Project template to use"
    //     )]
    //     template: String,

    //     /// Target directory (default: project name)
    //     #[arg(
    //         short = 'd',
    //         long,
    //         value_hint = clap::ValueHint::DirPath,
    //         help = "Directory to create project in"
    //     )]
    //     directory: Option<String>,
    // },
    /// Clean build artifacts and temporary files
    #[command(aliases = ["clear", "reset"])]
    Clean {
        /// Path to the project directory
        #[arg(
            short = 'p',
            long,
            value_hint = clap::ValueHint::DirPath,
            help = "Project directory to clean"
        )]
        path: Option<String>,

        /// Project directory path (positional argument)
        #[arg(index = 1, value_hint = clap::ValueHint::DirPath)]
        positional_path: Option<String>,
    },
}

/// Plugin management subcommands
#[derive(Subcommand, Debug)]
pub enum PluginSubcommands {
    /// List all available plugins
    List {
        /// Show detailed information
        #[arg(short, long)]
        all: bool,
    },

    /// Install a plugin
    Install {
        /// Plugin name, URL, or path
        plugin: String,

        /// Specific version to install (for crates.io plugins)
        #[arg(short, long)]
        version: Option<String>,
    },

    /// Uninstall a plugin
    Uninstall {
        /// Plugin name to uninstall
        plugin: String,
    },

    /// Update a plugin
    Update {
        /// Plugin name to update, or 'all' for all plugins
        plugin: String,
    },

    /// Enable or disable a plugin
    Enable {
        /// Plugin name
        plugin: String,

        /// Disable instead of enable
        #[arg(long)]
        disable: bool,
    },

    /// Show detailed information about a plugin
    Info {
        /// Plugin name
        plugin: String,
    },
    // TODO: Implement plugin search with proper plugin registry system
    // /// Search for available plugins
    // Search {
    //     /// Search query
    //     query: String,
    // },
}

/// Argument resolution with validation
#[derive(Debug)]
pub struct ResolvedArgs {
    pub path: String,
    pub port: u16,
    pub wasm: bool,
    pub watch: bool,
    #[allow(dead_code)] // TODO: Used for debug output control
    pub debug: bool,
    #[allow(dead_code)]
    pub command: Option<Commands>,
}

impl ResolvedArgs {
    /// Create from CLI args with path resolution and validation
    pub fn from_args(args: Args) -> Result<Self> {
        let resolved_path = PathResolver::resolve_input_path(args.positional_path, Some(args.path));

        Ok(Self {
            path: resolved_path,
            port: args.port,
            wasm: args.wasm,
            watch: args.watch,
            debug: args.debug,
            command: args.command,
        })
    }

    /// Validate the resolved arguments
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<()> {
        // Validate port range
        if self.port == 0 {
            return Err(WasmrunError::from(format!(
                "Invalid port number: {}. Must be between 1-65535",
                self.port
            )));
        }

        // Validate path based on context
        match &self.command {
            Some(Commands::Verify { .. }) | Some(Commands::Inspect { .. }) => {
                // These commands expect WASM files
                PathResolver::validate_wasm_file(&self.path)?;
            }
            Some(Commands::Compile { .. })
            | Some(Commands::Run { .. })
            | Some(Commands::Clean { .. }) => {
                // These commands expect project directories
                PathResolver::validate_directory_exists(&self.path)?;
            }
            _ => {
                // For default run command, validate based on wasm flag
                if self.wasm {
                    PathResolver::validate_wasm_file(&self.path)?;
                } else {
                    // Could be either file or directory
                    if !std::path::Path::new(&self.path).exists() {
                        return Err(WasmrunError::path(format!("Path not found: {}", self.path)));
                    }
                }
            }
        }

        Ok(())
    }
}

/// Command-specific argument resolution
pub trait CommandArgs {
    #[allow(dead_code)]
    fn resolve_path(&self) -> String;
}

impl CommandArgs for Commands {
    fn resolve_path(&self) -> String {
        match self {
            Commands::Compile {
                path,
                positional_path,
                ..
            } => PathResolver::resolve_input_path(positional_path.clone(), path.clone()),
            Commands::Verify {
                path,
                positional_path,
                ..
            } => PathResolver::resolve_input_path(positional_path.clone(), path.clone()),
            Commands::Inspect {
                path,
                positional_path,
                ..
            } => PathResolver::resolve_input_path(positional_path.clone(), path.clone()),
            Commands::Run {
                path,
                positional_path,
                ..
            } => PathResolver::resolve_input_path(positional_path.clone(), path.clone()),
            Commands::Clean {
                path,
                positional_path,
                ..
            } => PathResolver::resolve_input_path(positional_path.clone(), path.clone()),
            // TODO: Implement Init command
            // Commands::Init {
            //     name, directory, ..
            // } => directory.clone().unwrap_or_else(|| {
            //     name.clone()
            //         .unwrap_or_else(|| "my-wasmrun-project".to_string())
            // }),
            Commands::Plugin(_) => "./".to_string(),
            Commands::Stop => "./".to_string(),
        }
    }
}

/// Validation helper for specific command arguments
pub struct CommandValidator;

impl CommandValidator {
    #[allow(dead_code)]
    pub fn validate_compile_args(
        path: &Option<String>,
        positional_path: &Option<String>,
        output: &Option<String>,
    ) -> Result<(String, String)> {
        let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
        let output_dir = output.clone().unwrap_or_else(|| ".".to_string());

        PathResolver::validate_directory_exists(&project_path)?;
        PathResolver::ensure_output_directory(&output_dir)?;

        Ok((project_path, output_dir))
    }

    pub fn validate_verify_args(
        path: &Option<String>,
        positional_path: &Option<String>,
    ) -> Result<String> {
        let wasm_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
        PathResolver::validate_wasm_file(&wasm_path)?;
        Ok(wasm_path)
    }

    #[allow(dead_code)]
    pub fn validate_run_args(
        path: &Option<String>,
        positional_path: &Option<String>,
        port: u16,
    ) -> Result<(String, u16)> {
        let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());

        if !std::path::Path::new(&project_path).exists() {
            return Err(WasmrunError::path(format!(
                "Path not found: {project_path}"
            )));
        }

        Ok((project_path, port))
    }

    #[allow(dead_code)]
    pub fn validate_init_args(
        name: &Option<String>,
        template: &str,
        directory: &Option<String>,
    ) -> Result<(String, String, String)> {
        let project_name = name
            .clone()
            .unwrap_or_else(|| "my-wasmrun-project".to_string());
        let target_dir = directory.clone().unwrap_or_else(|| project_name.clone());

        let valid_templates = ["rust", "go", "c", "asc", "python"];
        if !valid_templates.contains(&template) {
            return Err(WasmrunError::from(format!(
                "Invalid template '{}'. Valid templates: {}",
                template,
                valid_templates.join(", ")
            )));
        }

        if std::path::Path::new(&target_dir).exists() {
            return Err(WasmrunError::path(format!(
                "Directory '{target_dir}' already exists"
            )));
        }

        Ok((project_name, template.to_string(), target_dir))
    }
}

/// Get version string
fn get_version_string() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn get_args() -> Args {
    if std::env::args().any(|arg| arg == "-V" || arg == "--version") {
        print_styled_version();
        std::process::exit(0);
    }

    let mut args = Args::parse();

    if let Some(pos_path) = args.positional_path.take() {
        args.path = pos_path;
    }

    args
}

/// Print styled version output
fn print_styled_version() {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    println!(
        "\n\x1b[1;34m╭\x1b[0m\n\
         \x1b[1;34m│\x1b[0m  🅦 \x1b[1;36m{name} v{version}\x1b[0m\n\
         \x1b[1;34m│\x1b[0m  \x1b[0;90mA lightweight WebAssembly runner\x1b[0m\n\
         \x1b[1;34m╰\x1b[0m\n"
    );
}

/// Argument parsing with validation
#[allow(dead_code)]
pub fn get_validated_args() -> Result<ResolvedArgs> {
    let args = get_args();
    let resolved = ResolvedArgs::from_args(args)?;
    resolved.validate()?;
    Ok(resolved)
}

// Helper function for error conversion
impl From<String> for WasmrunError {
    fn from(message: String) -> Self {
        Self::Command(crate::error::CommandError::invalid_arguments(message))
    }
}
