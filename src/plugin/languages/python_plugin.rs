use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::{Path, PathBuf};

/// Python WebAssembly plugin
#[derive(Clone)]
pub struct PythonPlugin {
    info: PluginInfo,
}

impl PythonPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "python".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: "Python WebAssembly compiler".into(),
            author: "Wasmrun Team".into(),
            extensions: vec!["py".into()],
            entry_files: vec!["main.py".into(), "app.py".into()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: true,
                optimization: false,
                custom_targets: vec!["wasm".into(), "web".into()],
            },
        };

        Self { info }
    }

    /// Find the entry point of the project
    fn find_entry_file(&self, project_path: &str) -> CompilationResult<PathBuf> {
        let common_entry_files = &self.info.entry_files;

        for entry_name in common_entry_files.iter() {
            let entry_path = Path::new(project_path).join(entry_name);
            if entry_path.exists() {
                return Ok(entry_path);
            }
        }

        // If no common entry file found, look for any .py file
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "py" {
                        return Ok(entry.path());
                    }
                }
            }
        }

        Err(CompilationError::MissingEntryFile {
            language: self.language_name().to_string(),
            candidates: vec!["main.py".to_string(), "app.py".to_string()],
        })
    }
}

impl Plugin for PythonPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // TODO: Detect project
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "py" {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(PythonPlugin::new())
    }
}

impl WasmBuilder for PythonPlugin {
    fn supported_extensions(&self) -> &[&str] {
        &["py"]
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.py", "app.py", "index.py", "src/main.py", "__main__.py"]
    }

    fn language_name(&self) -> &str {
        "Python"
    }

    fn check_dependencies(&self) -> Vec<String> {
        // Checking if Python3.11.0 is present

        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("python")
            && !CommandExecutor::is_tool_installed("python3")
        {
            missing.push("python or python3 (Python interpreter)".into());
        } else {
            let version =
                CommandExecutor::execute_command("python3", &["--version"], ".", true).unwrap();
            if version.status.success() {
                let version_number: String = String::from_utf8(version.stdout).unwrap();
                if !version_number.contains("3.11.0") {
                    missing.push("Python 3.11.0 is required for py2wasm to work!".into());
                }
            }
        }
        if !CommandExecutor::is_tool_installed("py2wasm") {
            missing.push("py2wasm is not installed, try running pip install py2wasm".into());
        }

        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        // TODO: Project validation
        if !std::path::Path::new(project_path).exists() {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: "Project directory does not exist".to_string(),
            });
        }

        Ok(())
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        let project_entry = self.find_entry_file(&_config.project_path).unwrap();
        PathResolver::ensure_output_directory(&_config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: _config.output_dir.clone(),
            }
        })?;

        let output_name = project_entry
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string()
            + ".wasm";
        let output_file = Path::new(&_config.output_dir).join(&output_name);

        let build_result = CommandExecutor::execute_command(
            "py2wasm",
            &[
                project_entry.to_str().unwrap(),
                "-o",
                output_file.to_str().unwrap(),
            ],
            &_config.project_path,
            _config.verbose,
        )
        .unwrap();
        if !build_result.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "Build failed: {}",
                    String::from_utf8_lossy(&build_result.stderr)
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

    fn can_handle_project(&self, project_path: &str) -> bool {
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if self.supported_extensions().contains(&ext.as_str()) {
                        return true;
                    }
                }
            }
        }

        for entry_file in self.entry_file_candidates() {
            let file_path = std::path::Path::new(project_path).join(entry_file);
            if file_path.exists() {
                return true;
            }
        }
        false
    }

    fn clean(&self, project_path: &str) -> crate::error::Result<()> {
        // Clean Python build artifacts
        let dist_dir = std::path::Path::new(project_path).join("dist");
        if dist_dir.exists() {
            std::fs::remove_dir_all(dist_dir)?;
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }
}

impl Default for PythonPlugin {
    fn default() -> Self {
        Self::new()
    }
}
