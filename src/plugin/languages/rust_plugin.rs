use crate::compiler::builder::{BuildConfig, BuildResult, OptimizationLevel, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::Path;

/// Rust WebAssembly plugin
pub struct RustPlugin {
    info: PluginInfo,
}

impl RustPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "rust".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Rust WebAssembly compiler with wasm-bindgen and web application support"
                .to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec!["rs".to_string()],
            entry_files: vec!["Cargo.toml".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
            },
        };

        Self { info }
    }

    /// Check if a Rust project uses wasm-bindgen
    pub fn uses_wasm_bindgen(&self, project_path: &str) -> bool {
        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");

        if let Ok(cargo_toml) = fs::read_to_string(cargo_toml_path) {
            cargo_toml.contains("wasm-bindgen")
                || cargo_toml.contains("web-sys")
                || cargo_toml.contains("js-sys")
        } else {
            false
        }
    }

    /// Check if a project is a Rust web application
    pub fn is_rust_web_application(&self, project_path: &str) -> bool {
        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");

        if let Ok(cargo_toml) = fs::read_to_string(cargo_toml_path) {
            let uses_wasm_bindgen = self.uses_wasm_bindgen(project_path);

            if !uses_wasm_bindgen {
                return false;
            }

            // Look for web framework dependencies
            let web_frameworks = [
                "yew", "leptos", "dioxus", "sycamore", "mogwai", "seed", "percy", "iced", "dodrio",
                "smithy", "trunk",
            ];

            for framework in web_frameworks {
                if cargo_toml.contains(framework) {
                    return true;
                }
            }

            // Check for lib target with cdylib
            if cargo_toml.contains("[lib]") && cargo_toml.contains("cdylib") {
                // Check if there's an index.html in the project
                if Path::new(project_path).join("index.html").exists() {
                    return true;
                }

                // Check for static directories that might indicate a web app
                let potential_static_dirs = ["public", "static", "assets", "dist", "www"];
                for dir in potential_static_dirs {
                    if Path::new(project_path).join(dir).exists() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Build standard WASM without wasm-bindgen
    fn build_standard_wasm(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Ensure wasm32 target is installed
        self.ensure_wasm32_target()?;

        // Determine build args based on optimization level
        let mut args = vec!["build", "--target", "wasm32-unknown-unknown"];

        match config.optimization_level {
            OptimizationLevel::Release => args.push("--release"),
            OptimizationLevel::Size => {
                args.push("--release");
                // TODO: Add size optimization flags
            }
            OptimizationLevel::Debug => {}
        }

        // Execute cargo build
        let output = if config.verbose {
            CommandExecutor::execute_command_with_output("cargo", &args, &config.project_path)?;
            true
        } else {
            let output =
                CommandExecutor::execute_command("cargo", &args, &config.project_path, false)?;
            output.status.success()
        };

        if !output {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "Cargo build failed".to_string(),
            });
        }

        // Find the generated WASM file
        let build_type = match config.optimization_level {
            OptimizationLevel::Debug => "debug",
            _ => "release",
        };

        let target_dir = PathResolver::join_paths(
            &config.project_path,
            &format!("target/wasm32-unknown-unknown/{}", build_type),
        );

        let wasm_files =
            PathResolver::find_files_with_extension(&target_dir, "wasm").map_err(|e| {
                CompilationError::BuildFailed {
                    language: "Rust".to_string(),
                    reason: format!("Failed to find WASM files: {}", e),
                }
            })?;

        if wasm_files.is_empty() {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "No WASM file found in target directory".to_string(),
            });
        }

        let wasm_file = &wasm_files[0];
        let output_path = CommandExecutor::copy_to_output(wasm_file, &config.output_dir, "Rust")?;

        Ok(BuildResult {
            wasm_path: output_path,
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    /// Build wasm-bindgen project
    fn build_wasm_bindgen(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if wasm-pack is installed
        if !CommandExecutor::is_tool_installed("wasm-pack") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "wasm-pack".to_string(),
                language: "Rust".to_string(),
            });
        }

        // Ensure wasm32 target is installed
        self.ensure_wasm32_target()?;

        // Determine wasm-pack args
        let mut args = vec!["build", "--target", "web"];

        match config.optimization_level {
            OptimizationLevel::Debug => args.push("--dev"),
            OptimizationLevel::Release => {} // Default
            OptimizationLevel::Size => {
                // TODO: Add size optimization flags for wasm-pack
            }
        }

        // Add no-typescript flag to simplify output
        args.push("--no-typescript");

        // Execute wasm-pack build
        let output = if config.verbose {
            CommandExecutor::execute_command_with_output("wasm-pack", &args, &config.project_path)?;
            true
        } else {
            let output =
                CommandExecutor::execute_command("wasm-pack", &args, &config.project_path, false)?;
            output.status.success()
        };

        if !output {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "wasm-pack build failed".to_string(),
            });
        }

        // Generated files in pkg
        let pkg_dir = PathResolver::join_paths(&config.project_path, "pkg");

        let wasm_files =
            PathResolver::find_files_with_extension(&pkg_dir, "wasm").map_err(|e| {
                CompilationError::BuildFailed {
                    language: "Rust".to_string(),
                    reason: format!("Failed to find WASM files in pkg directory: {}", e),
                }
            })?;
        let js_files = PathResolver::find_files_with_extension(&pkg_dir, "js").map_err(|e| {
            CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: format!("Failed to find JS files in pkg directory: {}", e),
            }
        })?;

        if wasm_files.is_empty() {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "No WASM file found in pkg directory".to_string(),
            });
        }

        let main_js_file = js_files
            .iter()
            .find(|path| !path.contains(".d.js"))
            .ok_or_else(|| CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "No main JS file found in pkg directory".to_string(),
            })?;

        // Copy files to output directory
        let wasm_output =
            CommandExecutor::copy_to_output(&wasm_files[0], &config.output_dir, "Rust")?;
        let js_output = CommandExecutor::copy_to_output(main_js_file, &config.output_dir, "Rust")?;

        // Copy any additional files (.d.ts, etc.)
        let mut additional_files = Vec::new();
        let all_pkg_files = fs::read_dir(&pkg_dir).map_err(|e| CompilationError::BuildFailed {
            language: "Rust".to_string(),
            reason: format!("Failed to read pkg directory: {}", e),
        })?;

        for entry in all_pkg_files.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "ts" || ext_str == "json" {
                    let copied_file = CommandExecutor::copy_to_output(
                        &path.to_string_lossy(),
                        &config.output_dir,
                        "Rust",
                    )?;
                    additional_files.push(copied_file);
                }
            }
        }

        Ok(BuildResult {
            wasm_path: wasm_output,
            js_path: Some(js_output),
            additional_files,
            is_wasm_bindgen: true,
        })
    }

    /// Build Rust web application (like Yew, Leptos, etc.)
    fn build_web_application(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if this project uses Trunk
        let uses_trunk = Path::new(&config.project_path).join("Trunk.toml").exists()
            || Path::new(&config.project_path).join("trunk.toml").exists();

        if uses_trunk {
            self.build_with_trunk(config)
        } else {
            // Fall back to wasm-pack
            self.build_wasm_bindgen(config)
        }
    }

    /// Install wasm32 target if not present
    fn ensure_wasm32_target(&self) -> CompilationResult<()> {
        let check_target = std::process::Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output()
            .map_err(|e| CompilationError::ToolExecutionFailed {
                tool: "rustup".to_string(),
                reason: e.to_string(),
            })?;

        let target_output = String::from_utf8_lossy(&check_target.stdout);

        if !target_output.contains("wasm32-unknown-unknown") {
            println!("⚙️ Installing wasm32-unknown-unknown target...");
            let install_output = std::process::Command::new("rustup")
                .args(["target", "add", "wasm32-unknown-unknown"])
                .output()
                .map_err(|e| CompilationError::ToolExecutionFailed {
                    tool: "rustup".to_string(),
                    reason: e.to_string(),
                })?;

            if !install_output.status.success() {
                return Err(CompilationError::ToolExecutionFailed {
                    tool: "rustup".to_string(),
                    reason: String::from_utf8_lossy(&install_output.stderr).to_string(),
                });
            }

            println!("✅ wasm32-unknown-unknown target installed");
        }

        Ok(())
    }

    /// Build with Trunk (for web frameworks like Yew)
    fn build_with_trunk(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        if !CommandExecutor::is_tool_installed("trunk") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "trunk".to_string(),
                language: "Rust".to_string(),
            });
        }

        // Determine trunk args
        let mut args = vec!["build"];

        match config.optimization_level {
            OptimizationLevel::Release => args.push("--release"),
            OptimizationLevel::Debug => {} // Default
            OptimizationLevel::Size => {
                args.push("--release");
                // TODO: Add size optimization flags
            }
        }

        // Execute trunk build
        let output = if config.verbose {
            CommandExecutor::execute_command_with_output("trunk", &args, &config.project_path)?;
            true
        } else {
            let output =
                CommandExecutor::execute_command("trunk", &args, &config.project_path, false)?;
            output.status.success()
        };

        if !output {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "Trunk build failed".to_string(),
            });
        }

        // Copy the dist directory to output
        let trunk_dist = PathResolver::join_paths(&config.project_path, "dist");
        if !Path::new(&trunk_dist).exists() {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "Trunk build completed but dist directory was not created".to_string(),
            });
        }

        let wasm_files =
            PathResolver::find_files_with_extension(&trunk_dist, "wasm").map_err(|e| {
                CompilationError::BuildFailed {
                    language: "Rust".to_string(),
                    reason: format!("Failed to find WASM files in trunk dist: {}", e),
                }
            })?;
        let js_files = PathResolver::find_files_with_extension(&trunk_dist, "js").map_err(|e| {
            CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: format!("Failed to find JS files in trunk dist: {}", e),
            }
        })?;

        if wasm_files.is_empty() {
            return Err(CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: "No WASM file found in trunk dist directory".to_string(),
            });
        }

        // Copy dist directory contents to output
        Self::copy_directory_recursively(&trunk_dist, &config.output_dir).map_err(|e| {
            CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: format!("Failed to copy trunk dist directory: {}", e),
            }
        })?;

        // Return relative paths within the output directory
        let wasm_filename = PathResolver::get_filename(&wasm_files[0]).map_err(|e| {
            CompilationError::BuildFailed {
                language: "Rust".to_string(),
                reason: format!("Failed to get WASM filename: {}", e),
            }
        })?;
        let js_filename = if !js_files.is_empty() {
            Some(PathResolver::get_filename(&js_files[0]).map_err(|e| {
                CompilationError::BuildFailed {
                    language: "Rust".to_string(),
                    reason: format!("Failed to get JS filename: {}", e),
                }
            })?)
        } else {
            None
        };

        Ok(BuildResult {
            wasm_path: PathResolver::join_paths(&config.output_dir, &wasm_filename),
            js_path: js_filename.map(|name| PathResolver::join_paths(&config.output_dir, &name)),
            additional_files: vec![], // TODO: Track all copied files
            is_wasm_bindgen: true,
        })
    }

    /// Copy directory recursively
    fn copy_directory_recursively(source: &str, destination: &str) -> Result<(), String> {
        PathResolver::ensure_output_directory(destination)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;

        let entries = fs::read_dir(source)
            .map_err(|e| format!("Failed to read source directory {}: {}", source, e))?;

        for entry in entries.flatten() {
            let source_path = entry.path();
            let file_name = source_path
                .file_name()
                .ok_or_else(|| "Invalid file name".to_string())?;
            let destination_path = Path::new(destination).join(file_name);

            if source_path.is_dir() {
                Self::copy_directory_recursively(
                    &source_path.to_string_lossy(),
                    &destination_path.to_string_lossy(),
                )?;
            } else {
                fs::copy(&source_path, &destination_path)
                    .map_err(|e| format!("Failed to copy file: {}", e))?;
            }
        }

        Ok(())
    }
}

