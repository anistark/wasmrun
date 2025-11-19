/// Native WASM executor for running WASM files directly
use super::executor::Executor;
use super::module::Module;
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

/// Execute WASM bytecode from memory
pub fn execute_wasm_bytes(wasm_bytes: &[u8]) -> Result<i32> {
    let module = Module::parse(wasm_bytes)
        .map_err(|e| WasmrunError::from(format!("Failed to parse WASM module: {e}")))?;

    let mut executor = Executor::new(module)
        .map_err(|e| WasmrunError::from(format!("Failed to initialize executor: {e}")))?;

    let wasi_env = Arc::new(Mutex::new(WasiEnv::new()));
    let _wasi_linker = create_wasi_linker(wasi_env);

    let start_func = executor.module().start;

    if let Some(func_idx) = start_func {
        execute_function(&mut executor, func_idx)?;
    } else if let Some((_, func_idx)) = find_export_function(executor.module(), "main") {
        execute_function(&mut executor, func_idx)?;
    } else if let Some((_, func_idx)) = find_export_function(executor.module(), "_start") {
        execute_function(&mut executor, func_idx)?;
    }

    Ok(0)
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

/// Execute a specific function
fn execute_function(executor: &mut Executor, func_idx: u32) -> Result<()> {
    executor.execute(func_idx).map_err(|e| {
        WasmrunError::from(format!(
            "Error executing WASM function (index {func_idx}): {e}"
        ))
    })?;

    Ok(())
}
