pub mod builder;
mod detect;

pub use builder::build_wasm_project;
pub use detect::{
    detect_operating_system, detect_project_language, get_missing_tools, print_system_info,
    OperatingSystem, ProjectLanguage,
};

use crate::compiler::builder::WasmBuilder;
use crate::error::{ChakraError, Result};
use crate::plugin::languages::rust_plugin::RustPlugin;
use crate::utils::PathResolver;

/// Compile a WASM file from a project directory
// TODO: Remove this function in the future.
#[allow(dead_code)]
pub fn create_wasm_from_project(project_path: &str, output_dir: &str) -> Result<String> {
    let language_type: ProjectLanguage = detect_project_language(project_path);

    PathResolver::ensure_output_directory(output_dir)?;

    let result = build_wasm_project(project_path, output_dir, &language_type, false)
        .map_err(ChakraError::Compilation)?;
    Ok(result.wasm_path)
}

/// AOT compile a project for execution
pub fn compile_for_execution(project_path: &str, output_dir: &str) -> Result<String> {
    let language_type: ProjectLanguage = detect_project_language(project_path);
    let os: OperatingSystem = detect_operating_system();

    let missing_tools = get_missing_tools(&language_type, &os);
    if !missing_tools.is_empty() {
        return Err(ChakraError::missing_tools(missing_tools));
    }

    PathResolver::ensure_output_directory(output_dir)?;

    let result = build_wasm_project(project_path, output_dir, &language_type, true)
        .map_err(ChakraError::Compilation)?;

    Ok(result.js_path.unwrap_or(result.wasm_path))
}

/// Build a web application from a Rust project
pub fn build_rust_web_application(project_path: &str, output_dir: &str) -> Result<String> {
    let config = builder::BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        verbose: true,
        optimization_level: builder::OptimizationLevel::Release,
        target_type: builder::TargetType::WebApp,
    };

    let builder = RustPlugin::new();
    let result = builder.build(&config).map_err(ChakraError::Compilation)?;

    Ok(result.js_path.unwrap_or(result.wasm_path))
}

/// Check if a project is a Rust web application
pub fn is_rust_web_application(project_path: &str) -> bool {
    let builder = RustPlugin::new();
    builder.is_rust_web_application(project_path)
}
