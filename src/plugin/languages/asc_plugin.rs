use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::{Path, PathBuf};

/// AssemblyScript WebAssembly plugin
pub struct AscPlugin {
    info: PluginInfo,
}

impl AscPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "asc".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "AssemblyScript WebAssembly compiler".to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec!["ts".to_string()],
            entry_files: vec![
                "assembly/index.ts".to_string(),
                "index.ts".to_string(),
                "package.json".to_string(),
            ],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm".to_string()],
            },
        };

        Self { info }
    }

    /// Check if this is an AssemblyScript project
    fn is_asc_project(&self, project_path: &str) -> bool {
        let package_json_path = PathResolver::join_paths(project_path, "package.json");

        if let Ok(package_json) = fs::read_to_string(package_json_path) {
            // Check for AssemblyScript dependencies
            package_json.contains("asc") || package_json.contains("@asc")
        } else {
            false
        }
    }

    /// Find AssemblyScript entry file
    fn find_entry_file(&self, project_path: &str) -> CompilationResult<PathBuf> {
        let common_entry_files = [
            "assembly/index.ts",
            "assembly/main.ts",
            "src/index.ts",
            "src/main.ts",
            "index.ts",
            "main.ts",
        ];

        for entry_name in common_entry_files.iter() {
            let entry_path = Path::new(project_path).join(entry_name);
            if entry_path.exists() {
                return Ok(entry_path);
            }
        }

        // Look for any .ts file in assembly or src directories
        let search_dirs = ["assembly", "src", "."];
        for dir in search_dirs {
            let search_path = if dir == "." {
                project_path.to_string()
            } else {
                PathResolver::join_paths(project_path, dir)
            };

            if let Ok(entries) = fs::read_dir(&search_path) {
                for entry in entries.flatten() {
                    if let Some(extension) = entry.path().extension() {
                        if extension == "ts" {
                            return Ok(entry.path());
                        }
                    }
                }
            }
        }

        Err(CompilationError::MissingEntryFile {
            language: self.language_name().to_string(),
            candidates: vec![
                "assembly/index.ts".to_string(),
                "assembly/main.ts".to_string(),
                "src/index.ts".to_string(),
                "index.ts".to_string(),
            ],
        })
    }

    /// Build using asc
    fn build_with_asc(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let entry_path = self.find_entry_file(&config.project_path)?;

        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        let output_name = entry_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let wasm_output_file = Path::new(&config.output_dir).join(format!("{}.wasm", output_name));

        println!("🔨 Building with AssemblyScript compiler...");

        let mut args = vec![
            entry_path.to_str().unwrap(),
            "--target",
            "release",
            "--outFile",
            wasm_output_file.to_str().unwrap(),
        ];

        // Optimization flags
        match config.optimization_level {
            crate::compiler::builder::OptimizationLevel::Debug => {
                args.extend(&["--debug"]);
            }
            crate::compiler::builder::OptimizationLevel::Release => {
                args.extend(&["--optimize"]);
            }
            crate::compiler::builder::OptimizationLevel::Size => {
                args.extend(&["--optimize", "--shrinkLevel", "2"]);
            }
        }

        let build_output =
            CommandExecutor::execute_command("asc", &args, &config.project_path, config.verbose)?;

        if !build_output.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "AssemblyScript build failed: {}",
                    String::from_utf8_lossy(&build_output.stderr)
                ),
            });
        }

        if !wasm_output_file.exists() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "AssemblyScript build completed but WASM file was not created".to_string(),
            });
        }

        Ok(BuildResult {
            wasm_path: wasm_output_file.to_string_lossy().to_string(),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    fn build_with_npm(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let package_json_path = PathResolver::join_paths(&config.project_path, "package.json");

        if !Path::new(&package_json_path).exists() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "No package.json found for npm build".to_string(),
            });
        }

        // Check which package manager to use
        let cmd = if CommandExecutor::is_tool_installed("yarn")
            && Path::new(&config.project_path).join("yarn.lock").exists()
        {
            "yarn"
        } else if CommandExecutor::is_tool_installed("npm") {
            "npm"
        } else {
            return Err(CompilationError::BuildToolNotFound {
                tool: "npm or yarn".to_string(),
                language: self.language_name().to_string(),
            });
        };
        // TODO: Add support for pnpm, bun, etc.

        println!("🔨 Building with {}...", cmd);
        let args = if cmd == "yarn" {
            vec!["build"]
        } else {
            vec!["run", "build"]
        };

        let build_output =
            CommandExecutor::execute_command(cmd, &args, &config.project_path, config.verbose)?;

        if !build_output.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "{} build failed: {}",
                    cmd,
                    String::from_utf8_lossy(&build_output.stderr)
                ),
            });
        }

        // Look for generated WASM files in common output directories
        let search_dirs = ["build", "dist", "out", "target", "."];
        let mut wasm_files = Vec::new();

        for dir in search_dirs {
            let search_path = if dir == "." {
                config.project_path.clone()
            } else {
                PathResolver::join_paths(&config.project_path, dir)
            };

            if let Ok(files) = PathResolver::find_files_with_extension(&search_path, "wasm") {
                wasm_files.extend(files);
            }
        }

        if wasm_files.is_empty() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "No WASM file found after npm/yarn build".to_string(),
            });
        }

        // Copy the first WASM file to output directory
        let output_path =
            CommandExecutor::copy_to_output(&wasm_files[0], &config.output_dir, "AssemblyScript")?;

        Ok(BuildResult {
            wasm_path: output_path,
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }
}

impl Plugin for AscPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check if it's an AssemblyScript project
        if self.is_asc_project(project_path) {
            return true;
        }

        // Check for common AssemblyScript entry files
        let assembly_files = ["assembly/index.ts", "assembly/main.ts"];

        for file in assembly_files {
            let file_path = PathResolver::join_paths(project_path, file);
            if Path::new(&file_path).exists() {
                return true;
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(AscPlugin::new())
    }
}

impl WasmBuilder for AscPlugin {
    fn language_name(&self) -> &str {
        "Asc"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &[
            "assembly/index.ts",
            "assembly/main.ts",
            "src/index.ts",
            "index.ts",
            "package.json",
        ]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["ts"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        // Check for AssemblyScript compiler
        if !CommandExecutor::is_tool_installed("asc") {
            missing.push(
                "asc (AssemblyScript compiler - install with: npm install -g asc)".to_string(),
            );
        }

        if !CommandExecutor::is_tool_installed("node") {
            missing.push("node (Node.js runtime)".to_string());
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

        // Try to find an AssemblyScript entry file
        let _ = self.find_entry_file(project_path)?;

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        if Path::new(&config.project_path)
            .join("package.json")
            .exists()
        {
            match self.build_with_npm(config) {
                Ok(result) => Ok(result),
                Err(_) => {
                    if CommandExecutor::is_tool_installed("asc") {
                        self.build_with_asc(config)
                    } else {
                        Err(CompilationError::BuildToolNotFound {
                            tool: "asc or npm/yarn".to_string(),
                            language: self.language_name().to_string(),
                        })
                    }
                }
            }
        } else if CommandExecutor::is_tool_installed("asc") {
            self.build_with_asc(config)
        } else {
            Err(CompilationError::BuildToolNotFound {
                tool: "asc".to_string(),
                language: self.language_name().to_string(),
            })
        }
    }
}

impl Default for AscPlugin {
    fn default() -> Self {
        Self::new()
    }
}
