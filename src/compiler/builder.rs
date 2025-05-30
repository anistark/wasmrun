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
pub trait WasmBuilder {
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
    fn build(&self, config: &BuildConfig) -> Result<BuildResult, String>;

    /// Default verbose build implementation
    fn build_verbose(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        println!(
            "🔨 Building {} project at: {}",
            self.language_name(),
            config.project_path
        );

        // Check dependencies first
        let missing_tools = self.check_dependencies();
        if !missing_tools.is_empty() {
            return Err(format!(
                "Missing required tools for {}: {}",
                self.language_name(),
                missing_tools.join(", ")
            ));
        }

        // Validate project structure
        self.validate_project(&config.project_path)?;

        // Ensure output directory exists
        PathResolver::ensure_output_directory(&config.output_dir)?;

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
    fn validate_project(&self, project_path: &str) -> Result<(), String> {
        PathResolver::validate_directory_exists(project_path)?;

        // Check for entry files
        let entry_file = PathResolver::find_entry_file(project_path, self.entry_file_candidates());
        if entry_file.is_none() {
            return Err(format!(
                "No entry file found for {} project. Expected one of: {}",
                self.language_name(),
                self.entry_file_candidates().join(", ")
            ));
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
    ) -> Result<std::process::Output, String> {
        if verbose {
            println!("🔧 Executing: {} {}", command, args.join(" "));
        }

        std::process::Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .output()
            .map_err(|e| format!("Failed to execute command '{}': {}", command, e))
    }

    /// Execute a command with live output (for verbose builds)
    fn execute_command_with_output(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &str,
    ) -> Result<(), String> {
        println!("🔧 Executing: {} {}", command, args.join(" "));

        let status = std::process::Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to execute command '{}': {}", command, e))?;

        if !status.success() {
            return Err(format!(
                "Command '{}' failed with exit code: {:?}",
                command,
                status.code()
            ));
        }

        Ok(())
    }

    /// Copy output file to the target directory
    fn copy_to_output(&self, source: &str, output_dir: &str) -> Result<String, String> {
        let source_path = Path::new(source);
        let filename = PathResolver::get_filename(source)?;
        let output_path = PathResolver::join_paths(output_dir, &filename);

        std::fs::copy(source_path, &output_path)
            .map_err(|e| format!("Failed to copy {} to {}: {}", source, output_path, e))?;

        Ok(output_path)
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
            ProjectLanguage::Unknown => panic!("Cannot create builder for unknown language"),
        }
    }
}

/// Unified build function that replaces the individual build functions
pub fn build_wasm_project(
    project_path: &str,
    output_dir: &str,
    language: &crate::compiler::ProjectLanguage,
    verbose: bool,
) -> Result<BuildResult, String> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        verbose,
        optimization_level: OptimizationLevel::Release,
        target_type: TargetType::Standard,
    };

    let builder = BuilderFactory::create_builder(language);

    if verbose {
        builder.build_verbose(&config)
    } else {
        builder.build(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    struct MockBuilder;

    impl WasmBuilder for MockBuilder {
        fn language_name(&self) -> &str {
            "mock"
        }

        fn entry_file_candidates(&self) -> &[&str] {
            &["main.mock", "app.mock"]
        }

        fn supported_extensions(&self) -> &[&str] {
            &["mock"]
        }

        fn check_dependencies(&self) -> Vec<String> {
            vec![] // No missing dependencies
        }

        fn build(&self, config: &BuildConfig) -> Result<BuildResult, String> {
            // Create a fake output file
            let output_path = PathResolver::join_paths(&config.output_dir, "test.wasm");
            fs::write(&output_path, b"fake wasm").unwrap();

            Ok(BuildResult {
                wasm_path: output_path,
                js_path: None,
                additional_files: vec![],
                is_wasm_bindgen: false,
            })
        }
    }

    #[test]
    fn test_build_config_default() {
        let config = BuildConfig::default();
        assert_eq!(config.project_path, "./");
        assert_eq!(config.output_dir, "./");
        assert!(!config.verbose);
    }

    #[test]
    fn test_mock_builder() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();
        let output_dir = temp_dir.path().join("output");
        let output_dir_str = output_dir.to_str().unwrap();

        // Create project structure
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(temp_dir.path().join("main.mock"), "mock content").unwrap();

        let config = BuildConfig {
            project_path: project_path.to_string(),
            output_dir: output_dir_str.to_string(),
            verbose: false,
            optimization_level: OptimizationLevel::Release,
            target_type: TargetType::Standard,
        };

        let builder = MockBuilder;
        let result = builder.build(&config).unwrap();

        assert!(result.wasm_path.ends_with("test.wasm"));
        assert!(Path::new(&result.wasm_path).exists());
    }

    #[test]
    fn test_validate_project_success() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create entry file
        fs::write(temp_dir.path().join("main.mock"), "mock content").unwrap();

        let builder = MockBuilder;
        assert!(builder.validate_project(project_path).is_ok());
    }

    #[test]
    fn test_validate_project_missing_entry() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        let builder = MockBuilder;
        let result = builder.validate_project(project_path);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No entry file found"));
    }
}
