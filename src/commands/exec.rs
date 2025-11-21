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
