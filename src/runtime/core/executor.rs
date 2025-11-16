/// WASM instruction executor
/// Handles execution context, stack, call frames, and instruction dispatch
use super::memory::LinearMemory;
use super::module::Module;
use super::values::Value;

/// Represents a single function call frame on the call stack
#[derive(Debug, Clone)]
pub struct Frame {
    /// Function index being executed
    pub func_idx: u32,
    /// Local variables in this frame
    pub locals: Vec<Value>,
    /// Return address (instruction pointer in calling function)
    pub return_addr: usize,
    /// Number of return values expected
    pub num_returns: usize,
}

impl Frame {
    pub fn new(func_idx: u32, locals: Vec<Value>, num_returns: usize) -> Self {
        Frame {
            func_idx,
            locals,
            return_addr: 0,
            num_returns,
        }
    }

    pub fn get_local(&self, idx: usize) -> Result<Value, String> {
        self.locals.get(idx).copied().ok_or_else(|| {
            format!(
                "Local variable index {} out of bounds ({})",
                idx,
                self.locals.len()
            )
        })
    }

    pub fn set_local(&mut self, idx: usize, value: Value) -> Result<(), String> {
        if idx >= self.locals.len() {
            return Err(format!(
                "Local variable index {} out of bounds ({})",
                idx,
                self.locals.len()
            ));
        }
        self.locals[idx] = value;
        Ok(())
    }
}

/// Execution context for WASM module execution
#[derive(Debug)]
pub struct ExecutionContext {
    /// Call stack (stack of frames)
    pub call_stack: Vec<Frame>,
    /// Operand stack (values pushed/popped during execution)
    pub operand_stack: Vec<Value>,
    /// Linear memory
    pub memory: LinearMemory,
}

impl ExecutionContext {
    /// Create new execution context with given memory config
    pub fn new(memory_initial: u32, memory_max: Option<u32>) -> Result<Self, String> {
        let memory = LinearMemory::new(memory_initial, memory_max)?;
        Ok(ExecutionContext {
            call_stack: Vec::new(),
            operand_stack: Vec::new(),
            memory,
        })
    }

    /// Push a value onto operand stack
    pub fn push(&mut self, value: Value) {
        self.operand_stack.push(value);
    }

    /// Pop a value from operand stack
    pub fn pop(&mut self) -> Result<Value, String> {
        self.operand_stack
            .pop()
            .ok_or_else(|| "Operand stack underflow".to_string())
    }

    /// Peek top value without removing it
    pub fn peek(&self) -> Result<Value, String> {
        self.operand_stack
            .last()
            .copied()
            .ok_or_else(|| "Operand stack is empty".to_string())
    }

    /// Pop n values from operand stack
    pub fn pop_n(&mut self, n: usize) -> Result<Vec<Value>, String> {
        if self.operand_stack.len() < n {
            return Err(format!(
                "Operand stack underflow: need {}, have {}",
                n,
                self.operand_stack.len()
            ));
        }
        let idx = self.operand_stack.len() - n;
        Ok(self.operand_stack.drain(idx..).collect())
    }

    /// Push call frame
    pub fn push_frame(&mut self, frame: Frame) {
        self.call_stack.push(frame);
    }

    /// Pop call frame
    pub fn pop_frame(&mut self) -> Result<Frame, String> {
        self.call_stack
            .pop()
            .ok_or_else(|| "Call stack underflow".to_string())
    }

    /// Get current frame (mutable)
    pub fn current_frame_mut(&mut self) -> Result<&mut Frame, String> {
        self.call_stack
            .last_mut()
            .ok_or_else(|| "No active frame".to_string())
    }

    /// Get current frame
    pub fn current_frame(&self) -> Result<&Frame, String> {
        self.call_stack
            .last()
            .ok_or_else(|| "No active frame".to_string())
    }
}

/// WASM instruction executor
pub struct Executor {
    context: ExecutionContext,
    module: Module,
}

impl Executor {
    /// Create new executor for module
    pub fn new(module: Module) -> Result<Self, String> {
        // Get memory config from module
        let (initial, max) = if let Some(mem) = &module.memory {
            (mem.initial, mem.max)
        } else {
            // Default: 1 page, no max
            (1, None)
        };

        let context = ExecutionContext::new(initial, max)?;
        Ok(Executor { context, module })
    }

    /// Execute a function by index
    pub fn execute(&mut self, _func_idx: u32) -> Result<Vec<Value>, String> {
        // TODO: Implement function execution (Phase 1c)
        Err("Not yet implemented".to_string())
    }

    /// Get reference to execution context
    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    /// Get mutable reference to execution context
    pub fn context_mut(&mut self) -> &mut ExecutionContext {
        &mut self.context
    }

    /// Get reference to module
    pub fn module(&self) -> &Module {
        &self.module
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_local_access() {
        let locals = vec![
            Value::I32(42),
            Value::I64(1000),
            Value::F32(std::f32::consts::PI),
        ];
        let mut frame = Frame::new(0, locals, 1);

        assert_eq!(frame.get_local(0).unwrap(), Value::I32(42));
        assert_eq!(frame.get_local(1).unwrap(), Value::I64(1000));

        frame.set_local(0, Value::I32(99)).unwrap();
        assert_eq!(frame.get_local(0).unwrap(), Value::I32(99));
    }

    #[test]
    fn test_execution_context_operand_stack() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();

        ctx.push(Value::I32(42));
        ctx.push(Value::I64(100));
        ctx.push(Value::I32(99));

        assert_eq!(ctx.pop().unwrap(), Value::I32(99));
        assert_eq!(ctx.pop().unwrap(), Value::I64(100));
        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_execution_context_stack_underflow() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        let result = ctx.pop();
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_context_pop_n() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();

        ctx.push(Value::I32(1));
        ctx.push(Value::I32(2));
        ctx.push(Value::I32(3));
        ctx.push(Value::I32(4));

        let values = ctx.pop_n(2).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], Value::I32(3));
        assert_eq!(values[1], Value::I32(4));

        assert_eq!(ctx.pop().unwrap(), Value::I32(2));
    }

    #[test]
    fn test_execution_context_call_stack() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();

        let frame1 = Frame::new(0, vec![Value::I32(42)], 1);
        let frame2 = Frame::new(1, vec![Value::I64(100), Value::I32(99)], 2);

        ctx.push_frame(frame1);
        ctx.push_frame(frame2);

        assert_eq!(ctx.current_frame().unwrap().func_idx, 1);

        let popped = ctx.pop_frame().unwrap();
        assert_eq!(popped.func_idx, 1);
        assert_eq!(ctx.current_frame().unwrap().func_idx, 0);
    }

    #[test]
    fn test_executor_creation() {
        let module = super::super::module::Module::new();
        let executor = Executor::new(module).unwrap();
        assert_eq!(executor.context().memory.size(), 1);
    }

    #[test]
    fn test_executor_memory_access() {
        let module = super::super::module::Module::new();
        let mut executor = Executor::new(module).unwrap();

        executor
            .context_mut()
            .memory
            .write_i32(0, 0xDEADBEEFu32 as i32)
            .unwrap();
        assert_eq!(
            executor.context().memory.read_i32(0).unwrap(),
            0xDEADBEEFu32 as i32
        );
    }
}
