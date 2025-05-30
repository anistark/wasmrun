use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};

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

    fn build(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        // For now, call the existing build_wasm function which returns an error
        build_wasm(&config.project_path, &config.output_dir)?;

        // This will never be reached since build_wasm returns an error
        unreachable!()
    }
}

/// Build a WASM file from a Python project
/// TODO: Integrate chakrapy
pub fn build_wasm(project_path: &str, _output_dir: &str) -> Result<String, String> {
    println!("🐍 Detected Python project at: {}", project_path);
    println!("\x1b[1;33mPython WebAssembly compilation coming soon!\x1b[0m");
    Err("Python WebAssembly compilation is coming soon.".to_string())
}
