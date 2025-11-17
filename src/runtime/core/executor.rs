/// WASM instruction executor
/// Handles execution context, stack, call frames, and instruction dispatch
use super::memory::LinearMemory;
use super::module::{Module, ValueType};
use super::values::Value;
use std::io::Cursor;

/// WASM instruction representation
/// Covers all instruction types from the WebAssembly specification
#[derive(Debug, Clone)]
pub enum Instruction {
    // Constants
    I32Const(i32),
    I64Const(i64),
    F32Const(f32),
    F64Const(f64),

    // Numeric operations - i32
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,

    // Numeric operations - i64
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,

    // Numeric operations - f32
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Sqrt,
    F32Min,
    F32Max,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Abs,
    F32Neg,
    F32Copysign,

    // Numeric operations - f64
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Sqrt,
    F64Min,
    F64Max,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Abs,
    F64Neg,
    F64Copysign,

    // Comparison - i32
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,

    // Comparison - i64
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,

    // Comparison - f32
    F32Eq,
    F32Ne,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,

    // Comparison - f64
    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,

    // Type conversions
    I32WrapI64,
    I32TruncF32S,
    I32TruncF32U,
    I32TruncF64S,
    I32TruncF64U,
    I64ExtendI32S,
    I64ExtendI32U,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    F32ConvertI32S,
    F32ConvertI32U,
    F32ConvertI64S,
    F32ConvertI64U,
    F32DemoteF64,
    F64ConvertI32S,
    F64ConvertI32U,
    F64ConvertI64S,
    F64ConvertI64U,
    F64PromoteF32,
    I32Reinterpret,
    I64Reinterpret,
    F32Reinterpret,
    F64Reinterpret,

    // Memory
    I32Load,
    I64Load,
    F32Load,
    F64Load,
    I32Load8S,
    I32Load8U,
    I32Load16S,
    I32Load16U,
    I64Load8S,
    I64Load8U,
    I64Load16S,
    I64Load16U,
    I64Load32S,
    I64Load32U,
    I32Store,
    I64Store,
    F32Store,
    F64Store,
    I32Store8,
    I32Store16,
    I64Store8,
    I64Store16,
    I64Store32,
    MemorySize,
    MemoryGrow,

    // Local/Global
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    GlobalGet(u32),
    GlobalSet(u32),

    // Control flow
    Nop,
    Unreachable,
    Block(Option<ValueType>),
    Loop(Option<ValueType>),
    If(Option<ValueType>),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable(Vec<u32>, u32),
    Return,
    Call(u32),
    CallIndirect(u32),
    Drop,
    Select,
}

/// Helper function to decode a LEB128-encoded signed integer
fn decode_i32_leb128(cursor: &mut Cursor<&[u8]>) -> Result<i32, String> {
    let mut result: i32 = 0;
    let mut shift = 0;
    let mut byte_buf = [0u8; 1];

    loop {
        if std::io::Read::read(cursor, &mut byte_buf).is_err() {
            return Err("EOF while reading LEB128".to_string());
        }
        let byte = byte_buf[0];
        result |= ((byte & 0x7f) as i32) << shift;

        if (byte & 0x80) == 0 {
            // Sign extend if necessary
            if shift < 31 && (byte & 0x40) != 0 {
                result |= -(1 << (shift + 7));
            }
            return Ok(result);
        }
        shift += 7;
        if shift >= 32 {
            return Err("LEB128 value too large for i32".to_string());
        }
    }
}

/// Helper function to decode a LEB128-encoded unsigned integer
fn decode_u32_leb128(cursor: &mut Cursor<&[u8]>) -> Result<u32, String> {
    let mut result: u32 = 0;
    let mut shift = 0;
    let mut byte_buf = [0u8; 1];

    loop {
        if std::io::Read::read(cursor, &mut byte_buf).is_err() {
            return Err("EOF while reading LEB128".to_string());
        }
        let byte = byte_buf[0];
        result |= ((byte & 0x7f) as u32) << shift;

        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 35 {
            return Err("LEB128 value too large for u32".to_string());
        }
    }
}

