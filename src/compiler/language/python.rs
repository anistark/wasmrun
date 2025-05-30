use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::CompilationResult;

pub struct PythonBuilder;

impl PythonBuilder {
    pub fn new() -> Self {
        Self
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
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: "Python".to_string(),
        })
    }
}
