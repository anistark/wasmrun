use crate::cli::CommandValidator;
use crate::error::{ChakraError, Result, WasmError};
use crate::utils::PathResolver;
use crate::verify;

/// Handle verify command
pub fn handle_verify_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    detailed: bool,
) -> Result<()> {
    let wasm_path = resolve_and_validate_wasm_path(path, positional_path)?;

    println!("🔍 Verifying WebAssembly file: {}", wasm_path);

    let result = verify::verify_wasm(&wasm_path)
        .map_err(|e| ChakraError::Wasm(WasmError::validation_failed(e)))?;

    verify::print_verification_results(&wasm_path, &result, detailed);

    // Return appropriate status based on verification result
    if !result.valid_magic {
        return Err(ChakraError::Wasm(WasmError::InvalidMagicBytes {
            path: wasm_path,
        }));
    }

    if result.section_count == 0 {
        return Err(ChakraError::Wasm(WasmError::validation_failed(
            "No sections found in WASM file",
        )));
    }

    Ok(())
}

/// Resolve and validate WASM file path with enhanced error handling
fn resolve_and_validate_wasm_path(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<String> {
    // First resolve the path
    let resolved_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());

    // Validate using CommandValidator for consistency
    CommandValidator::validate_verify_args(path, positional_path)?;

    // Additional validation
    PathResolver::validate_wasm_file(&resolved_path)?;

    // Check file size (warn if very large)
    match PathResolver::get_file_size_human(&resolved_path) {
        Ok(size) => {
            if let Ok(metadata) = std::fs::metadata(&resolved_path) {
                let size_bytes = metadata.len();
                if size_bytes > 100 * 1024 * 1024 {
                    // 100MB
                    println!(
                        "⚠️  Warning: Large WASM file ({}) - verification may take time",
                        size
                    );
                } else if size_bytes == 0 {
                    return Err(ChakraError::Wasm(WasmError::validation_failed(
                        "WASM file is empty",
                    )));
                }
            }
        }
        Err(_) => {
            // Continue anyway, but note that we couldn't get size info
            // TODO: Handle this later.
        }
    }

    Ok(resolved_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_and_validate_wasm_path_success() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("test.wasm");

        // Create a fake WASM file with magic bytes
        fs::write(&wasm_file, b"\0asm\x01\x00\x00\x00").unwrap();

        let result =
            resolve_and_validate_wasm_path(&Some(wasm_file.to_str().unwrap().to_string()), &None);

        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_and_validate_wasm_path_invalid_extension() {
        let temp_dir = tempdir().unwrap();
        let js_file = temp_dir.path().join("test.js");

        fs::write(&js_file, "console.log('test')").unwrap();

        let result =
            resolve_and_validate_wasm_path(&Some(js_file.to_str().unwrap().to_string()), &None);

        assert!(result.is_err());
        match result.unwrap_err() {
            ChakraError::InvalidFileFormat { .. } => {}
            _ => panic!("Expected InvalidFileFormat error"),
        }
    }

    #[test]
    fn test_resolve_and_validate_wasm_path_empty_file() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("empty.wasm");

        // Create empty file
        fs::write(&wasm_file, b"").unwrap();

        let result =
            resolve_and_validate_wasm_path(&Some(wasm_file.to_str().unwrap().to_string()), &None);

        assert!(result.is_err());
        match result.unwrap_err() {
            ChakraError::Wasm(WasmError::ValidationFailed { .. }) => {}
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_resolve_and_validate_wasm_path_file_not_found() {
        let result =
            resolve_and_validate_wasm_path(&Some("/nonexistent/file.wasm".to_string()), &None);

        assert!(result.is_err());
        match result.unwrap_err() {
            ChakraError::FileNotFound { .. } => {}
            _ => panic!("Expected FileNotFound error"),
        }
    }
}
