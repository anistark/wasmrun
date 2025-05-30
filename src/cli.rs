use crate::utils::PathResolver;
use clap::{Parser, Subcommand};

/// Chakra - WebAssembly project compiler and runtime 🌟
#[derive(Parser, Debug)]
#[command(author, version = get_version_string(), about, long_about = None, after_help = "If you find Chakra useful, please consider starring the repository \
                       on GitHub to support this open source project! ✨\n\
                       https://github.com/anistark/chakra")]
pub struct Args {
    /// Subcommands to control Chakra server
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to project directory or WASM file (default: current directory)
    #[arg(short = 'p', long, default_value = "./")]
    pub path: String,

    #[arg(index = 1)]
    pub positional_path: Option<String>,

    /// Port to serve (default: 8420)
    #[arg(short = 'P', long, default_value_t = 8420)]
    pub port: u16,

    /// Interpret path as a WebAssembly file (instead of a project directory)
    #[arg(short = 'w', long)]
    pub wasm: bool,

    /// Enable watch mode for live-reloading on file changes
    #[arg(short = 'W', long)]
    pub watch: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Stop the running Chakra server
    Stop,

    /// Compile a project to WebAssembly
    Compile {
        /// Path to the project directory
        #[arg(short = 'p', long)]
        path: Option<String>,

        #[arg(index = 1)]
        positional_path: Option<String>,

        /// Output directory for the WASM file (default: current directory)
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// Enable verbose output
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Optimization level: debug, release, size
        #[arg(long, default_value = "release")]
        optimization: String,
    },

    /// Verify WebAssembly file
    Verify {
        /// Path to the WASM file
        #[arg(short = 'p', long)]
        path: Option<String>,

        /// WASM file path (positional argument)
        #[arg(index = 1)]
        positional_path: Option<String>,

        /// Show detailed information about the WASM module
        #[arg(short = 'd', long)]
        detailed: bool,
    },

    /// Inspect WebAssembly file
    Inspect {
        /// Path to the WASM file
        #[arg(short = 'p', long)]
        path: Option<String>,

        /// WASM file path (positional argument)
        #[arg(index = 1)]
        positional_path: Option<String>,
    },

    /// AOT Compile and run a project
    Run {
        /// Path to the project
        #[arg(short = 'p', long)]
        path: Option<String>,

        #[arg(index = 1)]
        positional_path: Option<String>,

        /// Port to serve (default: 8420)
        #[arg(short = 'P', long, default_value_t = 8420)]
        port: u16,

        /// Language to use for compilation (auto-detect if not specified)
        #[arg(short = 'l', long)]
        language: Option<String>,

        /// Enable watch mode for live-reloading on file changes
        #[arg(long)]
        watch: bool,

        /// Enable verbose output
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Initialize a new Chakra project
    Init {
        /// Project name
        #[arg(index = 1)]
        name: Option<String>,

        /// Template to use (rust, go, c, assemblyscript)
        #[arg(short = 't', long, default_value = "rust")]
        template: String,

        /// Target directory (default: project name)
        #[arg(short = 'd', long)]
        directory: Option<String>,
    },

    /// Clean build artifacts
    Clean {
        /// Path to the project directory
        #[arg(short = 'p', long)]
        path: Option<String>,

        #[arg(index = 1)]
        positional_path: Option<String>,
    },
}

/// Enhanced argument resolution with validation
#[derive(Debug)]
pub struct ResolvedArgs {
    pub path: String,
    pub port: u16,
    pub wasm: bool,
    pub watch: bool,
    pub command: Option<Commands>,
}

impl ResolvedArgs {
    /// Create from CLI args with path resolution and validation
    pub fn from_args(args: Args) -> Result<Self, String> {
        let resolved_path = PathResolver::resolve_input_path(args.positional_path, Some(args.path));

        Ok(Self {
            path: resolved_path,
            port: args.port,
            wasm: args.wasm,
            watch: args.watch,
            command: args.command,
        })
    }

    /// Validate the resolved arguments
    pub fn validate(&self) -> Result<(), String> {
        // Validate port range
        if self.port == 0 {
            return Err(format!(
                "Invalid port number: {}. Must be between 1-65535",
                self.port
            ));
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
                        return Err(format!("Path not found: {}", self.path));
                    }
                }
            }
        }

        Ok(())
    }
}

/// Command-specific argument resolution
#[allow(dead_code)]
pub trait CommandArgs {
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
            Commands::Init {
                name, directory, ..
            } => directory.clone().unwrap_or_else(|| {
                name.clone()
                    .unwrap_or_else(|| "my-chakra-project".to_string())
            }),
            Commands::Stop => "./".to_string(),
        }
    }
}

/// Validation helper for specific command arguments
pub struct CommandValidator;

impl CommandValidator {
    pub fn validate_compile_args(
        path: &Option<String>,
        positional_path: &Option<String>,
        output: &Option<String>,
    ) -> Result<(String, String), String> {
        let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
        let output_dir = output.clone().unwrap_or_else(|| ".".to_string());

        PathResolver::validate_directory_exists(&project_path)?;
        PathResolver::ensure_output_directory(&output_dir)?;

        Ok((project_path, output_dir))
    }

