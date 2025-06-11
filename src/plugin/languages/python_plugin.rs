use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::CommandExecutor;
use std::fs;

/// Python WebAssembly plugin
pub struct PythonPlugin {
    info: PluginInfo,
}

impl PythonPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "python".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: "Python WebAssembly compiler".into(),
            author: "Chakra Team".into(),
            extensions: vec!["py".into()],
            entry_files: vec!["main.py".into(), "app.py".into(), "requirements.txt".into()],
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

    // TODO: Python to WebAssembly compilation methods
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
    fn language_name(&self) -> &str {
        "Python"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &[
            "main.py",
            "app.py",
            "index.py",
            "src/main.py",
            "requirements.txt",
        ]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["py"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        // TODO: Dependency checking

        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("python")
            && !CommandExecutor::is_tool_installed("python3")
        {
            missing.push("python or python3 (Python interpreter)".to_string());
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
        // TODO: Implement Python WebAssembly build
        println!("🔨 Python WebAssembly compilation - TBD");

        Err(CompilationError::BuildFailed {
            language: self.language_name().to_string(),
            reason: "Python WebAssembly compilation not yet implemented".to_string(),
        })
    }
}

impl Default for PythonPlugin {
    fn default() -> Self {
        Self::new()
    }
}
