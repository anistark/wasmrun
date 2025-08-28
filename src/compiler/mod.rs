pub mod builder;
mod detect;

pub use builder::build_wasm_project;
pub use detect::{
    detect_operating_system, detect_project_language, get_missing_tools, print_system_info,
    ProjectLanguage,
};

use crate::error::{Result, WasmrunError};
use crate::plugin::manager::PluginManager;
use crate::utils::PathResolver;

/// Compile a WASM file from a project directory using plugin system
#[allow(dead_code)] // TODO: Future project compilation interface
pub fn create_wasm_from_project(project_path: &str, output_dir: &str) -> Result<String> {
    PathResolver::ensure_output_directory(output_dir)?;

    // Try plugin-based compilation first
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(builder) = plugin_manager.get_builder_for_project(project_path) {
            let config = builder::BuildConfig::with_defaults(
                project_path.to_string(),
                output_dir.to_string(),
            );

            let result = builder.build(&config).map_err(WasmrunError::Compilation)?;
            return Ok(result.wasm_path);
        }
    }

    // Fall back to legacy detection
    let language_type = detect_project_language(project_path);
    let result = build_wasm_project(project_path, output_dir, &language_type, false)
        .map_err(WasmrunError::Compilation)?;
    Ok(result.wasm_path)
}

/// AOT compile a project for execution using plugin system
pub fn compile_for_execution(project_path: &str, output_dir: &str) -> Result<String> {
    PathResolver::ensure_output_directory(output_dir)?;

    // Try plugin-based compilation first
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
            let builder = plugin.get_builder();

            // Check dependencies
            let missing_deps = builder.check_dependencies();
            if !missing_deps.is_empty() {
                return Err(WasmrunError::missing_tools(missing_deps));
            }

            let config = builder::BuildConfig::with_defaults(
                project_path.to_string(),
                output_dir.to_string(),
            );

            let result = builder.build(&config).map_err(WasmrunError::Compilation)?;
            return Ok(result.js_path.unwrap_or(result.wasm_path));
        }
    }

    // Fall back to legacy detection
    let language_type = detect_project_language(project_path);
    let os = detect_operating_system();

    let missing_tools = get_missing_tools(&language_type, &os);
    if !missing_tools.is_empty() {
        return Err(WasmrunError::missing_tools(missing_tools));
    }

    let result = build_wasm_project(project_path, output_dir, &language_type, true)
        .map_err(WasmrunError::Compilation)?;

    Ok(result.js_path.unwrap_or(result.wasm_path))
}
