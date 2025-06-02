use crate::error::{CompilationError, CompilationResult};
use crate::utils::PathResolver;
use std::path::Path;

/// Configuration for building WASM modules
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub project_path: String,
    pub output_dir: String,
    pub verbose: bool,
    pub optimization_level: OptimizationLevel,
    #[allow(dead_code)]
    pub target_type: TargetType,
}

#[derive(Debug, Clone)]
pub enum OptimizationLevel {
    Debug,
    Release,
    Size,
}

#[derive(Debug, Clone)]
pub enum TargetType {
    Standard, // Regular WASM
    #[allow(dead_code)]
    WasmBindgen, // WASM with JS bindings
    WebApp,   // Full web application
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            project_path: "./".to_string(),
            output_dir: "./".to_string(),
            verbose: false,
            optimization_level: OptimizationLevel::Release,
            target_type: TargetType::Standard,
        }
    }
}

/// Result of a build operation
#[derive(Debug)]
pub struct BuildResult {
    pub wasm_path: String,
    pub js_path: Option<String>,       // For wasm-bindgen projects
    pub additional_files: Vec<String>, // .d.ts, .map files, etc.
    #[allow(dead_code)]
    pub is_wasm_bindgen: bool,
}

/// Common interface for all WASM builders
pub trait WasmBuilder: Send + Sync {
    /// Get the human-readable name of this language
    fn language_name(&self) -> &str;

    /// Get common entry file names for this language
    fn entry_file_candidates(&self) -> &[&str];

    /// Get file extensions that this builder can handle
    #[allow(dead_code)]
    fn supported_extensions(&self) -> &[&str];

    /// Check if all required tools are installed
    fn check_dependencies(&self) -> Vec<String>;

    /// Build the project with the given configuration
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;

    /// Default verbose build implementation
    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!(
            "🔨 Building {} project at: {}",
            self.language_name(),
            config.project_path
        );

        // Check dependencies first
        let missing_tools = self.check_dependencies();
        if !missing_tools.is_empty() {
            return Err(CompilationError::BuildToolNotFound {
                tool: missing_tools.join(", "),
                language: self.language_name().to_string(),
            });
        }

        // Validate project structure
        self.validate_project(&config.project_path)?;

        // Ensure output directory exists
        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        println!("✅ All dependencies found");
        println!("📂 Output directory: {}", config.output_dir);

        let result = self.build(config)?;

        println!("✅ {} build completed successfully", self.language_name());
        println!("📦 WASM file: {}", result.wasm_path);

        if let Some(js_path) = &result.js_path {
            println!("📝 JS file: {}", js_path);
        }

        if !result.additional_files.is_empty() {
            println!(
                "📄 Additional files: {}",
                result.additional_files.join(", ")
            );
        }

        Ok(result)
    }

    /// Validate that the project structure is correct for this language
    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        PathResolver::validate_directory_exists(project_path).map_err(|_| {
            CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("Project directory not found: {}", project_path),
            }
        })?;

        // Check for entry files
        let entry_file = PathResolver::find_entry_file(project_path, self.entry_file_candidates());
        if entry_file.is_none() {
            return Err(CompilationError::MissingEntryFile {
                language: self.language_name().to_string(),
                candidates: self
                    .entry_file_candidates()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            });
        }

        Ok(())
    }

    /// Check if a tool is installed on the system
    fn is_tool_installed(&self, tool_name: &str) -> bool {
        let command = if cfg!(target_os = "windows") {
            format!("where {}", tool_name)
        } else {
            format!("which {}", tool_name)
        };

        std::process::Command::new(if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        })
        .args(if cfg!(target_os = "windows") {
            ["/c", &command]
        } else {
            ["-c", &command]
        })
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    }

    /// Execute a command and return the result
    fn execute_command(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &str,
        verbose: bool,
    ) -> CompilationResult<std::process::Output> {
        if verbose {
            println!("🔧 Executing: {} {}", command, args.join(" "));
        }

        std::process::Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .output()
            .map_err(|e| CompilationError::ToolExecutionFailed {
                tool: command.to_string(),
                reason: e.to_string(),
            })
    }

    /// Execute a command with live output (for verbose builds)
    fn execute_command_with_output(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &str,
    ) -> CompilationResult<()> {
        println!("🔧 Executing: {} {}", command, args.join(" "));

        let status = std::process::Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| CompilationError::ToolExecutionFailed {
                tool: command.to_string(),
                reason: e.to_string(),
            })?;

        if !status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "Command '{}' failed with exit code: {:?}",
                    command,
                    status.code()
                ),
            });
        }

        Ok(())
    }

    /// Copy output file to the target directory
    fn copy_to_output(&self, source: &str, output_dir: &str) -> CompilationResult<String> {
        let source_path = Path::new(source);
        let filename =
            PathResolver::get_filename(source).map_err(|_| CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Invalid source file path: {}", source),
            })?;
        let output_path = PathResolver::join_paths(output_dir, &filename);

        std::fs::copy(source_path, &output_path).map_err(|e| CompilationError::BuildFailed {
            language: self.language_name().to_string(),
            reason: format!("Failed to copy {} to {}: {}", source, output_path, e),
        })?;

        Ok(output_path)
    }

    /// Check if a specific target is available (allow dead code for future use)
    #[allow(dead_code)]
    fn check_target_availability(&self, _target: &str) -> CompilationResult<()> {
        // Default implementation - can be overridden by specific builders
        Ok(())
    }

    /// Get installation instructions for missing tools (allow dead code for future use)
    #[allow(dead_code)]
    fn get_install_instructions(&self, tool: &str) -> String {
        match tool {
            "rustc" | "cargo" => "Install Rust from https://rustup.rs/".to_string(),
            "tinygo" => {
                "Install TinyGo from https://tinygo.org/getting-started/install/".to_string()
            }
            "emcc" => {
                "Install Emscripten from https://emscripten.org/docs/getting_started/downloads.html"
                    .to_string()
            }
            "node" => "Install Node.js from https://nodejs.org/".to_string(),
            "npm" => "Install npm (usually comes with Node.js)".to_string(),
            "wasm-pack" => "Install with: cargo install wasm-pack".to_string(),
            "trunk" => "Install with: cargo install trunk".to_string(),
            _ => format!("Please install {} and ensure it's in your PATH", tool),
        }
    }

    /// Validate build configuration
    fn validate_config(&self, config: &BuildConfig) -> CompilationResult<()> {
        // Validate project path
        if config.project_path.is_empty() {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: "Project path cannot be empty".to_string(),
            });
        }

        // Validate output directory
        if config.output_dir.is_empty() {
            return Err(CompilationError::OutputDirectoryCreationFailed {
                path: "Output directory cannot be empty".to_string(),
            });
        }

        // Check if paths are safe
        if !PathResolver::is_safe_path(&config.project_path) {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("Unsafe project path: {}", config.project_path),
            });
        }

        if !PathResolver::is_safe_path(&config.output_dir) {
            return Err(CompilationError::OutputDirectoryCreationFailed {
                path: format!("Unsafe output path: {}", config.output_dir),
            });
        }

        Ok(())
    }
}

