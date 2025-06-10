use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::{Path, PathBuf};

/// C WebAssembly plugin
pub struct CPlugin {
    info: PluginInfo,
}

impl CPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "c".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "C WebAssembly compiler using Emscripten".to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec!["c".to_string(), "h".to_string()],
            entry_files: vec!["main.c".to_string(), "Makefile".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm".to_string(), "web".to_string()],
            },
        };

        Self { info }
    }

    /// Find main.c or similar entry point
    fn find_entry_file(&self, project_path: &str) -> CompilationResult<PathBuf> {
        let common_entry_files = ["main.c", "src/main.c", "app.c", "index.c"];

        for entry_name in common_entry_files.iter() {
            let entry_path = Path::new(project_path).join(entry_name);
            if entry_path.exists() {
                return Ok(entry_path);
            }
        }

        // If no common entry file found, look for any .c file
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "c" {
                        return Ok(entry.path());
                    }
                }
            }
        }

        Err(CompilationError::MissingEntryFile {
            language: self.language_name().to_string(),
            candidates: vec![
                "main.c".to_string(),
                "src/main.c".to_string(),
                "app.c".to_string(),
                "index.c".to_string(),
            ],
        })
    }

    /// Check if project uses a Makefile
    fn has_makefile(&self, project_path: &str) -> bool {
        let makefile_variants = ["Makefile", "makefile", "GNUmakefile"];

        for variant in makefile_variants {
            let makefile_path = PathResolver::join_paths(project_path, variant);
            if Path::new(&makefile_path).exists() {
                return true;
            }
        }

        false
    }

    /// Build using Makefile if available
    fn build_with_makefile(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if make is installed
        if !CommandExecutor::is_tool_installed("make") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "make".to_string(),
                language: self.language_name().to_string(),
            });
        }

        // Execute make
        let build_output = CommandExecutor::execute_command(
            "make",
            &["wasm"],
            &config.project_path,
            config.verbose,
        )?;

        if !build_output.status.success() {
            // Try default make target
            let build_output = CommandExecutor::execute_command(
                "make",
                &[],
                &config.project_path,
                config.verbose,
            )?;

            if !build_output.status.success() {
                return Err(CompilationError::BuildFailed {
                    language: self.language_name().to_string(),
                    reason: format!(
                        "Make build failed: {}",
                        String::from_utf8_lossy(&build_output.stderr)
                    ),
                });
            }
        }

        // Look for generated WASM files
        let wasm_files = PathResolver::find_files_with_extension(&config.project_path, "wasm")
            .map_err(|e| CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Failed to find WASM files after make build: {}", e),
            })?;

        if wasm_files.is_empty() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "No WASM file found after make build".to_string(),
            });
        }

        // Copy the first WASM file to output directory
        let output_path = CommandExecutor::copy_to_output(&wasm_files[0], &config.output_dir, "C")?;

        // Look for JS files (for Emscripten)
        let js_files =
            PathResolver::find_files_with_extension(&config.project_path, "js").unwrap_or_default();

        let js_output_path = if !js_files.is_empty() {
            Some(CommandExecutor::copy_to_output(
                &js_files[0],
                &config.output_dir,
                "C",
            )?)
        } else {
            None
        };

        let has_js_bindings = js_output_path.is_some();

        Ok(BuildResult {
            wasm_path: output_path,
            js_path: js_output_path,
            additional_files: vec![],
            is_wasm_bindgen: has_js_bindings,
        })
    }

    /// Build using Emscripten directly
    fn build_with_emscripten(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Find entry file
        let entry_path = self.find_entry_file(&config.project_path)?;

        // Create output directory if it doesn't exist
        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        // Get the output filename
        let output_name = entry_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let wasm_output_file = Path::new(&config.output_dir).join(format!("{}.wasm", output_name));
        let js_output_file = Path::new(&config.output_dir).join(format!("{}.js", output_name));

        println!("🔨 Building with Emscripten...");

        // Collect all .c files in the project
        let c_files = self.collect_c_files(&config.project_path)?;

        // Build args for emcc
        let mut args = vec![
            "-o",
            js_output_file.to_str().unwrap(),
            "-s",
            "WASM=1",
            "-s",
            "EXPORTED_RUNTIME_METHODS=['cwrap']",
        ];

        // Add optimization flags based on build config
        match config.optimization_level {
            crate::compiler::builder::OptimizationLevel::Debug => {
                args.extend(&["-g", "-O0"]);
            }
            crate::compiler::builder::OptimizationLevel::Release => {
                args.extend(&["-O3"]);
            }
            crate::compiler::builder::OptimizationLevel::Size => {
                args.extend(&["-Os", "-s", "ELIMINATE_DUPLICATE_FUNCTIONS=1"]);
            }
        }

        // Add all C files
        for c_file in &c_files {
            args.push(c_file);
        }

        // Execute emcc
        let build_output =
            CommandExecutor::execute_command("emcc", &args, &config.project_path, config.verbose)?;

        if !build_output.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "Emscripten build failed: {}",
                    String::from_utf8_lossy(&build_output.stderr)
                ),
            });
        }

        // Check if files were generated
        if !wasm_output_file.exists() || !js_output_file.exists() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "Emscripten build completed but output files were not created".to_string(),
            });
        }

        Ok(BuildResult {
            wasm_path: wasm_output_file.to_string_lossy().to_string(),
            js_path: Some(js_output_file.to_string_lossy().to_string()),
            additional_files: vec![],
            is_wasm_bindgen: true,
        })
    }

    /// Collect all .c files in the project directory
    fn collect_c_files(&self, project_path: &str) -> CompilationResult<Vec<String>> {
        let mut c_files = Vec::new();

        let entries = fs::read_dir(project_path).map_err(|e| CompilationError::BuildFailed {
            language: self.language_name().to_string(),
            reason: format!("Failed to read project directory: {}", e),
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "c" {
                    if let Some(path_str) = path.to_str() {
                        c_files.push(path_str.to_string());
                    }
                }
            }
        }

        if c_files.is_empty() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "No .c files found in project directory".to_string(),
            });
        }

        Ok(c_files)
    }
}

impl Plugin for CPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check for Makefile
        if self.has_makefile(project_path) {
            return true;
        }

        // Look for .c files
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "c" {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(CPlugin::new())
    }
}

impl WasmBuilder for CPlugin {
    fn language_name(&self) -> &str {
        "C"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.c", "src/main.c", "app.c", "index.c", "Makefile"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["c", "h"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("emcc") {
            missing.push(
                "emcc (Emscripten compiler - install from https://emscripten.org)".to_string(),
            );
        }

        // Check for make if Makefile exists
        if self.has_makefile(&crate::compiler::builder::BuildConfig::default().project_path)
            && !CommandExecutor::is_tool_installed("make")
        {
            missing.push("make (build system)".to_string());
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

        // Check if we have either a Makefile or can find C files
        if !self.has_makefile(project_path) {
            let _ = self.find_entry_file(project_path)?;
        }

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if Emscripten is installed
        if !CommandExecutor::is_tool_installed("emcc") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "emcc".to_string(),
                language: self.language_name().to_string(),
            });
        }

        // Choose build method based on project structure
        if self.has_makefile(&config.project_path) {
            self.build_with_makefile(config)
        } else {
            self.build_with_emscripten(config)
        }
    }
}

impl Default for CPlugin {
    fn default() -> Self {
        Self::new()
    }
}
