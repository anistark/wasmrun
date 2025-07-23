use crate::compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel, TargetType};
use crate::error::{Result, WasmrunError};
use crate::ui::{print_compilation_success, print_compile_info};
use crate::utils::PathResolver;

/// Handle compile command
pub fn handle_compile_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    output: &Option<String>,
    verbose: bool,
    optimization: &Option<String>,
) -> Result<()> {
    let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
    PathResolver::validate_directory_exists(&project_path)?;

    let output_dir = match output {
        Some(dir) => dir.clone(),
        None => {
            let default_output = PathResolver::join_paths(&project_path, "target/wasm");
            std::fs::create_dir_all(&default_output)?;
            default_output
        }
    };

    let optimization_level = match optimization.as_deref() {
        Some("debug") => OptimizationLevel::Debug,
        Some("release") => OptimizationLevel::Release,
        Some("size") => OptimizationLevel::Size,
        _ => OptimizationLevel::Release,
    };

    let language = crate::compiler::detect_project_language(&project_path);
    print_compile_info(
        &project_path,
        &language,
        &output_dir,
        &optimization_level,
        verbose,
    );

    if let Ok(plugin_manager) = crate::plugin::PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(&project_path) {
            println!(
                "🔌 Using plugin: {} v{}",
                plugin.info().name,
                plugin.info().version
            );

            let builder = plugin.get_builder();
            let missing_deps = builder.check_dependencies();

            if !missing_deps.is_empty() {
                println!("❌ Missing dependencies:");
                for dep in missing_deps {
                    println!("   • {}", dep);
                }
                return Err(WasmrunError::from("Missing plugin dependencies"));
            }

            let config = BuildConfig {
                project_path: project_path.clone(),
                output_dir: output_dir.clone(),
                verbose,
                optimization_level,
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

    let builder = BuilderFactory::create_builder(&language);

    let config = BuildConfig {
        project_path,
        output_dir,
        verbose,
        optimization_level,
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