    pub fn validate_verify_args(
        path: &Option<String>,
        positional_path: &Option<String>,
    ) -> Result<String, String> {
        // Debug output
        eprintln!(
            "validate_verify_args - path: {:?}, positional: {:?}",
            path, positional_path
        );

        let wasm_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
        eprintln!("validate_verify_args - resolved path: {}", wasm_path);

        PathResolver::validate_wasm_file(&wasm_path)?;
        Ok(wasm_path)
    }

    pub fn validate_run_args(
        path: &Option<String>,
        positional_path: &Option<String>,
        port: u16,
    ) -> Result<(String, u16), String> {
        let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());

        // Validate port
        if port == 0 {
            return Err(format!(
                "Invalid port number: {}. Must be between 1-65535",
                port
            ));
        }

        // For run command, path can be either a project directory or a WASM file
        if !std::path::Path::new(&project_path).exists() {
            return Err(format!("Path not found: {}", project_path));
        }

        Ok((project_path, port))
    }

    pub fn validate_init_args(
        name: &Option<String>,
        template: &str,
        directory: &Option<String>,
    ) -> Result<(String, String, String), String> {
        let project_name = name
            .clone()
            .unwrap_or_else(|| "my-chakra-project".to_string());
        let target_dir = directory.clone().unwrap_or_else(|| project_name.clone());

        // Validate template
        let valid_templates = ["rust", "go", "c", "assemblyscript", "python"];
        if !valid_templates.contains(&template) {
            return Err(format!(
                "Invalid template '{}'. Valid templates: {}",
                template,
                valid_templates.join(", ")
            ));
        }

        // Check if target directory already exists
        if std::path::Path::new(&target_dir).exists() {
            return Err(format!("Directory '{}' already exists", target_dir));
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

    // Handle legacy behavior: if positional path is provided, use it as the main path
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
         \x1b[1;34m│\x1b[0m  🌀 \x1b[1;36m{} v{}\x1b[0m\n\
         \x1b[1;34m│\x1b[0m  \x1b[0;90mA lightweight WebAssembly runner\x1b[0m\n\
         \x1b[1;34m╰\x1b[0m\n",
        name, version
    );
}

/// Enhanced argument parsing with validation
#[allow(dead_code)]
pub fn get_validated_args() -> Result<ResolvedArgs, String> {
    let args = get_args();
    let resolved = ResolvedArgs::from_args(args)?;
    resolved.validate()?;
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolved_args_from_args() {
        let args = Args {
            command: None,
            path: "./test".to_string(),
            positional_path: Some("./positional".to_string()),
            port: 8420,
            wasm: false,
            watch: false,
        };

        let resolved = ResolvedArgs::from_args(args).unwrap();
        assert_eq!(resolved.path, "./positional"); // Positional takes precedence
        assert_eq!(resolved.port, 8420);
        assert!(!resolved.wasm);
        assert!(!resolved.watch);
    }

    #[test]
    fn test_command_args_resolve_path() {
        let compile_cmd = Commands::Compile {
            path: Some("./flag".to_string()),
            positional_path: Some("./positional".to_string()),
            output: None,
            verbose: false,
            optimization: "release".to_string(),
        };

        assert_eq!(compile_cmd.resolve_path(), "./positional");
    }

    #[test]
    fn test_validate_compile_args() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();
        let output_path = temp_dir.path().join("output");
        let output_str = output_path.to_str().unwrap();

        // Create a project file to make it a valid project directory
        fs::write(temp_dir.path().join("test.txt"), "test").unwrap();

        let result = CommandValidator::validate_compile_args(
            &Some(project_path.to_string()),
            &None,
            &Some(output_str.to_string()),
        );

        assert!(result.is_ok());
        let (proj_path, out_path) = result.unwrap();
        assert_eq!(proj_path, project_path);
        assert_eq!(out_path, output_str);
        assert!(output_path.exists()); // Should have been created
    }

    #[test]
    fn test_validate_verify_args_valid_wasm() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("test.wasm");

        // Create a fake WASM file
        fs::write(&wasm_file, b"fake wasm content").unwrap();

        let result = CommandValidator::validate_verify_args(
            &Some(wasm_file.to_str().unwrap().to_string()),
            &None,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_verify_args_invalid_extension() {
        let temp_dir = tempdir().unwrap();
        let js_file = temp_dir.path().join("test.js");

        fs::write(&js_file, b"console.log('test')").unwrap();

        let result = CommandValidator::validate_verify_args(
            &Some(js_file.to_str().unwrap().to_string()),
            &None,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not a WASM file"));
    }

    #[test]
    fn test_validate_init_args() {
        let result =
            CommandValidator::validate_init_args(&Some("my-project".to_string()), "rust", &None);

        assert!(result.is_ok());
        let (name, template, dir) = result.unwrap();
        assert_eq!(name, "my-project");
        assert_eq!(template, "rust");
        assert_eq!(dir, "my-project");
    }

    #[test]
    fn test_validate_init_args_invalid_template() {
        let result = CommandValidator::validate_init_args(
            &Some("my-project".to_string()),
            "invalid-template",
            &None,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid template"));
    }

    #[test]
    fn test_port_validation() {
        let args = Args {
            command: None,
            path: "./".to_string(),
            positional_path: None,
            port: 0, // Invalid port
            wasm: false,
            watch: false,
        };

        let resolved = ResolvedArgs::from_args(args).unwrap();
        let validation_result = resolved.validate();

        assert!(validation_result.is_err());
        assert!(validation_result
            .unwrap_err()
            .contains("Invalid port number"));
    }
}
