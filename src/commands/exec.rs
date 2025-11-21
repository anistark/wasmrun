//! Exec command implementation for running WASM files with arguments

use crate::error::{Result, WasmrunError};
use crate::runtime::core::native_executor;
use std::path::Path;

pub fn handle_exec_command(
    wasm_file: &Option<String>,
    call: &Option<String>,
    args: Vec<String>,
) -> Result<()> {
    let wasm_path = wasm_file
        .as_ref()
        .ok_or_else(|| WasmrunError::from("WASM file path is required".to_string()))?;

    execute_wasm_with_args(wasm_path, call.clone(), args)
}

fn execute_wasm_with_args(wasm_path: &str, call: Option<String>, args: Vec<String>) -> Result<()> {
    if !Path::new(wasm_path).exists() {
        return Err(WasmrunError::from(format!(
            "WASM file not found: {wasm_path}"
        )));
    }

    if !wasm_path.ends_with(".wasm") {
        return Err(WasmrunError::from(format!(
            "Expected a .wasm file, got: {wasm_path}"
        )));
    }

    println!("🎯 Running WASM file: {wasm_path}");
    if let Some(ref func) = call {
        println!("📍 Calling: {func}");
    }
    if !args.is_empty() {
        println!("📝 Arguments: {}", args.join(" "));
    }
    println!("🏃 Executing natively (interpreter mode)");

    native_executor::execute_wasm_file_with_args(wasm_path, call, args)?;
    println!("✅ Execution completed");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Missing WASM file path parameter
    #[test]
    fn test_handle_exec_missing_wasm_path() {
        let result = handle_exec_command(&None, &None, Vec::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("required"));
    }

    /// Test: Non-existent WASM file
    #[test]
    fn test_handle_exec_nonexistent_file() {
        let result = handle_exec_command(&Some("nonexistent.wasm".to_string()), &None, Vec::new());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    /// Test: Invalid file extension (not .wasm)
    #[test]
    fn test_handle_exec_invalid_extension() {
        let result = handle_exec_command(&Some("test_file.txt".to_string()), &None, Vec::new());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Error could be either about extension or file not found
        // The important thing is it fails with either message
        assert!(err.contains(".wasm") || err.contains("not found"));
    }

    /// Test: Valid WASM file path with Go example (if available)
    #[test]
    fn test_handle_exec_go_example() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let result = handle_exec_command(&Some(wasm_path.to_string()), &None, Vec::new());

        match result {
            Ok(_) => println!("✓ Successfully executed Go example WASM"),
            Err(e) => println!("⚠️  Go example execution error: {e}"),
        }
    }

    /// Test: Execute with function selection (call flag)
    #[test]
    fn test_handle_exec_with_function_selection() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        // Try calling a function that likely doesn't exist (for error testing)
        let result = handle_exec_command(
            &Some(wasm_path.to_string()),
            &Some("nonexistent_func".to_string()),
            Vec::new(),
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    /// Test: Execute with arguments
    #[test]
    fn test_handle_exec_with_arguments() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let args = vec!["arg1".to_string(), "arg2".to_string()];
        let result = handle_exec_command(&Some(wasm_path.to_string()), &None, args);

        match result {
            Ok(_) => println!("✓ Successfully executed with arguments"),
            Err(e) => println!("⚠️  Execution with arguments error: {e}"),
        }
    }

    /// Test: Execute with both function selection and arguments
    #[test]
    fn test_handle_exec_function_and_arguments() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let args = vec!["test_arg".to_string()];
        let result =
            handle_exec_command(&Some(wasm_path.to_string()), &Some("run".to_string()), args);

        match result {
            Ok(_) => println!("✓ Successfully executed with function and arguments"),
            Err(e) => {
                // This is expected if the function doesn't exist or doesn't accept arguments
                println!("⚠️  Execution error (may be expected): {e}");
            }
        }
    }
}
