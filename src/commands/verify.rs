use crate::cli::CommandValidator;
use crate::verify;

/// Handle verify command
pub fn handle_verify_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    detailed: bool,
) -> Result<(), String> {
    eprintln!("Debug - path flag: {:?}", path);
    eprintln!("Debug - positional_path: {:?}", positional_path);

    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    println!("🔍 Verifying WebAssembly file: {}", wasm_path);

    match verify::verify_wasm(&wasm_path) {
        Ok(result) => {
            verify::print_verification_results(&wasm_path, &result, detailed);
            Ok(())
        }
        Err(e) => Err(format!("Verification failed: {}", e)),
    }
}
