//! Compilation command implementation

use crate::compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel, TargetType};
use crate::compiler::{detect_operating_system, detect_project_language, get_missing_tools};
use crate::error::{Result, WasmrunError};
use crate::plugin::manager::PluginManager;
use crate::utils::PathResolver;
use std::path::Path;

pub fn handle_compile_command(
    project_path: String,
    output_dir: String,
    optimization_level: OptimizationLevel,
    verbose: bool,
) -> Result<()> {
    run_compile(project_path, output_dir, optimization_level, verbose)
}

pub fn run_compile(
    project_path: String,
    output_dir: String,
    optimization_level: OptimizationLevel,
    verbose: bool,
) -> Result<()> {
    PathResolver::validate_directory_exists(&project_path)?;
    PathResolver::ensure_output_directory(&output_dir)?;

    if verbose {
        println!("🔍 Detecting project type...");
    }

    // Try plugin-based compilation first
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(&project_path) {
            if verbose {
                println!(
                    "🔌 Using plugin: {} v{}",
                    plugin.info().name,
                    plugin.info().version
                );
                println!("📝 Description: {}", plugin.info().description);
            }

            // Check plugin dependencies
            let builder = plugin.get_builder();
            let missing_deps = builder.check_dependencies();
            if !missing_deps.is_empty() {
                return Err(WasmrunError::from(format!(
                    "Missing dependencies for {}: {}",
                    plugin.info().name,
                    missing_deps.join(", ")
                )));
            }

            let config = BuildConfig {
                project_path,
                output_dir,
                verbose,
                optimization_level,
                watch: false,
                target_type: TargetType::Standard,
            };

            let result = if verbose {
                builder
                    .build_verbose(&config)
                    .map_err(WasmrunError::Compilation)?
            } else {
                builder.build(&config).map_err(WasmrunError::Compilation)?
            };

            print_compilation_success(&result.wasm_path, &result.js_path, &result.additional_files);
            return Ok(());
        }
    }

    // Fall back to legacy language detection
    if verbose {
        println!("🔄 No plugin found, using legacy detection...");
    }

    let language = detect_project_language(&project_path);
    let os = detect_operating_system();

    let missing_tools = get_missing_tools(&language, &os);
    if !missing_tools.is_empty() {
        return Err(WasmrunError::missing_tools(missing_tools));
    }

    if verbose {
        println!("🎯 Language: {language:?}");
        println!("💻 OS: {os:?}");
    }

    let builder = BuilderFactory::create_builder(&language);

    let config = BuildConfig {
        project_path,
        output_dir,
        verbose,
        optimization_level,
        watch: false,
        target_type: TargetType::Standard,
    };

    let result = if verbose {
        builder
            .build_verbose(&config)
            .map_err(WasmrunError::Compilation)?
    } else {
        builder.build(&config).map_err(WasmrunError::Compilation)?
    };

    print_compilation_success(&result.wasm_path, &result.js_path, &result.additional_files);
    Ok(())
}

fn print_compilation_success(
    wasm_path: &str,
    js_path: &Option<String>,
    additional_files: &[String],
) {
    println!("✅ Compilation successful!");

    if Path::new(wasm_path).is_dir() {
        println!("📁 Web app built: {wasm_path}");
        if let Some(js_path) = js_path {
            println!("🌐 Entry point: {js_path}");
        }
    } else {
        println!("📄 WASM file: {wasm_path}");
        if let Some(js_path) = js_path {
            println!("📄 JS file: {js_path}");
        }
    }

    if !additional_files.is_empty() {
        println!("📎 Additional files:");
        for file in additional_files {
            println!("   - {file}");
        }
    }
}
