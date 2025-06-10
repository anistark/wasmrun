use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::{Path, PathBuf};

/// Go WebAssembly plugin
pub struct GoPlugin {
    info: PluginInfo,
}

impl GoPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "go".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Go WebAssembly compiler using TinyGo".to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec!["go".to_string()],
            entry_files: vec!["go.mod".to_string(), "main.go".to_string()],
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

    /// Find main.go or similar entry point
    fn find_entry_file(&self, project_path: &str) -> CompilationResult<PathBuf> {
        let common_entry_files = ["main.go", "cmd/main.go", "app.go"];

        for entry_name in common_entry_files.iter() {
            let entry_path = Path::new(project_path).join(entry_name);
            if entry_path.exists() {
                return Ok(entry_path);
            }
        }

        // If no common entry file found, look for any .go file
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "go" {
                        return Ok(entry.path());
                    }
                }
            }
        }

        Err(CompilationError::MissingEntryFile {
            language: self.language_name().to_string(),
            candidates: vec![
                "main.go".to_string(),
                "cmd/main.go".to_string(),
                "app.go".to_string(),
            ],
        })
    }
}

impl Plugin for GoPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        let go_mod_path = PathResolver::join_paths(project_path, "go.mod");
        if Path::new(&go_mod_path).exists() {
            return true;
        }

        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "go" {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(GoPlugin::new())
    }
}

impl WasmBuilder for GoPlugin {
    fn language_name(&self) -> &str {
        "Go"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.go", "cmd/main.go", "app.go", "go.mod"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["go"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("tinygo") {
            missing.push("tinygo (install from https://tinygo.org)".to_string());
        }

        if !CommandExecutor::is_tool_installed("go") {
            missing.push("go (Go compiler)".to_string());
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

        let _ = self.find_entry_file(project_path)?;

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if TinyGo is installed
        if !CommandExecutor::is_tool_installed("tinygo") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "tinygo".to_string(),
                language: self.language_name().to_string(),
            });
        }

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
            .to_string()
            + ".wasm";
        let output_file = Path::new(&config.output_dir).join(&output_name);

        println!("🔨 Building with TinyGo...");

        // Build with TinyGo
        let build_output = CommandExecutor::execute_command(
            "tinygo",
            &[
                "build",
                "-o",
                output_file.to_str().unwrap(),
                "-target=wasm",
                entry_path.to_str().unwrap(),
            ],
            &config.project_path,
            config.verbose,
        )?;

        if !build_output.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "Build failed: {}",
                    String::from_utf8_lossy(&build_output.stderr)
                ),
            });
        }

        Ok(BuildResult {
            wasm_path: output_file.to_string_lossy().to_string(),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }
}

impl Default for GoPlugin {
    fn default() -> Self {
        Self::new()
    }
}
