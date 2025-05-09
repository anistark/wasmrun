mod detect;
mod language;

pub use detect::{
    detect_operating_system, detect_project_language, get_missing_tools, print_system_info,
    OperatingSystem, ProjectLanguage,
};

use std::fs;
use std::path::Path;

/// Compile a WASM file from a project directory
pub fn create_wasm_from_project(project_path: &str, output_dir: &str) -> Result<String, String> {
    let language_type: ProjectLanguage = detect_project_language(project_path);
    let _os: OperatingSystem = detect_operating_system();

    let output_directory = if output_dir.is_empty() {
        "."
    } else {
        output_dir
    };

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_directory);
    if !output_path.exists() {
        fs::create_dir_all(output_path)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    match language_type {
        ProjectLanguage::Rust => language::rust::build_wasm(project_path, output_directory),
        ProjectLanguage::Go => language::go::build_wasm(project_path, output_directory),
        ProjectLanguage::C => language::c::build_wasm(project_path, output_directory),
        ProjectLanguage::AssemblyScript => {
            language::asc::build_wasm(project_path, output_directory)
        }
        ProjectLanguage::Python => language::python::build_wasm(project_path, output_directory),
        ProjectLanguage::Unknown => Err(format!(
            "Could not determine project language for: {}",
            project_path
        )),
    }
}

/// AOT compile a project for execution
pub fn compile_for_execution(project_path: &str, output_dir: &str) -> Result<String, String> {
    let language_type: ProjectLanguage = detect_project_language(project_path);
    let os: OperatingSystem = detect_operating_system();

    // Check for missing tools early
    let missing_tools = get_missing_tools(&language_type, &os);
    if !missing_tools.is_empty() {
        return Err(format!(
            "Missing required tools for {:?}: {}",
            language_type,
            missing_tools.join(", ")
        ));
    }

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    // Use language-specific compiler with extra verbosity for direct execution
    match language_type {
        ProjectLanguage::Rust => language::rust::build_wasm_verbose(project_path, output_dir),
        ProjectLanguage::Go => language::go::build_wasm_verbose(project_path, output_dir),
        ProjectLanguage::C => language::c::build_wasm_verbose(project_path, output_dir),
        ProjectLanguage::AssemblyScript => {
            language::asc::build_wasm_verbose(project_path, output_dir)
        }
        ProjectLanguage::Python => language::python::build_wasm(project_path, output_dir),
        ProjectLanguage::Unknown => Err(format!(
            "Could not determine project language for: {}",
            project_path
        )),
    }
}
