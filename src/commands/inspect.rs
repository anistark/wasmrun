use crate::cli::CommandValidator;
use crate::error::{ChakraError, Result};
use crate::utils::PathResolver;
use crate::verify;

/// Handle inspect command
pub fn handle_inspect_command(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<()> {
    let resolved_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
    println!("Resolved path: {:?}", resolved_path);

    // Validate using CommandValidator for consistency
    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    PathResolver::validate_wasm_file(&wasm_path)?;

    println!("🔍 Inspecting WebAssembly file: {}", wasm_path);

    verify::print_detailed_binary_info(&wasm_path).map_err(ChakraError::from)?;

    println!("Inspection completed successfully.");
    Ok(())
}
