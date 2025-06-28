use crate::cli::CommandValidator;
use crate::compiler;
use crate::compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel};
use crate::error::{CompilationError, Result, WasmrunError};
use crate::ui::{print_compilation_success, print_compile_info, print_missing_tools};
use std::fmt;

/// Handle compile command
pub fn handle_compile_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    output: &Option<String>,
    verbose: bool,
    optimization: &str,
) -> Result<()> {
    let (project_path, output_dir) =
        CommandValidator::validate_compile_args(path, positional_path, output)
            .map_err(|e| WasmrunError::from(e.to_string()))?;

    let optimization_level = match optimization.to_lowercase().as_str() {
        "debug" => OptimizationLevel::Debug,
        "release" => OptimizationLevel::Release,
        "size" => OptimizationLevel::Size,
        _ => {
            return Err(WasmrunError::Compilation(
                CompilationError::InvalidOptimizationLevel {
                    level: optimization.to_string(),
                    valid_options: vec![
                        "debug".to_string(),
                        "release".to_string(),
                        "size".to_string(),
                    ],
                },
            ));
        }
    };

    let language = compiler::detect_project_language(&project_path);

    if language == compiler::ProjectLanguage::Unknown {
        return Err(WasmrunError::language_detection(format!(
            "Could not detect project language in directory: {}",
            project_path
        )));
    }

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

    let builder = BuilderFactory::create_builder(&language);
    let missing_tools = builder.check_dependencies();

    if !missing_tools.is_empty() {
        print_missing_tools(&missing_tools);
        return Err(WasmrunError::missing_tools(missing_tools));
    }

    let config = BuildConfig {
        project_path,
        output_dir: output_dir.clone(),
        verbose,
        optimization_level,
        target_type: compiler::builder::TargetType::Standard,
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

impl fmt::Display for compiler::ProjectLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lang_str = match self {
            compiler::ProjectLanguage::Rust => "Rust",
            compiler::ProjectLanguage::Go => "Go",
            compiler::ProjectLanguage::C => "C",
            compiler::ProjectLanguage::Asc => "Asc",
            compiler::ProjectLanguage::Python => "Python",
            compiler::ProjectLanguage::Unknown => "Unknown",
        };
        write!(f, "{}", lang_str)
    }
}
