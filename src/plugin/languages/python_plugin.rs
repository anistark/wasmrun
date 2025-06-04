use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::PathResolver;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct PythonPlugin {
    info: PluginInfo,
    #[allow(dead_code)]
    builder: Arc<PythonBuilder>,
}

impl PythonPlugin {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let builder = Arc::new(PythonBuilder::new());

        let info = PluginInfo {
            name: "python".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Python WebAssembly compiler (coming soon)".to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec!["py".to_string()],
            entry_files: vec!["main.py".to_string(), "pyproject.toml".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: false, // Not yet implemented
                compile_webapp: false,
                live_reload: false,
                optimization: false,
                custom_targets: vec![],
            },
        };

        Self { info, builder }
    }
}

impl Plugin for PythonPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check for pyproject.toml or setup.py
        let pyproject_path = PathResolver::join_paths(project_path, "pyproject.toml");
        let setup_path = PathResolver::join_paths(project_path, "setup.py");

        if Path::new(&pyproject_path).exists() || Path::new(&setup_path).exists() {
            return true;
        }

        // Look for .py files
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "py" {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(PythonBuilder::new())
    }
}

pub struct PythonBuilder;

impl PythonBuilder {
    pub fn new() -> Self {
        Self
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
}

impl WasmBuilder for PythonBuilder {
    fn language_name(&self) -> &str {
        "Python"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.py", "app.py", "setup.py", "pyproject.toml"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["py"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !self.is_tool_installed("python") {
            missing.push("python (Python interpreter)".to_string());
        }

        // Python WASM compilation is not yet implemented
        missing.push("Python WebAssembly compilation is coming soon".to_string());

        missing
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(CompilationError::UnsupportedLanguage {
            language: "Python".to_string(),
        })
    }
}
