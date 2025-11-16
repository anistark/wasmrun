/// WASM module linker for imports and exports
use super::module::Module;

pub struct Linker {
    // TODO: host_functions: HashMap<String, Box<dyn HostFunction>>,
}

impl Linker {
    pub fn new() -> Self {
        Linker {
            // TODO: host_functions: HashMap::new(),
        }
    }

    pub fn link_imports(&self, _module: &mut Module) -> Result<(), String> {
        // TODO: Implement import linking
        Err("Not yet implemented".to_string())
    }

    pub fn get_exported_function(&self, _name: &str) -> Result<(), String> {
        // TODO: Implement export resolution
        Err("Not yet implemented".to_string())
    }
}
