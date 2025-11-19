/// WASM module linker for imports and exports
use super::module::Module;
use super::values::Value;
use std::collections::HashMap;

/// Trait for host functions that can be imported into WASM modules
pub trait HostFunction: Send + Sync {
    /// Execute the host function with given arguments
    /// Arguments are on the stack, results should be pushed to stack
    fn call(&self, args: Vec<Value>) -> Result<Vec<Value>, String>;

    /// Get the function signature (param count, result count)
    fn signature(&self) -> (usize, usize);
}

/// Simple host function implementation for closures
pub struct ClosureHostFunction<F>
where
    F: Fn(Vec<Value>) -> Result<Vec<Value>, String> + Send + Sync,
{
    func: F,
    params: usize,
    results: usize,
}

impl<F> ClosureHostFunction<F>
where
    F: Fn(Vec<Value>) -> Result<Vec<Value>, String> + Send + Sync,
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
    F: Fn(Vec<Value>) -> Result<Vec<Value>, String> + Send + Sync,
{
    fn call(&self, args: Vec<Value>) -> Result<Vec<Value>, String> {
        (self.func)(args)
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

    /// Register a host function with the linker
    pub fn register(&mut self, name: String, func: Box<dyn HostFunction>) {
        self.host_functions.insert(name, func);
    }

    /// Get a host function by name
    pub fn get(&self, name: &str) -> Option<&dyn HostFunction> {
        self.host_functions.get(name).map(|b| b.as_ref())
    }

    /// Check if a function is registered
    pub fn has(&self, name: &str) -> bool {
        self.host_functions.contains_key(name)
    }

    pub fn link_imports(&self, _module: &mut Module) -> Result<(), String> {
        // TODO: Implement import linking for WASI and other host functions
        Ok(())
    }

    pub fn get_exported_function(&self, _name: &str) -> Result<(), String> {
        // TODO: Implement export resolution
        Err("Export resolution not yet implemented".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_function_trait() {
        let func = ClosureHostFunction::new(
            |args| {
                if args.len() != 2 {
                    return Err("Expected 2 args".to_string());
                }
                match (&args[0], &args[1]) {
                    (Value::I32(a), Value::I32(b)) => Ok(vec![Value::I32(a + b)]),
                    _ => Err("Type mismatch".to_string()),
                }
            },
            2,
            1,
        );

        let result = func.call(vec![Value::I32(5), Value::I32(3)]).unwrap();
        assert_eq!(result[0], Value::I32(8));

        let sig = func.signature();
        assert_eq!(sig, (2, 1));
    }

    #[test]
    fn test_linker_register_get() {
        let mut linker = Linker::new();

        let func = Box::new(ClosureHostFunction::new(
            |_args| Ok(vec![Value::I32(42)]),
            0,
            1,
        ));

        linker.register("test_func".to_string(), func);

        assert!(linker.has("test_func"));
        assert!(!linker.has("other_func"));

        let retrieved = linker.get("test_func");
        assert!(retrieved.is_some());

        if let Some(f) = retrieved {
            let result = f.call(vec![]).unwrap();
            assert_eq!(result[0], Value::I32(42));
        }
    }

    #[test]
    fn test_linker_multiple_functions() {
        let mut linker = Linker::new();

        linker.register(
            "add".to_string(),
            Box::new(ClosureHostFunction::new(
                |args| match (&args[0], &args[1]) {
                    (Value::I32(a), Value::I32(b)) => Ok(vec![Value::I32(a + b)]),
                    _ => Err("Type error".to_string()),
                },
                2,
                1,
            )),
        );

        linker.register(
            "multiply".to_string(),
            Box::new(ClosureHostFunction::new(
                |args| match (&args[0], &args[1]) {
                    (Value::I32(a), Value::I32(b)) => Ok(vec![Value::I32(a * b)]),
                    _ => Err("Type error".to_string()),
                },
                2,
                1,
            )),
        );

        assert_eq!(
            linker
                .get("add")
                .unwrap()
                .call(vec![Value::I32(10), Value::I32(5)])
                .unwrap()[0],
            Value::I32(15)
        );
        assert_eq!(
            linker
                .get("multiply")
                .unwrap()
                .call(vec![Value::I32(10), Value::I32(5)])
                .unwrap()[0],
            Value::I32(50)
        );
    }
}