impl Plugin for RustPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");
        Path::new(&cargo_toml_path).exists()
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(RustPlugin::new())
    }
}

impl WasmBuilder for RustPlugin {
    fn language_name(&self) -> &str {
        "Rust"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("rustc") {
            missing.push("rustc (Rust compiler)".to_string());
        }

        if !CommandExecutor::is_tool_installed("cargo") {
            missing.push("cargo (Rust package manager)".to_string());
        }

        // Check for wasm32 target
        let check_target = std::process::Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output();

        if let Ok(output) = check_target {
            let target_output = String::from_utf8_lossy(&output.stdout);
            if !target_output.contains("wasm32-unknown-unknown") {
                missing.push("wasm32-unknown-unknown target (install with: rustup target add wasm32-unknown-unknown)".to_string());
            }
        }

        // Check for wasm-pack
        if self.uses_wasm_bindgen(&BuildConfig::default().project_path)
            && !CommandExecutor::is_tool_installed("wasm-pack")
        {
            missing.push("wasm-pack (install with: cargo install wasm-pack)".to_string());
        }

        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        PathResolver::validate_directory_exists(project_path).map_err(|e| {
            CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("Project directory validation failed: {}", e),
            }
        })?;

        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");
        if !Path::new(&cargo_toml_path).exists() {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("No Cargo.toml found in {}", project_path),
            });
        }

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if wasm-bindgen project
        if self.uses_wasm_bindgen(&config.project_path) {
            if self.is_rust_web_application(&config.project_path) {
                self.build_web_application(config)
            } else {
                self.build_wasm_bindgen(config)
            }
        } else {
            self.build_standard_wasm(config)
        }
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!(
            "🔨 Building {} project at: {}",
            self.language_name(),
            config.project_path
        );

        let missing_tools = self.check_dependencies();
        if !missing_tools.is_empty() {
            return Err(CompilationError::BuildToolNotFound {
                tool: missing_tools.join(", "),
                language: self.language_name().to_string(),
            });
        }

        // Validate project structure
        self.validate_project(&config.project_path)?;

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
}

impl Default for RustPlugin {
    fn default() -> Self {
        Self::new()
    }
}
