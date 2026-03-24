/// Native WASM executor for running WASM files directly
use super::executor::{Executor, WASI_PROC_EXIT_PREFIX};
use super::module::Module;
use super::values::Value;
use crate::error::{Result, WasmrunError};
use crate::runtime::wasi::{create_wasi_linker, WasiEnv};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

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

pub fn execute_wasm_bytes(wasm_bytes: &[u8]) -> Result<i32> {
    execute_wasm_bytes_with_args(wasm_bytes, None, Vec::new())
}

pub fn execute_wasm_bytes_with_args(
    wasm_bytes: &[u8],
    function: Option<String>,
    args: Vec<String>,
) -> Result<i32> {
    let module = Module::parse(wasm_bytes)
        .map_err(|e| WasmrunError::from(format!("Failed to parse WASM module: {e}")))?;

    let wasi_env = Arc::new(Mutex::new(WasiEnv::new().with_args(args.clone())));
    let wasi_linker = create_wasi_linker(wasi_env.clone());

    let mut executor = Executor::new_with_linker(module, wasi_linker)
        .map_err(|e| WasmrunError::from(format!("Failed to initialize executor: {e}")))?;

    let func_idx = if let Some(func_name) = function {
        find_export_function(executor.module(), &func_name)
            .map(|(_, idx)| idx)
            .ok_or_else(|| {
                WasmrunError::from(format!(
                    "Exported function '{func_name}' not found in WASM module"
                ))
            })?
    } else {
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

    let wasm_args = convert_string_args_to_values(&args);

    match execute_function(&mut executor, func_idx, wasm_args) {
        Ok(()) => {
            // Print captured stdout
            if let Ok(env) = wasi_env.lock() {
                let out = env.get_stdout();
                if !out.is_empty() {
                    print!("{}", String::from_utf8_lossy(&out));
                }
                let err_out = env.get_stderr();
                if !err_out.is_empty() {
                    eprint!("{}", String::from_utf8_lossy(&err_out));
                }
            }
            Ok(0)
        }
        Err(e) => {
            let err_str = e.to_string();
            if let Some(code) = Executor::is_proc_exit(&err_str) {
                // Print captured output even on proc_exit
                if let Ok(env) = wasi_env.lock() {
                    let out = env.get_stdout();
                    if !out.is_empty() {
                        print!("{}", String::from_utf8_lossy(&out));
                    }
                    let err_out = env.get_stderr();
                    if !err_out.is_empty() {
                        eprint!("{}", String::from_utf8_lossy(&err_out));
                    }
                }
                Ok(code)
            } else {
                Err(e)
            }
        }
    }
}

fn convert_string_args_to_values(args: &[String]) -> Vec<Value> {
    args.iter()
        .map(|arg| {
            if let Ok(v) = arg.parse::<i32>() {
                return Value::I32(v);
            }
            if let Ok(v) = arg.parse::<i64>() {
                return Value::I64(v);
            }
            Value::I32(0)
        })
        .collect()
}

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

fn execute_function(executor: &mut Executor, func_idx: u32, args: Vec<Value>) -> Result<()> {
    executor.execute_with_args(func_idx, args).map_err(|e| {
        // Propagate proc_exit as-is so the caller can detect it
        if e.starts_with(WASI_PROC_EXIT_PREFIX) {
            WasmrunError::from(e)
        } else {
            WasmrunError::from(format!(
                "Error executing WASM function (index {func_idx}): {e}"
            ))
        }
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

    #[test]
    fn test_execute_nonexistent_wasm_file() {
        let result = execute_wasm_file("nonexistent.wasm");
        assert!(result.is_err());
    }

    #[test]
    fn test_minimal_wasm_no_entry_point() {
        let minimal_wasm = vec![
            0x00, 0x61, 0x73, 0x6d, // magic
            0x01, 0x00, 0x00, 0x00, // version
        ];
        let result = execute_wasm_bytes(&minimal_wasm);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_wasm_file() {
        if !wasm_file_exists(GO_WASM_PATH) {
            return;
        }
        match execute_wasm_file(GO_WASM_PATH) {
            Ok(code) => println!("✓ exit code: {code}"),
            Err(e) => println!("⚠️  {e}"),
        }
    }

    #[test]
    fn test_execute_wasm_file_with_args() {
        if !wasm_file_exists(GO_WASM_PATH) {
            return;
        }
        let args = vec!["test_arg1".to_string(), "test_arg2".to_string()];
        match execute_wasm_file_with_args(GO_WASM_PATH, None, args) {
            Ok(code) => println!("✓ exit code: {code}"),
            Err(e) => println!("⚠️  {e}"),
        }
    }

    #[test]
    fn test_function_selection_nonexistent() {
        if !wasm_file_exists(GO_WASM_PATH) {
            return;
        }
        let wasm_bytes = fs::read(GO_WASM_PATH).unwrap();
        let result = execute_wasm_bytes_with_args(
            &wasm_bytes,
            Some("nonexistent_function".to_string()),
            Vec::new(),
        );
        assert!(result.is_err());
    }

    /// End-to-end test: hand-built WASM that calls fd_write to print "Hello, World!\n"
    #[test]
    fn test_hello_world_wasi_program() {
        // This WASM module:
        //   imports wasi_snapshot_preview1::fd_write
        //   has data segment "Hello, World!\n" at offset 16
        //   _start: stores iovec {ptr=16, len=14} at offset 0
        //           calls fd_write(1, 0, 1, 8) → writes to stdout
        #[rustfmt::skip]
        let wasm: Vec<u8> = vec![
            // Header
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            // Type section (id=1): 2 types
            0x01, 0x0c, // section id=1, size=12
            0x02,       // 2 types
            // type 0: (i32,i32,i32,i32)->i32
            0x60, 0x04, 0x7f, 0x7f, 0x7f, 0x7f, 0x01, 0x7f,
            // type 1: ()->()
            0x60, 0x00, 0x00,
            // Import section (id=2): 1 import
            0x02, 0x23, // section id=2, size=35
            0x01,       // 1 import
            0x16,       // module name len=22
            // "wasi_snapshot_preview1"
            0x77, 0x61, 0x73, 0x69, 0x5f, 0x73, 0x6e, 0x61,
            0x70, 0x73, 0x68, 0x6f, 0x74, 0x5f, 0x70, 0x72,
            0x65, 0x76, 0x69, 0x65, 0x77, 0x31,
            0x08,       // name len=8
            // "fd_write"
            0x66, 0x64, 0x5f, 0x77, 0x72, 0x69, 0x74, 0x65,
            0x00,       // function import
            0x00,       // type index 0
            // Function section (id=3): 1 function
            0x03, 0x02, 0x01, 0x01, // 1 func, type index 1
            // Memory section (id=5): 1 memory, 1 page
            0x05, 0x03, 0x01, 0x00, 0x01,
            // Export section (id=7): 2 exports
            0x07, 0x13, // size=19
            0x02,       // 2 exports
            0x06,       // "memory" (6 bytes)
            0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79,
            0x02, 0x00, // memory, index 0
            0x06,       // "_start" (6 bytes)
            0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
            0x00, 0x01, // function, index 1 (abs: import_count=1 + defined 0)
            // Code section (id=10)
            0x0a, 0x1d, // size=29
            0x01,       // 1 body
            0x1b,       // body size=27
            0x00,       // 0 local declarations
            // i32.const 0; i32.const 16; i32.store align=2 offset=0
            0x41, 0x00, 0x41, 0x10, 0x36, 0x02, 0x00,
            // i32.const 4; i32.const 14; i32.store align=2 offset=0
            0x41, 0x04, 0x41, 0x0e, 0x36, 0x02, 0x00,
            // call fd_write(1, 0, 1, 8)
            0x41, 0x01, // fd=1 (stdout)
            0x41, 0x00, // iovs=0
            0x41, 0x01, // iovs_len=1
            0x41, 0x08, // nwritten=8
            0x10, 0x00, // call func 0 (fd_write import)
            0x1a,       // drop
            0x0b,       // end
            // Data section (id=11)
            0x0b, 0x14, // size=20
            0x01,       // 1 segment
            0x00,       // active, memory 0
            0x41, 0x10, 0x0b, // i32.const 16, end
            0x0e,       // 14 bytes
            // "Hello, World!\n"
            0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20,
            0x57, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x0a,
        ];

        let module = Module::parse(&wasm).expect("Failed to parse test WASM");
        assert_eq!(module.imports.len(), 1);
        assert_eq!(module.functions.len(), 1);

        let wasi_env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(wasi_env.clone());
        let mut executor =
            Executor::new_with_linker(module, linker).expect("Failed to create executor");

        // _start is export index 1 (abs func index 1 = defined func 0)
        let result = executor.execute_with_args(1, vec![]);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        let stdout = wasi_env.lock().unwrap().get_stdout();
        assert_eq!(
            String::from_utf8_lossy(&stdout),
            "Hello, World!\n",
            "Captured stdout mismatch"
        );
    }

    /// Test proc_exit terminates cleanly and returns exit code
    #[test]
    fn test_proc_exit_handling() {
        // WASM module that imports proc_exit and calls it with code 42
        #[rustfmt::skip]
        let wasm: Vec<u8> = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            // Type section: 2 types (size=8)
            0x01, 0x08, 0x02,
            0x60, 0x01, 0x7f, 0x00, // type 0: (i32)->()
            0x60, 0x00, 0x00,       // type 1: ()->()
            // Import section: proc_exit (size=36)
            0x02, 0x24, 0x01,
            0x16, // module name len=22
            0x77, 0x61, 0x73, 0x69, 0x5f, 0x73, 0x6e, 0x61,
            0x70, 0x73, 0x68, 0x6f, 0x74, 0x5f, 0x70, 0x72,
            0x65, 0x76, 0x69, 0x65, 0x77, 0x31,
            0x09, // name len=9
            0x70, 0x72, 0x6f, 0x63, 0x5f, 0x65, 0x78, 0x69, 0x74,
            0x00, 0x00, // function, type 0
            // Function section
            0x03, 0x02, 0x01, 0x01,
            // Memory section
            0x05, 0x03, 0x01, 0x00, 0x01,
            // Export section: _start (size=10)
            0x07, 0x0a, 0x01,
            0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
            0x00, 0x01,
            // Code section
            0x0a, 0x08, 0x01,
            0x06, 0x00,
            0x41, 0x2a, // i32.const 42
            0x10, 0x00, // call 0 (proc_exit)
            0x0b,       // end
        ];

        let module = Module::parse(&wasm).expect("parse");
        let wasi_env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(wasi_env);
        let mut executor = Executor::new_with_linker(module, linker).expect("init");

        let result = executor.execute_with_args(1, vec![]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(Executor::is_proc_exit(&err), Some(42));
    }

    /// Test that host function reads/writes match imported function dispatch
    #[test]
    fn test_import_dispatch_with_defined_function() {
        use crate::runtime::core::linker::{ClosureHostFunction, Linker};
        use crate::runtime::core::module::{
            ExportDesc, ExportKind, Function, FunctionType, ImportDesc, ImportKind, ValueType,
        };
        use std::collections::HashMap;

        let mut exports = HashMap::new();
        exports.insert(
            "run".to_string(),
            ExportDesc {
                name: "run".to_string(),
                kind: ExportKind::Function,
                index: 1,
            },
        );

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![ValueType::I32],
                results: vec![ValueType::I32],
            }],
            imports: vec![ImportDesc {
                module: "env".to_string(),
                name: "add_ten".to_string(),
                kind: ImportKind::Function(0),
            }],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                // local.get 0, call 0 (import: add_ten), end
                code: vec![0x20, 0x00, 0x10, 0x00, 0x0b],
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports,
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut linker = Linker::new();
        linker.register(
            "env",
            "add_ten",
            Box::new(ClosureHostFunction::new(
                |args, _mem| match args[0] {
                    Value::I32(v) => Ok(vec![Value::I32(v + 10)]),
                    _ => Err("expected i32".into()),
                },
                1,
                1,
            )),
        );

        let mut executor = Executor::new_with_linker(module, linker).unwrap();
        // Call "run" (abs index 1 = defined func 0), passing 5
        let results = executor.execute_with_args(1, vec![Value::I32(5)]).unwrap();
        assert_eq!(results, vec![Value::I32(15)]);
    }
}
