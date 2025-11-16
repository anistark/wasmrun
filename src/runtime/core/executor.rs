/// WASM instruction executor
use super::module::Module;
use super::values::Value;

#[derive(Debug)]
pub struct ExecutionContext {
    // TODO: call_stack: Vec<Frame>,
    // TODO: operand_stack: Vec<Value>,
    // TODO: locals: Vec<Value>,
    // TODO: memory: LinearMemory,
}

pub struct Executor {
    // TODO: context: ExecutionContext,
    // TODO: module: Module,
}

impl Executor {
    pub fn new(_module: Module) -> Result<Self, String> {
        // TODO: Implement executor initialization
        Err("Not yet implemented".to_string())
    }

    pub fn execute(&mut self, _func_idx: u32) -> Result<Vec<Value>, String> {
        // TODO: Implement function execution
        Err("Not yet implemented".to_string())
    }
}
