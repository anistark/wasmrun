// TODO: Implement Python runtime with CPython WASM

#[allow(dead_code)] // TODO: Will be used when Python runtime is fully implemented
pub struct PythonRuntime;

#[allow(dead_code)]
impl PythonRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PythonRuntime {
    fn default() -> Self {
        Self::new()
    }
}
