pub mod builder;
mod detect;
pub mod language;

pub use builder::build_wasm_project;
pub use detect::{
    detect_operating_system, detect_project_language, get_missing_tools, print_system_info,
    OperatingSystem, ProjectLanguage,
};
pub use language::rust::{build_rust_web_application, is_rust_web_application};

use crate::error::{ChakraError, Result};
use crate::utils::PathResolver;

/// Compile a WASM file from a project directory (legacy function)
#[allow(dead_code)]
pub fn create_wasm_from_project(project_path: &str, output_dir: &str) -> Result<String> {
    let language_type: ProjectLanguage = detect_project_language(project_path);

    // Create output directory if it doesn't exist
    PathResolver::ensure_output_directory(output_dir)?;

    // Use the new builder system - Fix redundant closure
    let result = build_wasm_project(project_path, output_dir, &language_type, false)
        .map_err(ChakraError::Compilation)?;
    Ok(result.wasm_path)
}

/// AOT compile a project for execution (legacy function)
pub fn compile_for_execution(project_path: &str, output_dir: &str) -> Result<String> {
    let language_type: ProjectLanguage = detect_project_language(project_path);
    let os: OperatingSystem = detect_operating_system();

    // Check for missing tools early
    let missing_tools = get_missing_tools(&language_type, &os);
    if !missing_tools.is_empty() {
        return Err(ChakraError::missing_tools(missing_tools));
    }

    // Create output directory if it doesn't exist
    PathResolver::ensure_output_directory(output_dir)?;

    // Use the new builder system with verbose output
    let result = build_wasm_project(project_path, output_dir, &language_type, true)
        .map_err(ChakraError::Compilation)?;

    // For legacy compatibility, return the JS path if available (for wasm-bindgen), otherwise WASM path
    // TODO: Remove legacy JS path handling in future versions
    Ok(result.js_path.unwrap_or(result.wasm_path))
}
