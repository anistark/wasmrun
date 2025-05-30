use crate::cli::CommandValidator;
use crate::compiler;
use crate::compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel};
use crate::ui::{print_compilation_success, print_compile_info, print_missing_tools};

/// Handle compile command
pub fn handle_compile_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    output: &Option<String>,
    verbose: bool,
    optimization: &str,
) -> Result<(), String> {
    let (project_path, output_dir) =
        CommandValidator::validate_compile_args(path, positional_path, output)?;

    // Parse optimization level
    let optimization_level = match optimization.to_lowercase().as_str() {
        "debug" => OptimizationLevel::Debug,
        "release" => OptimizationLevel::Release,
        "size" => OptimizationLevel::Size,
        _ => {
            return Err(format!(
                "Invalid optimization level '{}'. Valid options: debug, release, size",
                optimization
            ))
        }
    };

    // Detect project language and get system info
    let language = compiler::detect_project_language(&project_path);

    if verbose {
        compiler::print_system_info();
    }

    print_compile_info(
        &project_path,
        &language,
        &output_dir,
        &optimization_level,
        verbose,
    );

    // Check for missing tools
    let builder = BuilderFactory::create_builder(&language);
    let missing_tools = builder.check_dependencies();

    if !missing_tools.is_empty() {
        print_missing_tools(&missing_tools);
        return Err("Missing required tools for compilation".to_string());
    }

    // Create build configuration
    let config = BuildConfig {
        project_path,
        output_dir: output_dir.clone(),
        verbose,
        optimization_level,
        target_type: compiler::builder::TargetType::Standard,
    };

    // Compile WASM
    let result = if verbose {
        builder.build_verbose(&config)?
    } else {
        builder.build(&config)?
    };

    print_compilation_success(&result.wasm_path, &result.js_path, &result.additional_files);
    Ok(())
}
