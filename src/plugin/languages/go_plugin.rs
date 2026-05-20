use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult, Result};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::Path;

/// Go WebAssembly plugin (uses TinyGo)
#[derive(Clone)]
pub struct GoPlugin {
    info: PluginInfo,
}

impl GoPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "go".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Go WebAssembly compiler using TinyGo".to_string(),
            author: "Wasmrun Team".to_string(),
            extensions: vec!["go".to_string()],
            entry_files: vec!["go.mod".to_string(), "main.go".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: false,
                optimization: true,
                custom_targets: vec!["wasm".to_string(), "wasi".to_string()],
                supported_languages: Some(vec!["go".to_string()]),
            },
        };

        Self { info }
    }

    fn is_go_project(project_path: &str) -> bool {
        let path = Path::new(project_path);

        if path.join("go.mod").exists() {
            return true;
        }

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if ext == "go" {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn find_package_name(project_path: &str) -> String {
        let go_mod = Path::new(project_path).join("go.mod");
        if let Ok(content) = fs::read_to_string(go_mod) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("module ") {
                    let module = line.trim_start_matches("module ").trim();
                    // Use just the last path segment as the name
                    return module.split('/').next_back().unwrap_or(module).to_string();
                }
            }
        }
        "main".to_string()
    }
}

impl Plugin for GoPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        Self::is_go_project(project_path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(GoPlugin::new())
    }
}

impl WasmBuilder for GoPlugin {
    fn supported_extensions(&self) -> &[&str] {
        &["go"]
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["go.mod", "main.go"]
    }

    fn language_name(&self) -> &str {
        "Go"
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if !CommandExecutor::is_tool_installed("tinygo") {
            missing.push(
                "tinygo (install from https://tinygo.org/getting-started/install/)".to_string(),
            );
        }
        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        PathResolver::validate_directory_exists(project_path).map_err(|e| {
            CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("Project directory validation failed: {e}"),
            }
        })?;

        if !Self::is_go_project(project_path) {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: "No go.mod or .go files found".to_string(),
            });
        }

        Ok(())
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        Self::is_go_project(project_path)
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        if !CommandExecutor::is_tool_installed("tinygo") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "tinygo".to_string(),
                language: self.language_name().to_string(),
            });
        }

        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        let pkg_name = Self::find_package_name(&config.project_path);
        let wasm_output = Path::new(&config.output_dir)
            .join(format!("{pkg_name}.wasm"))
            .to_string_lossy()
            .to_string();

        if config.verbose {
            println!("🔨 Building Go project with TinyGo...");
        }

        // Try wasi target first, fall back to wasm
        let targets = ["wasi", "wasm"];
        let mut last_error = String::new();

        for target in &targets {
            let output = CommandExecutor::execute_command(
                "tinygo",
                &["build", "-o", &wasm_output, "-target", target, "."],
                &config.project_path,
                config.verbose,
            );

            match output {
                Ok(result) if result.status.success() => {
                    if Path::new(&wasm_output).exists() {
                        return Ok(BuildResult {
                            wasm_path: wasm_output,
                            js_path: None,
                            additional_files: vec![],
                            is_wasm_bindgen: false,
                        });
                    }
                }
                Ok(result) => {
                    last_error = String::from_utf8_lossy(&result.stderr).to_string();
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }
        }

        Err(CompilationError::BuildFailed {
            language: self.language_name().to_string(),
            reason: format!("TinyGo build failed: {last_error}"),
        })
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        let artifacts = ["main.wasm", "*.wasm"];
        for artifact in &artifacts {
            let path = Path::new(project_path).join(artifact);
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }
}

impl Default for GoPlugin {
    fn default() -> Self {
        Self::new()
    }
}
