use crate::cli::CommandValidator;
use crate::utils::PathResolver;
use crate::verify;

/// Handle inspect command
pub fn handle_inspect_command(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<(), String> {
    // TODO: Remove debug logs or add to a debug mode.
    eprintln!("Debug - path flag: {:?}", path);
    eprintln!("Debug - positional_path: {:?}", positional_path);

    // Test the path resolution directly
    let resolved = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
    eprintln!("Debug - resolved path: {}", resolved);

    // Check if the resolved path exists
    if std::path::Path::new(&resolved).exists() {
        eprintln!("Debug - path exists: true");
    } else {
        eprintln!("Debug - path exists: false");
    }

    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    println!("🔍 Inspecting WebAssembly file: {}", wasm_path);

    match verify::print_detailed_binary_info(&wasm_path) {
        Ok(()) => {
            println!("Inspection completed successfully.");
            Ok(())
        }
        Err(e) => Err(format!("Inspection failed: {}", e)),
    }
}
