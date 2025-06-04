use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct AssemblyScriptPlugin {
    info: PluginInfo,
    #[allow(dead_code)]
    builder: Arc<AssemblyScriptBuilder>,
}

impl AssemblyScriptPlugin {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let builder = Arc::new(AssemblyScriptBuilder::new());

        let info = PluginInfo {
            name: "assemblyscript".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "AssemblyScript WebAssembly compiler".to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec!["ts".to_string()],
            entry_files: vec!["package.json".to_string(), "asconfig.json".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32".to_string()],
            },
        };

        Self { info, builder }
    }
}

impl Plugin for AssemblyScriptPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check for package.json with AssemblyScript dependency
        let package_json_path = PathResolver::join_paths(project_path, "package.json");
        if Path::new(&package_json_path).exists() {
            if let Ok(content) = fs::read_to_string(&package_json_path) {
                if content.contains("assemblyscript") {
                    return true;
                }
            }
        }

        // Check for asconfig.json
        let asconfig_path = PathResolver::join_paths(project_path, "asconfig.json");
        if Path::new(&asconfig_path).exists() {
            return true;
        }

        // Look for .ts files
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "ts" {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(AssemblyScriptBuilder::new())
    }
}

pub struct AssemblyScriptBuilder;

impl AssemblyScriptBuilder {
    pub fn new() -> Self {
        Self
    }
}

impl WasmBuilder for AssemblyScriptBuilder {
    fn language_name(&self) -> &str {
        "AssemblyScript"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["assembly/index.ts", "package.json", "asconfig.json"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["ts"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("node") {
            missing.push("node (Node.js - install from https://nodejs.org)".to_string());
        }

        if !CommandExecutor::is_tool_installed("npm") {
            missing.push("npm (Node Package Manager)".to_string());
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

        // Check for package.json
        let package_json_path = PathResolver::join_paths(project_path, "package.json");
        if !Path::new(&package_json_path).exists() {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("No package.json found in {}", project_path),
            });
        }

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if Node.js is installed
        if !CommandExecutor::is_tool_installed("node") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "node".to_string(),
                language: self.language_name().to_string(),
            });
        }

        // Create output directory if it doesn't exist
        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        println!("⚙️ Building with AssemblyScript...");

        // Try to build with npx asc first
        let build_output = CommandExecutor::execute_command(
            "npx",
            &[
                "asc",
                "--optimize",
                "--outFile",
                "build/release.wasm",
                "assembly/index.ts",
            ],
            &config.project_path,
            config.verbose,
        );

        // Check if we succeeded
        let wasm_file = if let Ok(output) = build_output {
            if output.status.success() {
                Path::new(&config.project_path).join("build/release.wasm")
            } else {
                // Try npm build command instead
                let npm_build = CommandExecutor::execute_command(
                    "npm",
                    &["run", "asbuild"],
                    &config.project_path,
                    config.verbose,
                )?;

                if !npm_build.status.success() {
                    return Err(CompilationError::BuildFailed {
                        language: self.language_name().to_string(),
                        reason: format!(
                            "Build failed: {}",
                            String::from_utf8_lossy(&npm_build.stderr)
                        ),
                    });
                }

                // Look for build output files
                let build_dir = Path::new(&config.project_path).join("build");
                let mut wasm_path = None;

                if build_dir.exists() {
                    if let Ok(entries) = fs::read_dir(&build_dir) {
                        for entry in entries.flatten() {
                            if let Some(extension) = entry.path().extension() {
                                if extension == "wasm" {
                                    wasm_path = Some(entry.path());
                                    break;
                                }
                            }
                        }
                    }
                }

                wasm_path.ok_or_else(|| CompilationError::BuildFailed {
                    language: self.language_name().to_string(),
                    reason: "No WASM file found after build".to_string(),
                })?
            }
        } else {
            return Err(CompilationError::BuildToolNotFound {
                tool: "AssemblyScript compiler".to_string(),
                language: self.language_name().to_string(),
            });
        };

        // Copy the wasm file to the output directory
        let output_file = CommandExecutor::copy_to_output(
            wasm_file.to_string_lossy().as_ref(),
            &config.output_dir,
            "AssemblyScript",
        )?;

        Ok(BuildResult {
            wasm_path: output_file,
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }
}
