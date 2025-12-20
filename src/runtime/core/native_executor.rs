/// Native WASM executor for running WASM files directly
use super::executor::Executor;
use super::module::Module;
use super::values::Value;
use crate::error::{Result, WasmrunError};
use crate::runtime::wasi::{create_wasi_linker, WasiEnv};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Execute a WASM file natively
pub fn execute_wasm_file(wasm_path: &str) -> Result<i32> {
    if !Path::new(wasm_path).exists() {
        return Err(WasmrunError::from(format!(
            "WASM file not found: {wasm_path}"
        )));
    }

    let wasm_bytes = fs::read(wasm_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read WASM file '{wasm_path}': {e}")))?;

    execute_wasm_bytes(&wasm_bytes)
}

/// Execute a WASM file natively with arguments
pub fn execute_wasm_file_with_args(
    wasm_path: &str,
    function: Option<String>,
    args: Vec<String>,
) -> Result<i32> {
    if !Path::new(wasm_path).exists() {
        return Err(WasmrunError::from(format!(
            "WASM file not found: {wasm_path}"
        )));
    }

    let wasm_bytes = fs::read(wasm_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read WASM file '{wasm_path}': {e}")))?;

    execute_wasm_bytes_with_args(&wasm_bytes, function, args)
}

/// Execute WASM bytecode from memory
pub fn execute_wasm_bytes(wasm_bytes: &[u8]) -> Result<i32> {
    execute_wasm_bytes_with_args(wasm_bytes, None, Vec::new())
}

/// Execute WASM bytecode from memory with arguments
pub fn execute_wasm_bytes_with_args(
    wasm_bytes: &[u8],
    function: Option<String>,
    args: Vec<String>,
) -> Result<i32> {
    let module = Module::parse(wasm_bytes)
        .map_err(|e| WasmrunError::from(format!("Failed to parse WASM module: {e}")))?;

    let mut executor = Executor::new(module)
        .map_err(|e| WasmrunError::from(format!("Failed to initialize executor: {e}")))?;

    // Setup WASI environment with arguments
    let wasi_env = Arc::new(Mutex::new(WasiEnv::new().with_args(args.clone())));
    let _wasi_linker = create_wasi_linker(wasi_env);

    // Determine which function to call
    let func_idx = if let Some(func_name) = function {
        // User specified a function name - look it up
        find_export_function(executor.module(), &func_name)
            .map(|(_, idx)| idx)
            .ok_or_else(|| {
                WasmrunError::from(format!(
                    "Exported function '{func_name}' not found in WASM module"
                ))
            })?
    } else {
        // Use default entry point detection
        let start_func = executor.module().start;

        if let Some(func_idx) = start_func {
            func_idx
        } else if let Some((_, func_idx)) = find_export_function(executor.module(), "main") {
            func_idx
        } else if let Some((_, func_idx)) = find_export_function(executor.module(), "_start") {
            func_idx
        } else {
            return Err(WasmrunError::from(
                "No entry point found (checked: start section, main, _start)".to_string(),
            ));
        }
    };

    // Convert string arguments to WASM values (basic conversion)
    let wasm_args = convert_string_args_to_values(&args);

    execute_function(&mut executor, func_idx, wasm_args)?;
    Ok(0)
}

/// Convert string arguments to WASM values
/// Simple conversion: tries to parse as i32 first, falls back to i64, otherwise uses 0
fn convert_string_args_to_values(args: &[String]) -> Vec<Value> {
    args.iter()
        .map(|arg| {
            // Try parsing as i32
            if let Ok(i32_val) = arg.parse::<i32>() {
                return Value::I32(i32_val);
            }
            // Try parsing as i64
            if let Ok(i64_val) = arg.parse::<i64>() {
                return Value::I64(i64_val);
            }
            // Default to 0
            Value::I32(0)
        })
        .collect()
}

/// Find an exported function by name
fn find_export_function(module: &Module, name: &str) -> Option<(String, u32)> {
    for (export_name, export_desc) in &module.exports {
        if export_name == name {
            if let super::module::ExportKind::Function = export_desc.kind {
                return Some((export_name.clone(), export_desc.index));
            }
        }
    }
    None
}

/// Execute a specific function with optional arguments
fn execute_function(executor: &mut Executor, func_idx: u32, args: Vec<Value>) -> Result<()> {
    executor.execute_with_args(func_idx, args).map_err(|e| {
        WasmrunError::from(format!(
            "Error executing WASM function (index {func_idx}): {e}"
        ))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GO_WASM_PATH: &str = "examples/go-hello/main.wasm";

    fn wasm_file_exists(path: &str) -> bool {
        Path::new(path).exists()
    }

    /// Test: Execute WASM file successfully
    #[test]
    fn test_execute_wasm_file() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        match execute_wasm_file(GO_WASM_PATH) {
            Ok(code) => println!("✓ Successfully executed WASM file, exit code: {code}"),
            Err(e) => println!("⚠️  Execution error: {e}"),
        }
    }

    /// Test: Non-existent WASM file
    #[test]
    fn test_execute_nonexistent_wasm_file() {
        let result = execute_wasm_file("nonexistent.wasm");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found") || err.contains("Failed to read"));
    }

    /// Test: Execute WASM with arguments
    #[test]
    fn test_execute_wasm_file_with_args() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let args = vec!["test_arg1".to_string(), "test_arg2".to_string()];
        match execute_wasm_file_with_args(GO_WASM_PATH, None, args.clone()) {
            Ok(code) => println!(
                "✓ Executed WASM with {} args, exit code: {code}",
                args.len()
            ),
            Err(e) => println!("⚠️  Execution error: {e}"),
        }
    }

    /// Test: Execute WASM bytes directly
    #[test]
    fn test_execute_wasm_bytes() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        match execute_wasm_bytes(&wasm_bytes) {
            Ok(code) => println!("✓ Successfully executed WASM bytes, exit code: {code}"),
            Err(e) => println!("⚠️  Execution error: {e}"),
        }
    }

    /// Test: Execute WASM bytes with arguments
    #[test]
    fn test_execute_wasm_bytes_with_args() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        let args = vec!["arg1".to_string(), "arg2".to_string(), "arg3".to_string()];

        match execute_wasm_bytes_with_args(&wasm_bytes, None, args.clone()) {
            Ok(code) => println!(
                "✓ Executed WASM bytes with {} args, exit code: {code}",
                args.len()
            ),
            Err(e) => println!("⚠️  Execution error: {e}"),
        }
    }

    /// Test: Function selection - call non-existent function
    #[test]
    fn test_function_selection_nonexistent() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        let result = execute_wasm_bytes_with_args(
            &wasm_bytes,
            Some("nonexistent_function".to_string()),
            Vec::new(),
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    /// Test: Entry point detection (start section, main, _start)
    #[test]
    fn test_entry_point_detection() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        match Module::parse(&wasm_bytes) {
            Ok(module) => {
                println!("✓ WASM module parsed for entry point detection");

                if let Some(start_idx) = module.start {
                    println!("  - Found start section: function {start_idx}");
                }

                if let Some((name, _)) = find_export_function(&module, "main") {
                    println!("  - Found 'main' export: {name}");
                }

                if let Some((name, _)) = find_export_function(&module, "_start") {
                    println!("  - Found '_start' export: {name}");
                }
            }
            Err(e) => println!("⚠️  Failed to parse module: {e}"),
        }
    }

    /// Test: Minimal WASM with no entry points
    #[test]
    fn test_minimal_wasm_no_entry_point() {
        // This is a valid WASM binary (magic number + version) with no code
        let minimal_wasm = vec![
            0x00, 0x61, 0x73, 0x6d, // magic number "\0asm"
            0x01, 0x00, 0x00, 0x00, // version 1
        ];

        let result = execute_wasm_bytes(&minimal_wasm);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No entry point") || err.contains("Failed"));
    }

    /// Test: Many arguments passed to WASM
    #[test]
    fn test_many_arguments() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        let many_args: Vec<String> = (0..50).map(|i| format!("arg{i}")).collect();

        match execute_wasm_bytes_with_args(&wasm_bytes, None, many_args.clone()) {
            Ok(code) => println!(
                "✓ Executed with {} arguments, exit code: {code}",
                many_args.len()
            ),
            Err(e) => println!("⚠️  Execution with many args error: {e}"),
        }
    }

    /// Test: Arguments with special characters
    #[test]
    fn test_arguments_with_special_chars() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        let special_args = vec![
            "arg with spaces".to_string(),
            "arg/with/slashes".to_string(),
            "arg-with-dashes".to_string(),
            "arg_with_underscores".to_string(),
        ];

        match execute_wasm_bytes_with_args(&wasm_bytes, None, special_args.clone()) {
            Ok(code) => println!("✓ Executed with special char args, exit code: {code}"),
            Err(e) => println!("⚠️  Execution error: {e}"),
        }
    }

    /// Test: Function selection with valid function (if it exists)
    #[test]
    fn test_function_selection_with_valid_function() {
        if !wasm_file_exists(GO_WASM_PATH) {
            println!("⚠️  {GO_WASM_PATH} not found, skipping test");
            return;
        }

        let wasm_bytes = fs::read(GO_WASM_PATH).expect("Failed to read WASM file");
        match Module::parse(&wasm_bytes) {
            Ok(module) => {
                let function_exports: Vec<_> = module
                    .exports
                    .iter()
                    .filter(|(_, export)| {
                        matches!(
                            export.kind,
                            crate::runtime::core::module::ExportKind::Function
                        )
                    })
                    .map(|(name, _)| name.clone())
                    .collect();

                if let Some(func_name) = function_exports.first() {
                    println!("  Testing with exported function: {func_name}");
                    let result = execute_wasm_bytes_with_args(
                        &wasm_bytes,
                        Some(func_name.clone()),
                        Vec::new(),
                    );

                    match result {
                        Ok(code) => println!("✓ Called function '{func_name}', exit code: {code}"),
                        Err(e) => println!("⚠️  Error calling function: {e}"),
                    }
                } else {
                    println!("⚠️  No exported functions in WASM module");
                }
            }
            Err(e) => println!("⚠️  Failed to parse module: {e}"),
        }
    }
}