/// Factory for creating builders
pub struct BuilderFactory;

impl BuilderFactory {
    pub fn create_builder(language: &crate::compiler::ProjectLanguage) -> Box<dyn WasmBuilder> {
        use crate::compiler::ProjectLanguage;

        match language {
            ProjectLanguage::Rust => Box::new(crate::compiler::language::rust::RustBuilder::new()),
            ProjectLanguage::Go => Box::new(crate::compiler::language::go::GoBuilder::new()),
            ProjectLanguage::C => Box::new(crate::compiler::language::c::CBuilder::new()),
            ProjectLanguage::AssemblyScript => {
                Box::new(crate::compiler::language::asc::AssemblyScriptBuilder::new())
            }
            ProjectLanguage::Python => {
                Box::new(crate::compiler::language::python::PythonBuilder::new())
            }
            ProjectLanguage::Unknown => {
                // Return a dummy builder that always fails
                Box::new(UnknownBuilder)
            }
        }
    }

    /// Get supported languages (allow dead code for future use)
    #[allow(dead_code)]
    pub fn supported_languages() -> Vec<&'static str> {
        vec!["Rust", "Go", "C", "AssemblyScript", "Python"]
    }

    /// Check if a language is supported (allow dead code for future use)
    #[allow(dead_code)]
    pub fn is_language_supported(language: &str) -> bool {
        Self::supported_languages().contains(&language)
    }
}

/// Dummy builder for unknown languages
struct UnknownBuilder;

impl WasmBuilder for UnknownBuilder {
    fn language_name(&self) -> &str {
        "Unknown"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &[]
    }

    fn supported_extensions(&self) -> &[&str] {
        &[]
    }

    fn check_dependencies(&self) -> Vec<String> {
        vec!["Language not detected or supported".to_string()]
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(CompilationError::UnsupportedLanguage {
            language: "Unknown".to_string(),
        })
    }
}

/// Unified build function that replaces the individual build functions
pub fn build_wasm_project(
    project_path: &str,
    output_dir: &str,
    language: &crate::compiler::ProjectLanguage,
    verbose: bool,
) -> CompilationResult<BuildResult> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        verbose,
        optimization_level: OptimizationLevel::Release,
        target_type: TargetType::Standard,
    };

    let builder = BuilderFactory::create_builder(language);

    // Validate configuration
    builder.validate_config(&config)?;

    if verbose {
        builder.build_verbose(&config)
    } else {
        builder.build(&config)
    }
}

/// Helper function to convert build errors to user-friendly messages (allow dead code for future use)
#[allow(dead_code)]
pub fn format_build_error(error: &CompilationError) -> String {
    match error {
        CompilationError::UnsupportedLanguage { language } => {
            format!(
                "Language '{}' is not supported.\nSupported languages: {}",
                language,
                BuilderFactory::supported_languages().join(", ")
            )
        }
        CompilationError::BuildToolNotFound { tool, language } => {
            let builder = BuilderFactory::create_builder(&match language.as_str() {
                "Rust" => crate::compiler::ProjectLanguage::Rust,
                "Go" => crate::compiler::ProjectLanguage::Go,
                "C" => crate::compiler::ProjectLanguage::C,
                "AssemblyScript" => crate::compiler::ProjectLanguage::AssemblyScript,
                "Python" => crate::compiler::ProjectLanguage::Python,
                _ => crate::compiler::ProjectLanguage::Unknown,
            });

            format!(
                "Build tool '{}' not found for {} projects.\n💡 {}",
                tool,
                language,
                builder.get_install_instructions(tool)
            )
        }
        CompilationError::MissingEntryFile {
            language,
            candidates,
        } => {
            format!(
                "No entry file found for {} project.\nExpected one of: {}\n💡 Create one of these files to get started",
                language,
                candidates.join(", ")
            )
        }
        _ => error.to_string(),
    }
}