/// Helper function to decode a LEB128-encoded signed i64
fn decode_i64_leb128(cursor: &mut Cursor<&[u8]>) -> Result<i64, String> {
    let mut result: i64 = 0;
    let mut shift = 0;
    let mut byte_buf = [0u8; 1];

    loop {
        if std::io::Read::read(cursor, &mut byte_buf).is_err() {
            return Err("EOF while reading LEB128".to_string());
        }
        let byte = byte_buf[0];
        result |= ((byte & 0x7f) as i64) << shift;

        if (byte & 0x80) == 0 {
            // Sign extend if necessary
            if shift < 63 && (byte & 0x40) != 0 {
                result |= -(1i64 << (shift + 7));
            }
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            return Err("LEB128 value too large for i64".to_string());
        }
    }
}

/// Decode a single WASM instruction from bytecode
pub fn decode_instruction(cursor: &mut Cursor<&[u8]>) -> Result<Instruction, String> {
    let mut byte_buf = [0u8; 1];
    if std::io::Read::read(cursor, &mut byte_buf).is_err() {
        return Err("EOF while reading instruction".to_string());
    }
    let byte = byte_buf[0];

    match byte {
        // Constants
        0x41 => Ok(Instruction::I32Const(decode_i32_leb128(cursor)?)),
        0x42 => Ok(Instruction::I64Const(decode_i64_leb128(cursor)?)),
        0x43 => {
            let mut buf = [0u8; 4];
            if std::io::Read::read(cursor, &mut buf).is_err() {
                return Err("EOF while reading f32".to_string());
            }
            Ok(Instruction::F32Const(f32::from_le_bytes(buf)))
        }
        0x44 => {
            let mut buf = [0u8; 8];
            if std::io::Read::read(cursor, &mut buf).is_err() {
                return Err("EOF while reading f64".to_string());
            }
            Ok(Instruction::F64Const(f64::from_le_bytes(buf)))
        }

        // i32 comparison
        0x46 => Ok(Instruction::I32Eq),
        0x47 => Ok(Instruction::I32Ne),
        0x48 => Ok(Instruction::I32LtS),
        0x49 => Ok(Instruction::I32LtU),
        0x4A => Ok(Instruction::I32GtS),
        0x4B => Ok(Instruction::I32GtU),
        0x4C => Ok(Instruction::I32LeS),
        0x4D => Ok(Instruction::I32LeU),
        0x4E => Ok(Instruction::I32GeS),
        0x4F => Ok(Instruction::I32GeU),

        // i64 comparison
        0x51 => Ok(Instruction::I64Eq),
        0x52 => Ok(Instruction::I64Ne),
        0x53 => Ok(Instruction::I64LtS),
        0x54 => Ok(Instruction::I64LtU),
        0x55 => Ok(Instruction::I64GtS),
        0x56 => Ok(Instruction::I64GtU),
        0x57 => Ok(Instruction::I64LeS),
        0x58 => Ok(Instruction::I64LeU),
        0x59 => Ok(Instruction::I64GeS),
        0x5A => Ok(Instruction::I64GeU),

        // f32 comparison
        0x5B => Ok(Instruction::F32Eq),
        0x5C => Ok(Instruction::F32Ne),
        0x5D => Ok(Instruction::F32Lt),
        0x5E => Ok(Instruction::F32Gt),
        0x5F => Ok(Instruction::F32Le),
        0x60 => Ok(Instruction::F32Ge),

        // f64 comparison
        0x61 => Ok(Instruction::F64Eq),
        0x62 => Ok(Instruction::F64Ne),
        0x63 => Ok(Instruction::F64Lt),
        0x64 => Ok(Instruction::F64Gt),
        0x65 => Ok(Instruction::F64Le),
        0x66 => Ok(Instruction::F64Ge),

        // i32 arithmetic operations
        0x6A => Ok(Instruction::I32Add),
        0x6B => Ok(Instruction::I32Sub),
        0x6C => Ok(Instruction::I32Mul),
        0x6D => Ok(Instruction::I32DivS),
        0x6E => Ok(Instruction::I32DivU),
        0x6F => Ok(Instruction::I32RemS),
        0x70 => Ok(Instruction::I32RemU),
        0x71 => Ok(Instruction::I32And),
        0x72 => Ok(Instruction::I32Or),
        0x73 => Ok(Instruction::I32Xor),
        0x74 => Ok(Instruction::I32Shl),
        0x75 => Ok(Instruction::I32ShrS),
        0x76 => Ok(Instruction::I32ShrU),
        0x77 => Ok(Instruction::I32Rotl),
        0x78 => Ok(Instruction::I32Rotr),

        // i64 arithmetic operations
        0x7C => Ok(Instruction::I64Add),
        0x7D => Ok(Instruction::I64Sub),
        0x7E => Ok(Instruction::I64Mul),
        0x7F => Ok(Instruction::I64DivS),
        0x80 => Ok(Instruction::I64DivU),
        0x81 => Ok(Instruction::I64RemS),
        0x82 => Ok(Instruction::I64RemU),
        0x83 => Ok(Instruction::I64And),
        0x84 => Ok(Instruction::I64Or),
        0x85 => Ok(Instruction::I64Xor),
        0x86 => Ok(Instruction::I64Shl),
        0x87 => Ok(Instruction::I64ShrS),
        0x88 => Ok(Instruction::I64ShrU),
        0x89 => Ok(Instruction::I64Rotl),
        0x8A => Ok(Instruction::I64Rotr),

        // f32 arithmetic operations
        0x92 => Ok(Instruction::F32Add),
        0x93 => Ok(Instruction::F32Sub),
        0x94 => Ok(Instruction::F32Mul),
        0x95 => Ok(Instruction::F32Div),
        0x96 => Ok(Instruction::F32Sqrt),
        0x97 => Ok(Instruction::F32Min),
        0x98 => Ok(Instruction::F32Max),
        0x99 => Ok(Instruction::F32Ceil),
        0x9A => Ok(Instruction::F32Floor),
        0x9B => Ok(Instruction::F32Trunc),
        0x9C => Ok(Instruction::F32Nearest),
        0x9D => Ok(Instruction::F32Abs),
        0x9E => Ok(Instruction::F32Neg),
        0x9F => Ok(Instruction::F32Copysign),

        // f64 arithmetic operations
        0xA0 => Ok(Instruction::F64Add),
        0xA1 => Ok(Instruction::F64Sub),
        0xA2 => Ok(Instruction::F64Mul),
        0xA3 => Ok(Instruction::F64Div),
        0xA4 => Ok(Instruction::F64Sqrt),
        0xA5 => Ok(Instruction::F64Min),
        0xA6 => Ok(Instruction::F64Max),
        0xA7 => Ok(Instruction::F64Ceil),
        0xA8 => Ok(Instruction::F64Floor),
        0xA9 => Ok(Instruction::F64Trunc),
        0xAA => Ok(Instruction::F64Nearest),
        0xAB => Ok(Instruction::F64Abs),
        0xAC => Ok(Instruction::F64Neg),
        0xAD => Ok(Instruction::F64Copysign),

        // Memory
        0x28 => Ok(Instruction::I32Load),
        0x29 => Ok(Instruction::I64Load),
        0x2A => Ok(Instruction::F32Load),
        0x2B => Ok(Instruction::F64Load),
        0x2C => Ok(Instruction::I32Load8S),
        0x2D => Ok(Instruction::I32Load8U),
        0x2E => Ok(Instruction::I32Load16S),
        0x2F => Ok(Instruction::I32Load16U),
        0x30 => Ok(Instruction::I64Load8S),
        0x31 => Ok(Instruction::I64Load8U),
        0x32 => Ok(Instruction::I64Load16S),
        0x33 => Ok(Instruction::I64Load16U),
        0x34 => Ok(Instruction::I64Load32S),
        0x35 => Ok(Instruction::I64Load32U),
        0x36 => Ok(Instruction::I32Store),
        0x37 => Ok(Instruction::I64Store),
        0x38 => Ok(Instruction::F32Store),
        0x39 => Ok(Instruction::F64Store),
        0x3A => Ok(Instruction::I32Store8),
        0x3B => Ok(Instruction::I32Store16),
        0x3C => Ok(Instruction::I64Store8),
        0x3D => Ok(Instruction::I64Store16),
        0x3E => Ok(Instruction::I64Store32),
        0x3F => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::MemorySize)
        }
        0x40 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::MemoryGrow)
        }

        // Local/Global
        0x20 => Ok(Instruction::LocalGet(decode_u32_leb128(cursor)?)),
        0x21 => Ok(Instruction::LocalSet(decode_u32_leb128(cursor)?)),
        0x22 => Ok(Instruction::LocalTee(decode_u32_leb128(cursor)?)),
        0x23 => Ok(Instruction::GlobalGet(decode_u32_leb128(cursor)?)),
        0x24 => Ok(Instruction::GlobalSet(decode_u32_leb128(cursor)?)),

        // Control flow
        0x00 => Ok(Instruction::Unreachable),
        0x01 => Ok(Instruction::Nop),
        0x02 => Ok(Instruction::Block(None)),
        0x03 => Ok(Instruction::Loop(None)),
        0x04 => Ok(Instruction::If(None)),
        0x05 => Ok(Instruction::Else),
        0x0B => Ok(Instruction::End),
        0x0C => Ok(Instruction::Br(decode_u32_leb128(cursor)?)),
        0x0D => Ok(Instruction::BrIf(decode_u32_leb128(cursor)?)),
        0x0E => {
            let count = decode_u32_leb128(cursor)? as usize;
            let mut targets = Vec::with_capacity(count);
            for _ in 0..count {
                targets.push(decode_u32_leb128(cursor)?);
            }
            let default = decode_u32_leb128(cursor)?;
            Ok(Instruction::BrTable(targets, default))
        }
        0x0F => Ok(Instruction::Return),
        0x10 => Ok(Instruction::Call(decode_u32_leb128(cursor)?)),
        0x11 => Ok(Instruction::CallIndirect(decode_u32_leb128(cursor)?)),
        0x1A => Ok(Instruction::Drop),
        0x1B => Ok(Instruction::Select),

        _ => Err(format!("Unknown instruction: 0x{byte:02X}")),
    }
}

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

    /// Execute a function by index and return its results
    pub fn execute(&mut self, func_idx: u32) -> Result<Vec<Value>, String> {
        // Get function signature and code (clone to avoid borrow issues)
        let func = {
            let func = self
                .module
                .functions
                .get(func_idx as usize)
                .ok_or_else(|| format!("Function index {func_idx} out of bounds"))?;

            let func_type = self
                .module
                .types
                .get(func.type_index as usize)
                .ok_or_else(|| format!("Function type index {} out of bounds", func.type_index))?;

            // Initialize locals: parameters + local variables
            let mut locals = Vec::new();

            // Add parameter slots (initialized to zero by default)
            for _ in 0..func_type.params.len() {
                locals.push(Value::I32(0)); // Placeholder for parameters
            }

            // Add local variable slots
            for (count, value_type) in &func.locals {
                for _ in 0..*count {
                    let default_value = match value_type {
                        ValueType::I32 => Value::I32(0),
                        ValueType::I64 => Value::I64(0),
                        ValueType::F32 => Value::F32(0.0),
                        ValueType::F64 => Value::F64(0.0),
                        _ => {
                            return Err(format!("Unsupported value type in locals: {value_type:?}"))
                        }
                    };
                    locals.push(default_value);
                }
            }

            // Return tuple with locals, num_returns, and code
            (locals, func_type.results.len(), func.code.clone())
        };

        let (locals, num_returns, code) = func;

        // Create call frame
        let frame = Frame::new(func_idx, locals, num_returns);
        self.context.push_frame(frame);

        // Execute bytecode
        let mut cursor = Cursor::new(code.as_slice());
        self.execute_bytecode(&mut cursor)?;

        // Pop frame and collect return values
        let _frame = self.context.pop_frame()?;

        // Pop return values from stack (in reverse order)
        let mut results = Vec::new();
        for _ in 0..num_returns {
            results.push(self.context.pop()?);
        }
        results.reverse();

        Ok(results)
    }

    /// Execute bytecode starting from current position in cursor
    fn execute_bytecode(&mut self, cursor: &mut Cursor<&[u8]>) -> Result<(), String> {
        loop {
            // Check if we've reached end of bytecode
            if cursor.position() >= cursor.get_ref().len() as u64 {
                break;
            }

            let instr = decode_instruction(cursor)?;
            self.dispatch_instruction(instr)?;
        }
        Ok(())
    }

    /// Dispatch instruction to handler
    fn dispatch_instruction(&mut self, instr: Instruction) -> Result<(), String> {
        match instr {
            // Constants
            Instruction::I32Const(v) => self.context.push(Value::I32(v)),
            Instruction::I64Const(v) => self.context.push(Value::I64(v)),
            Instruction::F32Const(v) => self.context.push(Value::F32(v)),
            Instruction::F64Const(v) => self.context.push(Value::F64(v)),

            // i32 arithmetic
            Instruction::I32Add => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(x.wrapping_add(y)))
                    }
                    _ => return Err("Type mismatch for i32.add".to_string()),
                }
            }
            Instruction::I32Sub => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(x.wrapping_sub(y)))
                    }
                    _ => return Err("Type mismatch for i32.sub".to_string()),
                }
            }
            Instruction::I32Mul => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(x.wrapping_mul(y)))
                    }
                    _ => return Err("Type mismatch for i32.mul".to_string()),
                }
            }
            Instruction::I32DivS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        if x == i32::MIN && y == -1 {
                            return Err("Integer overflow in division".to_string());
                        }
                        self.context.push(Value::I32(x / y));
                    }
                    _ => return Err("Type mismatch for i32.div_s".to_string()),
                }
            }
            Instruction::I32DivU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        self.context
                            .push(Value::I32(((x as u32) / (y as u32)) as i32));
                    }
                    _ => return Err("Type mismatch for i32.div_u".to_string()),
                }
            }
            Instruction::I32RemS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        self.context.push(Value::I32(x % y));
                    }
                    _ => return Err("Type mismatch for i32.rem_s".to_string()),
                }
            }
            Instruction::I32RemU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        self.context
                            .push(Value::I32(((x as u32) % (y as u32)) as i32));
                    }
                    _ => return Err("Type mismatch for i32.rem_u".to_string()),
                }
            }

            // i32 bitwise
            Instruction::I32And => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => self.context.push(Value::I32(x & y)),
                    _ => return Err("Type mismatch for i32.and".to_string()),
                }
            }
            Instruction::I32Or => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => self.context.push(Value::I32(x | y)),
                    _ => return Err("Type mismatch for i32.or".to_string()),
                }
            }
            Instruction::I32Xor => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => self.context.push(Value::I32(x ^ y)),
                    _ => return Err("Type mismatch for i32.xor".to_string()),
                }
            }
            Instruction::I32Shl => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(x.wrapping_shl(y as u32 & 31)));
                    }
                    _ => return Err("Type mismatch for i32.shl".to_string()),
                }
            }
            Instruction::I32ShrS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(x >> (y as u32 & 31)));
                    }
                    _ => return Err("Type mismatch for i32.shr_s".to_string()),
                }
            }
            Instruction::I32ShrU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32(((x as u32) >> (y as u32 & 31)) as i32));
                    }
                    _ => return Err("Type mismatch for i32.shr_u".to_string()),
                }
            }
            Instruction::I32Rotl => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32((x as u32).rotate_left(y as u32) as i32));
                    }
                    _ => return Err("Type mismatch for i32.rotl".to_string()),
                }
            }
            Instruction::I32Rotr => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32((x as u32).rotate_right(y as u32) as i32));
                    }
                    _ => return Err("Type mismatch for i32.rotr".to_string()),
                }
            }

            // i32 comparison
            Instruction::I32Eq => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(if x == y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.eq".to_string()),
                }
            }
            Instruction::I32Ne => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(if x != y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.ne".to_string()),
                }
            }
            Instruction::I32LtS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(if x < y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.lt_s".to_string()),
                }
            }
            Instruction::I32LtU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32(if (x as u32) < (y as u32) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.lt_u".to_string()),
                }
            }
            Instruction::I32GtS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(if x > y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.gt_s".to_string()),
                }
            }
            Instruction::I32GtU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32(if (x as u32) > (y as u32) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.gt_u".to_string()),
                }
            }
            Instruction::I32LeS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(if x <= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.le_s".to_string()),
                }
            }
            Instruction::I32LeU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32(if (x as u32) <= (y as u32) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.le_u".to_string()),
                }
            }
            Instruction::I32GeS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context.push(Value::I32(if x >= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.ge_s".to_string()),
                }
            }
            Instruction::I32GeU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I32(x), Value::I32(y)) => {
                        self.context
                            .push(Value::I32(if (x as u32) >= (y as u32) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i32.ge_u".to_string()),
                }
            }

            // Local operations
            Instruction::LocalGet(idx) => {
                let frame = self.context.current_frame()?;
                let value = frame.get_local(idx as usize)?;
                self.context.push(value);
            }
            Instruction::LocalSet(idx) => {
                let value = self.context.pop()?;
                let frame = self.context.current_frame_mut()?;
                frame.set_local(idx as usize, value)?;
            }
            Instruction::LocalTee(idx) => {
                let value = self.context.peek()?;
                let frame = self.context.current_frame_mut()?;
                frame.set_local(idx as usize, value)?;
            }

            // Control flow - basic ones first
            Instruction::Nop => {}
            Instruction::Unreachable => return Err("Unreachable instruction executed".to_string()),
            Instruction::Return => return Ok(()),
            Instruction::Drop => {
                self.context.pop()?;
            }

            // Not yet implemented
            Instruction::I64Add
            | Instruction::I64Sub
            | Instruction::I64Mul
            | Instruction::I64DivS
            | Instruction::I64DivU
            | Instruction::I64RemS
            | Instruction::I64RemU
            | Instruction::I64And
            | Instruction::I64Or
            | Instruction::I64Xor
            | Instruction::I64Shl
            | Instruction::I64ShrS
            | Instruction::I64ShrU
            | Instruction::I64Rotl
            | Instruction::I64Rotr
            | Instruction::I64Eq
            | Instruction::I64Ne
            | Instruction::I64LtS
            | Instruction::I64LtU
            | Instruction::I64GtS
            | Instruction::I64GtU
            | Instruction::I64LeS
            | Instruction::I64LeU
            | Instruction::I64GeS
            | Instruction::I64GeU
            | Instruction::F32Add
            | Instruction::F32Sub
            | Instruction::F32Mul
            | Instruction::F32Div
            | Instruction::F32Sqrt
            | Instruction::F32Min
            | Instruction::F32Max
            | Instruction::F32Ceil
            | Instruction::F32Floor
            | Instruction::F32Trunc
            | Instruction::F32Nearest
            | Instruction::F32Abs
            | Instruction::F32Neg
            | Instruction::F32Copysign
            | Instruction::F64Add
            | Instruction::F64Sub
            | Instruction::F64Mul
            | Instruction::F64Div
            | Instruction::F64Sqrt
            | Instruction::F64Min
            | Instruction::F64Max
            | Instruction::F64Ceil
            | Instruction::F64Floor
            | Instruction::F64Trunc
            | Instruction::F64Nearest
            | Instruction::F64Abs
            | Instruction::F64Neg
            | Instruction::F64Copysign
            | Instruction::F32Eq
            | Instruction::F32Ne
            | Instruction::F32Lt
            | Instruction::F32Gt
            | Instruction::F32Le
            | Instruction::F32Ge
            | Instruction::F64Eq
            | Instruction::F64Ne
            | Instruction::F64Lt
            | Instruction::F64Gt
            | Instruction::F64Le
            | Instruction::F64Ge
            | Instruction::I32WrapI64
            | Instruction::I32TruncF32S
            | Instruction::I32TruncF32U
            | Instruction::I32TruncF64S
            | Instruction::I32TruncF64U
            | Instruction::I64ExtendI32S
            | Instruction::I64ExtendI32U
            | Instruction::I64TruncF32S
            | Instruction::I64TruncF32U
            | Instruction::I64TruncF64S
            | Instruction::I64TruncF64U
            | Instruction::F32ConvertI32S
            | Instruction::F32ConvertI32U
            | Instruction::F32ConvertI64S
            | Instruction::F32ConvertI64U
            | Instruction::F32DemoteF64
            | Instruction::F64ConvertI32S
            | Instruction::F64ConvertI32U
            | Instruction::F64ConvertI64S
            | Instruction::F64ConvertI64U
            | Instruction::F64PromoteF32
            | Instruction::I32Reinterpret
            | Instruction::I64Reinterpret
            | Instruction::F32Reinterpret
            | Instruction::F64Reinterpret
            | Instruction::I32Load
            | Instruction::I64Load
            | Instruction::F32Load
            | Instruction::F64Load
            | Instruction::I32Load8S
            | Instruction::I32Load8U
            | Instruction::I32Load16S
            | Instruction::I32Load16U
            | Instruction::I64Load8S
            | Instruction::I64Load8U
            | Instruction::I64Load16S
            | Instruction::I64Load16U
            | Instruction::I64Load32S
            | Instruction::I64Load32U
            | Instruction::I32Store
            | Instruction::I64Store
            | Instruction::F32Store
            | Instruction::F64Store
            | Instruction::I32Store8
            | Instruction::I32Store16
            | Instruction::I64Store8
            | Instruction::I64Store16
            | Instruction::I64Store32
            | Instruction::MemorySize
            | Instruction::MemoryGrow
            | Instruction::GlobalGet(_)
            | Instruction::GlobalSet(_)
            | Instruction::Block(_)
            | Instruction::Loop(_)
            | Instruction::If(_)
            | Instruction::Else
            | Instruction::End
            | Instruction::Br(_)
            | Instruction::BrIf(_)
            | Instruction::BrTable(_, _)
            | Instruction::Call(_)
            | Instruction::CallIndirect(_)
            | Instruction::Select => {
                return Err(format!("Instruction not yet implemented: {instr:?}"));
            }
        }

        Ok(())
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

    #[test]
    fn test_instruction_i32_const() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(42));
        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_instruction_i32_add() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(10));
        ctx.push(Value::I32(32));

        // i32.add pops two values and pushes result
        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => ctx.push(Value::I32(x.wrapping_add(y))),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_instruction_i32_sub() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(52));
        ctx.push(Value::I32(10));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => ctx.push(Value::I32(x.wrapping_sub(y))),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_instruction_i32_mul() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(6));
        ctx.push(Value::I32(7));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => ctx.push(Value::I32(x.wrapping_mul(y))),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_instruction_i32_div_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(84));
        ctx.push(Value::I32(2));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                if y != 0 {
                    ctx.push(Value::I32(x / y));
                }
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_instruction_i32_eq() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(42));
        ctx.push(Value::I32(42));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32(if x == y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_instruction_i32_ne() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(42));
        ctx.push(Value::I32(10));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32(if x != y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_instruction_i32_lt_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(10));
        ctx.push(Value::I32(42));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32(if x < y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_instruction_i32_gt_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(42));
        ctx.push(Value::I32(10));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32(if x > y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_instruction_i32_and() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(0xFF00));
        ctx.push(Value::I32(0x00FF));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => ctx.push(Value::I32(x & y)),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(0x0000));
    }

    #[test]
    fn test_instruction_i32_or() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(0xFF00));
        ctx.push(Value::I32(0x00FF));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => ctx.push(Value::I32(x | y)),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(0xFFFF));
    }

    #[test]
    fn test_instruction_i32_xor() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(0xFFFF));
        ctx.push(Value::I32(0x00FF));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => ctx.push(Value::I32(x ^ y)),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(0xFF00));
    }

    #[test]
    fn test_instruction_i32_shl() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(1));
        ctx.push(Value::I32(3));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32(x.wrapping_shl(y as u32 & 31)));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(8));
    }

    #[test]
    fn test_instruction_i32_shr_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(16));
        ctx.push(Value::I32(2));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32(x >> (y as u32 & 31)));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(4));
    }

    #[test]
    fn test_instruction_local_get_set() {
        let locals = vec![Value::I32(10), Value::I32(20), Value::I32(30)];
        let mut frame = Frame::new(0, locals, 0);

        // local.get 1
        let val = frame.get_local(1).unwrap();
        assert_eq!(val, Value::I32(20));

        // local.set 1 (set to 99)
        frame.set_local(1, Value::I32(99)).unwrap();
        assert_eq!(frame.get_local(1).unwrap(), Value::I32(99));
    }

    #[test]
    fn test_instruction_local_tee() {
        let locals = vec![Value::I32(0)];
        let mut frame = Frame::new(0, locals, 0);
        let ctx_stack_val = Value::I32(42);

        // local.tee 0 (set local and keep value on stack)
        frame.set_local(0, ctx_stack_val).unwrap();
        assert_eq!(frame.get_local(0).unwrap(), Value::I32(42));
    }

    #[test]
    fn test_decode_instruction_i32_const() {
        let bytecode = vec![0x41, 0x2A]; // i32.const 42
        let mut cursor = Cursor::new(bytecode.as_slice());
        let instr = decode_instruction(&mut cursor).unwrap();
        match instr {
            Instruction::I32Const(v) => assert_eq!(v, 42),
            _ => panic!("Expected I32Const"),
        }
    }

    #[test]
    fn test_decode_instruction_i32_add() {
        let bytecode = vec![0x6A]; // i32.add
        let mut cursor = Cursor::new(bytecode.as_slice());
        let instr = decode_instruction(&mut cursor).unwrap();
        match instr {
            Instruction::I32Add => {}
            _ => panic!("Expected I32Add"),
        }
    }

    #[test]
    fn test_decode_instruction_local_get() {
        let bytecode = vec![0x20, 0x01]; // local.get 1
        let mut cursor = Cursor::new(bytecode.as_slice());
        let instr = decode_instruction(&mut cursor).unwrap();
        match instr {
            Instruction::LocalGet(idx) => assert_eq!(idx, 1),
            _ => panic!("Expected LocalGet"),
        }
    }

    #[test]
    fn test_decode_instruction_local_set() {
        let bytecode = vec![0x21, 0x02]; // local.set 2
        let mut cursor = Cursor::new(bytecode.as_slice());
        let instr = decode_instruction(&mut cursor).unwrap();
        match instr {
            Instruction::LocalSet(idx) => assert_eq!(idx, 2),
            _ => panic!("Expected LocalSet"),
        }
    }

    #[test]
    fn test_instruction_nop() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(42));
        // nop does nothing
        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_instruction_drop() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(42));
        ctx.push(Value::I32(99));
        // drop removes top value
        ctx.pop().unwrap();
        assert_eq!(ctx.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_i32_rem_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(47));
        ctx.push(Value::I32(5));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                if y != 0 {
                    ctx.push(Value::I32(x % y));
                }
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(2)); // 47 % 5 = 2
    }

    #[test]
    fn test_i32_rotl() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(0x00000001u32 as i32));
        ctx.push(Value::I32(1));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32((x as u32).rotate_left(y as u32) as i32));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(0x00000002u32 as i32));
    }

    #[test]
    fn test_i32_rotr() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I32(0x80000000u32 as i32));
        ctx.push(Value::I32(1));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I32(x), Value::I32(y)) => {
                ctx.push(Value::I32((x as u32).rotate_right(y as u32) as i32));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(0x40000000u32 as i32));
    }
}
