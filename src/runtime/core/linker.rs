/// WASM module linker for imports and exports
///
/// Host functions receive linear memory access so WASI syscalls can
/// read pointers and write results back into the module's address space.
use super::memory::LinearMemory;
use super::values::Value;
use std::collections::HashMap;

pub trait HostFunction: Send + Sync {
    fn call(&self, args: Vec<Value>, memory: &mut LinearMemory) -> Result<Vec<Value>, String>;
    fn signature(&self) -> (usize, usize);
}

pub struct ClosureHostFunction<F>
where
    F: Fn(Vec<Value>, &mut LinearMemory) -> Result<Vec<Value>, String> + Send + Sync,
{
    func: F,
    params: usize,
    results: usize,
}

impl<F> ClosureHostFunction<F>
where
    F: Fn(Vec<Value>, &mut LinearMemory) -> Result<Vec<Value>, String> + Send + Sync,
{
    pub fn new(func: F, params: usize, results: usize) -> Self {
        ClosureHostFunction {
            func,
            params,
            results,
        }
    }
}

impl<F> HostFunction for ClosureHostFunction<F>
where
    F: Fn(Vec<Value>, &mut LinearMemory) -> Result<Vec<Value>, String> + Send + Sync,
{
    fn call(&self, args: Vec<Value>, memory: &mut LinearMemory) -> Result<Vec<Value>, String> {
        (self.func)(args, memory)
    }

    fn signature(&self) -> (usize, usize) {
        (self.params, self.results)
    }
}

pub struct Linker {
    host_functions: HashMap<String, Box<dyn HostFunction>>,
}

impl Linker {
    pub fn new() -> Self {
        Linker {
            host_functions: HashMap::new(),
        }
    }

    /// Register a host function keyed by `"module::name"`.
    pub fn register(&mut self, module: &str, name: &str, func: Box<dyn HostFunction>) {
        let key = format!("{module}::{name}");
        self.host_functions.insert(key, func);
    }

    /// Look up a host function by WASM import module and name.
    pub fn get_import(&self, module: &str, name: &str) -> Option<&dyn HostFunction> {
        let key = format!("{module}::{name}");
        self.host_functions.get(&key).map(|b| b.as_ref())
    }

    pub fn has_import(&self, module: &str, name: &str) -> bool {
        let key = format!("{module}::{name}");
        self.host_functions.contains_key(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_function_with_memory() {
        let mut memory = LinearMemory::new(1, None).unwrap();
        memory.write_i32(100, 42).unwrap();

        let func = ClosureHostFunction::new(
            |_args, mem: &mut LinearMemory| {
                let val = mem.read_i32(100)?;
                Ok(vec![Value::I32(val)])
            },
            0,
            1,
        );

        let result = func.call(vec![], &mut memory).unwrap();
        assert_eq!(result[0], Value::I32(42));
    }

    #[test]
    fn test_host_function_writes_memory() {
        let mut memory = LinearMemory::new(1, None).unwrap();

        let func = ClosureHostFunction::new(
            |args, mem: &mut LinearMemory| {
                if let Value::I32(addr) = args[0] {
                    mem.write_i32(addr as usize, 0xDEAD)?;
                }
                Ok(vec![])
            },
            1,
            0,
        );

        func.call(vec![Value::I32(200)], &mut memory).unwrap();
        assert_eq!(memory.read_i32(200).unwrap(), 0xDEAD);
    }

    #[test]
    fn test_linker_register_and_lookup() {
        let mut linker = Linker::new();
        let mut memory = LinearMemory::new(1, None).unwrap();

        linker.register(
            "env",
            "add",
            Box::new(ClosureHostFunction::new(
                |args, _mem: &mut LinearMemory| match (&args[0], &args[1]) {
                    (Value::I32(a), Value::I32(b)) => Ok(vec![Value::I32(a + b)]),
                    _ => Err("type error".into()),
                },
                2,
                1,
            )),
        );

        assert!(linker.has_import("env", "add"));
        assert!(!linker.has_import("env", "sub"));

        let f = linker.get_import("env", "add").unwrap();
        let r = f
            .call(vec![Value::I32(3), Value::I32(4)], &mut memory)
            .unwrap();
        assert_eq!(r[0], Value::I32(7));
    }

    #[test]
    fn test_linker_multiple_modules() {
        let mut linker = Linker::new();

        linker.register(
            "wasi_snapshot_preview1",
            "fd_write",
            Box::new(ClosureHostFunction::new(
                |_args, _mem: &mut LinearMemory| Ok(vec![Value::I32(0)]),
                4,
                1,
            )),
        );

        linker.register(
            "env",
            "log",
            Box::new(ClosureHostFunction::new(
                |_args, _mem: &mut LinearMemory| Ok(vec![]),
                1,
                0,
            )),
        );

        assert!(linker.has_import("wasi_snapshot_preview1", "fd_write"));
        assert!(linker.has_import("env", "log"));
        assert!(!linker.has_import("wasi_snapshot_preview1", "log"));
    }
}
