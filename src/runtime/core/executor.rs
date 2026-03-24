/// WASM instruction executor
/// Handles execution context, stack, call frames, and instruction dispatch
use super::linker::Linker;
use super::memory::LinearMemory;
use super::module::{ImportKind, Module, ValueType};
use super::values::Value;
use std::io::Cursor;

/// Sentinel prefix for proc_exit errors so callers can extract the exit code.
pub const WASI_PROC_EXIT_PREFIX: &str = "__wasi_proc_exit:";

/// Result of instruction dispatch for control flow signaling
#[derive(Debug, Clone, PartialEq)]
enum ControlFlow {
    Continue,
    Return,
}

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
    I32Clz,
    I32Ctz,
    I32Popcnt,
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
    I32Eqz,

    // Numeric operations - i64
    I64Clz,
    I64Ctz,
    I64Popcnt,
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
    I64Eqz,

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
/// Helper function to read a single byte
fn read_u8(cursor: &mut Cursor<&[u8]>) -> Result<u8, String> {
    let mut byte_buf = [0u8; 1];
    if std::io::Read::read(cursor, &mut byte_buf).is_err() {
        return Err("EOF while reading byte".to_string());
    }
    Ok(byte_buf[0])
}

/// Decode block type (for block, loop, if instructions)
fn decode_block_type(cursor: &mut Cursor<&[u8]>) -> Result<Option<ValueType>, String> {
    let byte = read_u8(cursor)?;
    match byte {
        0x40 => Ok(None), // empty block type (no result)
        0x7F => Ok(Some(ValueType::I32)),
        0x7E => Ok(Some(ValueType::I64)),
        0x7D => Ok(Some(ValueType::F32)),
        0x7C => Ok(Some(ValueType::F64)),
        // Function type index (0x00-0x3F) - for multi-value blocks
        // For now, treat as empty (no result)
        0x00..=0x3F => Ok(None),
        _ => Err(format!("Invalid block type: 0x{byte:02X}")),
    }
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

        // i32 test
        0x45 => Ok(Instruction::I32Eqz),

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

        // i64 test
        0x50 => Ok(Instruction::I64Eqz),

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

        // i32 unary operations
        0x67 => Ok(Instruction::I32Clz),
        0x68 => Ok(Instruction::I32Ctz),
        0x69 => Ok(Instruction::I32Popcnt),

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

        // i64 unary operations
        0x79 => Ok(Instruction::I64Clz),
        0x7A => Ok(Instruction::I64Ctz),
        0x7B => Ok(Instruction::I64Popcnt),

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

        // f32 unary operations
        0x8B => Ok(Instruction::F32Abs),
        0x8C => Ok(Instruction::F32Neg),
        0x8D => Ok(Instruction::F32Ceil),
        0x8E => Ok(Instruction::F32Floor),
        0x8F => Ok(Instruction::F32Trunc),
        0x90 => Ok(Instruction::F32Nearest),
        0x91 => Ok(Instruction::F32Sqrt),

        // f32 binary operations
        0x92 => Ok(Instruction::F32Add),
        0x93 => Ok(Instruction::F32Sub),
        0x94 => Ok(Instruction::F32Mul),
        0x95 => Ok(Instruction::F32Div),
        0x96 => Ok(Instruction::F32Min),
        0x97 => Ok(Instruction::F32Max),
        0x98 => Ok(Instruction::F32Copysign),

        // f64 unary operations
        0x99 => Ok(Instruction::F64Abs),
        0x9A => Ok(Instruction::F64Neg),
        0x9B => Ok(Instruction::F64Ceil),
        0x9C => Ok(Instruction::F64Floor),
        0x9D => Ok(Instruction::F64Trunc),
        0x9E => Ok(Instruction::F64Nearest),
        0x9F => Ok(Instruction::F64Sqrt),

        // f64 binary operations
        0xA0 => Ok(Instruction::F64Add),
        0xA1 => Ok(Instruction::F64Sub),
        0xA2 => Ok(Instruction::F64Mul),
        0xA3 => Ok(Instruction::F64Div),
        0xA4 => Ok(Instruction::F64Min),
        0xA5 => Ok(Instruction::F64Max),
        0xA6 => Ok(Instruction::F64Copysign),

        // Type conversions
        0xA7 => Ok(Instruction::I32WrapI64),
        0xA8 => Ok(Instruction::I32TruncF32S),
        0xA9 => Ok(Instruction::I32TruncF32U),
        0xAA => Ok(Instruction::I32TruncF64S),
        0xAB => Ok(Instruction::I32TruncF64U),
        0xAC => Ok(Instruction::I64ExtendI32S),
        0xAD => Ok(Instruction::I64ExtendI32U),
        0xAE => Ok(Instruction::I64TruncF32S),
        0xAF => Ok(Instruction::I64TruncF32U),
        0xB0 => Ok(Instruction::I64TruncF64S),
        0xB1 => Ok(Instruction::I64TruncF64U),
        0xB2 => Ok(Instruction::F32ConvertI32S),
        0xB3 => Ok(Instruction::F32ConvertI32U),
        0xB4 => Ok(Instruction::F32ConvertI64S),
        0xB5 => Ok(Instruction::F32ConvertI64U),
        0xB6 => Ok(Instruction::F32DemoteF64),
        0xB7 => Ok(Instruction::F64ConvertI32S),
        0xB8 => Ok(Instruction::F64ConvertI32U),
        0xB9 => Ok(Instruction::F64ConvertI64S),
        0xBA => Ok(Instruction::F64ConvertI64U),
        0xBB => Ok(Instruction::F64PromoteF32),
        0xBC => Ok(Instruction::I32Reinterpret),
        0xBD => Ok(Instruction::I64Reinterpret),
        0xBE => Ok(Instruction::F32Reinterpret),
        0xBF => Ok(Instruction::F64Reinterpret),

        // Memory (each has memarg: align + offset)
        0x28 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load)
        }
        0x29 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load)
        }
        0x2A => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F32Load)
        }
        0x2B => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F64Load)
        }
        0x2C => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load8S)
        }
        0x2D => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load8U)
        }
        0x2E => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load16S)
        }
        0x2F => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load16U)
        }
        0x30 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load8S)
        }
        0x31 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load8U)
        }
        0x32 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load16S)
        }
        0x33 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load16U)
        }
        0x34 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load32S)
        }
        0x35 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load32U)
        }
        0x36 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Store)
        }
        0x37 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store)
        }
        0x38 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F32Store)
        }
        0x39 => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F64Store)
        }
        0x3A => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Store8)
        }
        0x3B => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Store16)
        }
        0x3C => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store8)
        }
        0x3D => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store16)
        }
        0x3E => {
            let _align = decode_u32_leb128(cursor)?;
            let _offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store32)
        }
        0x3F => {
            // memory.size - reads memory index (0x00)
            let _mem_idx = read_u8(cursor)?;
            Ok(Instruction::MemorySize)
        }
        0x40 => {
            // memory.grow - reads memory index (0x00)
            let _mem_idx = read_u8(cursor)?;
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
        0x02 => {
            // block - read block type
            let block_type = decode_block_type(cursor)?;
            Ok(Instruction::Block(block_type))
        }
        0x03 => {
            // loop - read block type
            let block_type = decode_block_type(cursor)?;
            Ok(Instruction::Loop(block_type))
        }
        0x04 => {
            // if - read block type
            let block_type = decode_block_type(cursor)?;
            Ok(Instruction::If(block_type))
        }
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
        0x11 => {
            let type_idx = decode_u32_leb128(cursor)?;
            let _table_idx = read_u8(cursor)?; // table index (always 0x00 in WASM MVP)
            Ok(Instruction::CallIndirect(type_idx))
        }
        0x1A => Ok(Instruction::Drop),
        0x1B => Ok(Instruction::Select),

        _ => Err(format!("Unknown instruction: 0x{byte:02X}")),
    }
}

/// Represents a single function call frame on the call stack
/// Control flow block state for branching
#[derive(Debug, Clone)]
pub struct BlockFrame {
    /// Block type (None for Block/Loop, Some for If)
    pub block_type: Option<ValueType>,
    /// Stack depth at block entry
    pub stack_depth: usize,
    /// Whether this is a loop (affects branching)
    pub is_loop: bool,
    /// Bytecode position of the block start
    pub start_pos: usize,
    /// Bytecode position after 'end' instruction (branch target)
    pub end_pos: usize,
    /// For if blocks: whether we're in the then-branch (true) or else-branch (false)
    pub is_then_branch: bool,
}

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
    /// Control flow block stack (for block, loop, if)
    pub block_stack: Vec<BlockFrame>,
    /// Global variable values (mutable)
    pub globals: Vec<Value>,
}

impl ExecutionContext {
    /// Create new execution context with given memory config
    pub fn new(memory_initial: u32, memory_max: Option<u32>) -> Result<Self, String> {
        let memory = LinearMemory::new(memory_initial, memory_max)?;
        Ok(ExecutionContext {
            call_stack: Vec::new(),
            operand_stack: Vec::new(),
            memory,
            block_stack: Vec::new(),
            globals: Vec::new(),
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

    /// Push a control flow block
    pub fn push_block(
        &mut self,
        block_type: Option<ValueType>,
        is_loop: bool,
        start_pos: usize,
        end_pos: usize,
        is_then_branch: bool,
    ) {
        let stack_depth = self.operand_stack.len();
        self.block_stack.push(BlockFrame {
            block_type,
            stack_depth,
            is_loop,
            start_pos,
            end_pos,
            is_then_branch,
        });
    }

    /// Pop a control flow block
    pub fn pop_block(&mut self) -> Result<BlockFrame, String> {
        self.block_stack
            .pop()
            .ok_or_else(|| "Block stack underflow".to_string())
    }

    /// Get current block
    pub fn current_block(&self) -> Result<&BlockFrame, String> {
        self.block_stack
            .last()
            .ok_or_else(|| "No active block".to_string())
    }
}

/// Evaluate a constant expression (used for data/element segment offsets and global init).
/// Supports i32.const, i64.const, and global.get followed by end.
fn evaluate_const_expr(expr: &[u8]) -> Result<usize, String> {
    let mut cursor = Cursor::new(expr);
    let instr = decode_instruction(&mut cursor)?;
    match instr {
        Instruction::I32Const(v) => Ok(v as u32 as usize),
        Instruction::I64Const(v) => Ok(v as u64 as usize),
        _ => Err(format!(
            "Unsupported constant expression instruction: {instr:?}"
        )),
    }
}

/// WASM instruction executor
pub struct Executor {
    context: ExecutionContext,
    module: Module,
    linker: Option<Linker>,
    import_func_count: usize,
}

impl Executor {
    /// Create new executor for module (no host function support).
    pub fn new(module: Module) -> Result<Self, String> {
        Self::build(module, None)
    }

    /// Create executor with a linker that provides host functions for imports.
    pub fn new_with_linker(module: Module, linker: Linker) -> Result<Self, String> {
        Self::build(module, Some(linker))
    }

    fn build(module: Module, linker: Option<Linker>) -> Result<Self, String> {
        let import_func_count = module
            .imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::Function(_)))
            .count();

        // Memory config: check module section first, then imported memory
        let (initial, max) = if let Some(mem) = &module.memory {
            (mem.initial, mem.max)
        } else {
            let imported_mem = module.imports.iter().find_map(|i| match &i.kind {
                ImportKind::Memory(mem) => Some((mem.initial, mem.max)),
                _ => None,
            });
            imported_mem.unwrap_or((1, None))
        };

        let mut context = ExecutionContext::new(initial, max)?;

        for global in &module.globals {
            let default_val = match global.value_type {
                ValueType::I32 => Value::I32(0),
                ValueType::I64 => Value::I64(0),
                ValueType::F32 => Value::F32(0.0),
                ValueType::F64 => Value::F64(0.0),
                _ => return Err(format!("Unsupported global type: {:?}", global.value_type)),
            };
            context.globals.push(default_val);
        }

        for segment in &module.data {
            if segment.offset_expr.is_empty() {
                continue;
            }
            let offset = evaluate_const_expr(&segment.offset_expr)?;
            let end = offset + segment.data.len();
            let mem_size = context.memory.size_bytes();
            if end > mem_size {
                return Err(format!(
                    "Data segment out of bounds: offset={offset}, len={}, memory size={mem_size}",
                    segment.data.len()
                ));
            }
            context.memory.write_bytes(offset, &segment.data)?;
        }

        Ok(Executor {
            context,
            module,
            linker,
            import_func_count,
        })
    }

    /// Execute a function by index and return its results
    pub fn execute(&mut self, func_idx: u32) -> Result<Vec<Value>, String> {
        self.execute_with_args(func_idx, Vec::new())
    }

    /// Execute a function with arguments and return its results
    pub fn execute_with_args(
        &mut self,
        func_idx: u32,
        args: Vec<Value>,
    ) -> Result<Vec<Value>, String> {
        // If func_idx refers to an import, dispatch through the linker
        if (func_idx as usize) < self.import_func_count {
            return self.call_host_function_with_args(func_idx, args);
        }

        let defined_idx = func_idx as usize - self.import_func_count;

        // Get function signature and code (clone to avoid borrow issues)
        let func = {
            let func = self.module.functions.get(defined_idx).ok_or_else(|| {
                format!("Function index {func_idx} out of bounds (defined index {defined_idx})")
            })?;

            let func_type = self
                .module
                .types
                .get(func.type_index as usize)
                .ok_or_else(|| format!("Function type index {} out of bounds", func.type_index))?;

            // Initialize locals: parameters + local variables
            let mut locals = Vec::new();

            // Add parameter slots (initialized with provided arguments or zero)
            for (i, _param_type) in func_type.params.iter().enumerate() {
                if i < args.len() {
                    locals.push(args[i]);
                } else {
                    locals.push(Value::I32(0)); // Placeholder for missing parameters
                }
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
            if cursor.position() >= cursor.get_ref().len() as u64 {
                break;
            }

            let instr = decode_instruction(cursor)?;
            if self.dispatch_instruction(instr, cursor)? == ControlFlow::Return {
                break;
            }
        }
        Ok(())
    }

    /// Skip bytecode until we find the matching else or end instruction
    fn skip_to_else_or_end(&mut self, cursor: &mut Cursor<&[u8]>) -> Result<(), String> {
        let mut depth = 0;

        loop {
            if cursor.position() >= cursor.get_ref().len() as u64 {
                return Err("Unexpected EOF while seeking else/end".to_string());
            }

            let instr = decode_instruction(cursor)?;
            match instr {
                Instruction::Block(_) | Instruction::Loop(_) | Instruction::If(_) => {
                    depth += 1;
                }
                Instruction::Else if depth == 0 => {
                    // Found matching else
                    return Ok(());
                }
                Instruction::End => {
                    if depth == 0 {
                        // Found matching end (no else branch)
                        return Ok(());
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
    }

    /// Skip bytecode until we find the matching end instruction
    fn skip_to_end(&mut self, cursor: &mut Cursor<&[u8]>) -> Result<(), String> {
        self.skip_n_ends(cursor, 1)
    }

    /// Skip past `n` unmatched end instructions in the bytecode
    fn skip_n_ends(&mut self, cursor: &mut Cursor<&[u8]>, n: usize) -> Result<(), String> {
        let mut depth: i32 = 0;
        let mut ends_found: usize = 0;

        loop {
            if cursor.position() >= cursor.get_ref().len() as u64 {
                return Err("Unexpected EOF while seeking end".to_string());
            }

            let instr = decode_instruction(cursor)?;
            match instr {
                Instruction::Block(_) | Instruction::Loop(_) | Instruction::If(_) => {
                    depth += 1;
                }
                Instruction::End => {
                    if depth == 0 {
                        ends_found += 1;
                        if ends_found >= n {
                            return Ok(());
                        }
                    } else {
                        depth -= 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// Execute a branch to the given label depth
    fn do_branch(&mut self, label: u32, cursor: &mut Cursor<&[u8]>) -> Result<(), String> {
        let label_idx = label as usize;
        if label_idx >= self.context.block_stack.len() {
            return Err(format!("br: invalid label {label}"));
        }

        let block_idx = self.context.block_stack.len() - 1 - label_idx;
        let target_block = &self.context.block_stack[block_idx];

        if target_block.is_loop {
            cursor.set_position(target_block.start_pos as u64);
            // Pop only the blocks above the loop (not the loop itself)
            for _ in 0..label_idx {
                self.context.pop_block()?;
            }
        } else {
            // Pop all blocks up to and including the target
            for _ in 0..=label_idx {
                self.context.pop_block()?;
            }
            // Skip past the remaining nested end instructions in bytecode
            self.skip_n_ends(cursor, label_idx + 1)?;
        }
        Ok(())
    }

    /// Call a function with arguments already on stack
    fn call_function(&mut self, func_idx: u32) -> Result<(), String> {
        if (func_idx as usize) < self.import_func_count {
            return self.call_host_function(func_idx);
        }

        let defined_idx = func_idx as usize - self.import_func_count;

        let (arg_count, num_results, code, local_types) = {
            let func = self.module.functions.get(defined_idx).ok_or_else(|| {
                format!("Function index {func_idx} out of bounds (defined index {defined_idx})")
            })?;

            let func_type = self
                .module
                .types
                .get(func.type_index as usize)
                .ok_or_else(|| format!("Function type index {} out of bounds", func.type_index))?;

            (
                func_type.params.len(),
                func_type.results.len(),
                func.code.clone(),
                func.locals.clone(),
            )
        };

        // Pop arguments from operand stack
        let args = self.context.pop_n(arg_count)?;

        // Initialize locals: parameters + local variables
        let mut locals = args;

        // Add local variable slots
        for (count, value_type) in local_types {
            for _ in 0..count {
                let default_value = match value_type {
                    ValueType::I32 => Value::I32(0),
                    ValueType::I64 => Value::I64(0),
                    ValueType::F32 => Value::F32(0.0),
                    ValueType::F64 => Value::F64(0.0),
                    _ => return Err(format!("Unsupported value type in locals: {value_type:?}")),
                };
                locals.push(default_value);
            }
        }

        // Create call frame and push it
        let frame = Frame::new(func_idx, locals, num_results);
        self.context.push_frame(frame);

        // Execute function bytecode
        let mut cursor = Cursor::new(code.as_slice());
        self.execute_bytecode(&mut cursor)?;

        // Pop frame
        self.context.pop_frame()?;

        Ok(())
    }

    /// Call a function indirectly via table lookup
    fn call_function_indirect(&mut self, table_idx: u32, type_idx: u32) -> Result<(), String> {
        if self.module.tables.is_empty() {
            return Err("No tables defined in module".to_string());
        }

        if self.module.elements.is_empty() {
            return Err("No element segments in module".to_string());
        }

        let element_segment = &self.module.elements[0];
        let abs_func_idx = *element_segment
            .function_indices
            .get(table_idx as usize)
            .ok_or_else(|| format!("Table index {table_idx} out of bounds"))?;

        // Resolve defined function index for type checking
        if (abs_func_idx as usize) < self.import_func_count {
            // Imported function via indirect call – look up type from import
            let import = &self.module.imports[abs_func_idx as usize];
            if let ImportKind::Function(import_type_idx) = &import.kind {
                let func_type = self
                    .module
                    .types
                    .get(*import_type_idx as usize)
                    .ok_or_else(|| format!("Import type index {import_type_idx} out of bounds"))?;
                let expected_type = self
                    .module
                    .types
                    .get(type_idx as usize)
                    .ok_or_else(|| format!("Expected type index {type_idx} out of bounds"))?;
                if func_type.params != expected_type.params
                    || func_type.results != expected_type.results
                {
                    return Err("Function signature mismatch in call_indirect".into());
                }
            }
        } else {
            let defined_idx = abs_func_idx as usize - self.import_func_count;
            let func = self
                .module
                .functions
                .get(defined_idx)
                .ok_or_else(|| format!("Function index {abs_func_idx} out of bounds"))?;
            let func_type = self
                .module
                .types
                .get(func.type_index as usize)
                .ok_or_else(|| format!("Function type index {} out of bounds", func.type_index))?;
            let expected_type = self
                .module
                .types
                .get(type_idx as usize)
                .ok_or_else(|| format!("Expected type index {type_idx} out of bounds"))?;
            if func_type.params != expected_type.params
                || func_type.results != expected_type.results
            {
                return Err("Function signature mismatch in call_indirect".into());
            }
        }

        self.call_function(abs_func_idx)?;
        Ok(())
    }

    /// Dispatch an imported function call through the linker.
    /// Arguments are already on the operand stack.
    fn call_host_function(&mut self, func_idx: u32) -> Result<(), String> {
        let idx = func_idx as usize;
        let import = self
            .module
            .imports
            .get(idx)
            .ok_or_else(|| format!("Import index {idx} out of bounds"))?;

        let type_idx = match &import.kind {
            ImportKind::Function(ti) => *ti,
            _ => return Err(format!("Import {idx} is not a function")),
        };

        let (param_count, result_count) = {
            let ft = self
                .module
                .types
                .get(type_idx as usize)
                .ok_or_else(|| format!("Type index {type_idx} out of bounds"))?;
            (ft.params.len(), ft.results.len())
        };

        let module_name = self.module.imports[idx].module.clone();
        let func_name = self.module.imports[idx].name.clone();

        let args = self.context.pop_n(param_count)?;

        let linker = self
            .linker
            .as_ref()
            .ok_or_else(|| format!("No linker: cannot call import {module_name}::{func_name}"))?;
        let host_fn = linker
            .get_import(&module_name, &func_name)
            .ok_or_else(|| format!("Unresolved import: {module_name}::{func_name}"))?;

        let results = host_fn.call(args, &mut self.context.memory)?;

        if results.len() != result_count {
            return Err(format!(
                "Host function {module_name}::{func_name} returned {} values, expected {result_count}",
                results.len()
            ));
        }
        for v in results {
            self.context.push(v);
        }
        Ok(())
    }

    /// Dispatch an imported function call with explicit arguments (for execute_with_args).
    fn call_host_function_with_args(
        &mut self,
        func_idx: u32,
        args: Vec<Value>,
    ) -> Result<Vec<Value>, String> {
        let idx = func_idx as usize;
        let import = self
            .module
            .imports
            .get(idx)
            .ok_or_else(|| format!("Import index {idx} out of bounds"))?;

        let module_name = import.module.clone();
        let func_name = import.name.clone();

        let linker = self
            .linker
            .as_ref()
            .ok_or_else(|| format!("No linker: cannot call import {module_name}::{func_name}"))?;
        let host_fn = linker
            .get_import(&module_name, &func_name)
            .ok_or_else(|| format!("Unresolved import: {module_name}::{func_name}"))?;

        host_fn.call(args, &mut self.context.memory)
    }

    /// Dispatch instruction to handler
    fn dispatch_instruction(
        &mut self,
        instr: Instruction,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ControlFlow, String> {
        match instr {
            // Constants
            Instruction::I32Const(v) => self.context.push(Value::I32(v)),
            Instruction::I64Const(v) => self.context.push(Value::I64(v)),
            Instruction::F32Const(v) => self.context.push(Value::F32(v)),
            Instruction::F64Const(v) => self.context.push(Value::F64(v)),

            // i32 unary operations
            Instruction::I32Eqz => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::I32(if x == 0 { 1 } else { 0 })),
                    _ => return Err("Type mismatch for i32.eqz".to_string()),
                }
            }
            Instruction::I32Clz => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::I32(x.leading_zeros() as i32)),
                    _ => return Err("Type mismatch for i32.clz".to_string()),
                }
            }
            Instruction::I32Ctz => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::I32(x.trailing_zeros() as i32)),
                    _ => return Err("Type mismatch for i32.ctz".to_string()),
                }
            }
            Instruction::I32Popcnt => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::I32(x.count_ones() as i32)),
                    _ => return Err("Type mismatch for i32.popcnt".to_string()),
                }
            }

            // i64 unary operations
            Instruction::I64Eqz => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::I32(if x == 0 { 1 } else { 0 })),
                    _ => return Err("Type mismatch for i64.eqz".to_string()),
                }
            }
            Instruction::I64Clz => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::I64(x.leading_zeros() as i64)),
                    _ => return Err("Type mismatch for i64.clz".to_string()),
                }
            }
            Instruction::I64Ctz => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::I64(x.trailing_zeros() as i64)),
                    _ => return Err("Type mismatch for i64.ctz".to_string()),
                }
            }
            Instruction::I64Popcnt => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::I64(x.count_ones() as i64)),
                    _ => return Err("Type mismatch for i64.popcnt".to_string()),
                }
            }

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

            // i64 arithmetic
            Instruction::I64Add => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I64(x.wrapping_add(y)))
                    }
                    _ => return Err("Type mismatch for i64.add".to_string()),
                }
            }
            Instruction::I64Sub => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I64(x.wrapping_sub(y)))
                    }
                    _ => return Err("Type mismatch for i64.sub".to_string()),
                }
            }
            Instruction::I64Mul => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I64(x.wrapping_mul(y)))
                    }
                    _ => return Err("Type mismatch for i64.mul".to_string()),
                }
            }
            Instruction::I64DivS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        if x == i64::MIN && y == -1 {
                            return Err("Integer overflow in division".to_string());
                        }
                        self.context.push(Value::I64(x / y));
                    }
                    _ => return Err("Type mismatch for i64.div_s".to_string()),
                }
            }
            Instruction::I64DivU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        self.context
                            .push(Value::I64(((x as u64) / (y as u64)) as i64));
                    }
                    _ => return Err("Type mismatch for i64.div_u".to_string()),
                }
            }
            Instruction::I64RemS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        self.context.push(Value::I64(x % y));
                    }
                    _ => return Err("Type mismatch for i64.rem_s".to_string()),
                }
            }
            Instruction::I64RemU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        if y == 0 {
                            return Err("Integer division by zero".to_string());
                        }
                        self.context
                            .push(Value::I64(((x as u64) % (y as u64)) as i64));
                    }
                    _ => return Err("Type mismatch for i64.rem_u".to_string()),
                }
            }

            // i64 bitwise
            Instruction::I64And => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => self.context.push(Value::I64(x & y)),
                    _ => return Err("Type mismatch for i64.and".to_string()),
                }
            }
            Instruction::I64Or => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => self.context.push(Value::I64(x | y)),
                    _ => return Err("Type mismatch for i64.or".to_string()),
                }
            }
            Instruction::I64Xor => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => self.context.push(Value::I64(x ^ y)),
                    _ => return Err("Type mismatch for i64.xor".to_string()),
                }
            }
            Instruction::I64Shl => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I64(x.wrapping_shl(y as u32 & 63)));
                    }
                    _ => return Err("Type mismatch for i64.shl".to_string()),
                }
            }
            Instruction::I64ShrS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I64(x >> (y as u32 & 63)));
                    }
                    _ => return Err("Type mismatch for i64.shr_s".to_string()),
                }
            }
            Instruction::I64ShrU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context
                            .push(Value::I64(((x as u64) >> (y as u32 & 63)) as i64));
                    }
                    _ => return Err("Type mismatch for i64.shr_u".to_string()),
                }
            }
            Instruction::I64Rotl => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        let shift = (y as u32) & 63;
                        self.context.push(Value::I64(x.rotate_left(shift)));
                    }
                    _ => return Err("Type mismatch for i64.rotl".to_string()),
                }
            }
            Instruction::I64Rotr => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        let shift = (y as u32) & 63;
                        self.context.push(Value::I64(x.rotate_right(shift)));
                    }
                    _ => return Err("Type mismatch for i64.rotr".to_string()),
                }
            }

            // i64 comparison
            Instruction::I64Eq => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I32(if x == y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.eq".to_string()),
                }
            }
            Instruction::I64Ne => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I32(if x != y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.ne".to_string()),
                }
            }
            Instruction::I64LtS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I32(if x < y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.lt_s".to_string()),
                }
            }
            Instruction::I64LtU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context
                            .push(Value::I32(if (x as u64) < (y as u64) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.lt_u".to_string()),
                }
            }
            Instruction::I64GtS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I32(if x > y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.gt_s".to_string()),
                }
            }
            Instruction::I64GtU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context
                            .push(Value::I32(if (x as u64) > (y as u64) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.gt_u".to_string()),
                }
            }
            Instruction::I64LeS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I32(if x <= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.le_s".to_string()),
                }
            }
            Instruction::I64LeU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context
                            .push(Value::I32(if (x as u64) <= (y as u64) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.le_u".to_string()),
                }
            }
            Instruction::I64GeS => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context.push(Value::I32(if x >= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.ge_s".to_string()),
                }
            }
            Instruction::I64GeU => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::I64(x), Value::I64(y)) => {
                        self.context
                            .push(Value::I32(if (x as u64) >= (y as u64) { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for i64.ge_u".to_string()),
                }
            }

            // f32 arithmetic
            Instruction::F32Add => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x + y)),
                    _ => return Err("Type mismatch for f32.add".to_string()),
                }
            }
            Instruction::F32Sub => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x - y)),
                    _ => return Err("Type mismatch for f32.sub".to_string()),
                }
            }
            Instruction::F32Mul => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x * y)),
                    _ => return Err("Type mismatch for f32.mul".to_string()),
                }
            }
            Instruction::F32Div => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x / y)),
                    _ => return Err("Type mismatch for f32.div".to_string()),
                }
            }
            Instruction::F32Sqrt => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(x.sqrt())),
                    _ => return Err("Type mismatch for f32.sqrt".to_string()),
                }
            }
            Instruction::F32Min => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x.min(y))),
                    _ => return Err("Type mismatch for f32.min".to_string()),
                }
            }
            Instruction::F32Max => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x.max(y))),
                    _ => return Err("Type mismatch for f32.max".to_string()),
                }
            }
            Instruction::F32Ceil => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(x.ceil())),
                    _ => return Err("Type mismatch for f32.ceil".to_string()),
                }
            }
            Instruction::F32Floor => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(x.floor())),
                    _ => return Err("Type mismatch for f32.floor".to_string()),
                }
            }
            Instruction::F32Trunc => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(x.trunc())),
                    _ => return Err("Type mismatch for f32.trunc".to_string()),
                }
            }
            Instruction::F32Nearest => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(x.round())),
                    _ => return Err("Type mismatch for f32.nearest".to_string()),
                }
            }
            Instruction::F32Abs => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(x.abs())),
                    _ => return Err("Type mismatch for f32.abs".to_string()),
                }
            }
            Instruction::F32Neg => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F32(-x)),
                    _ => return Err("Type mismatch for f32.neg".to_string()),
                }
            }
            Instruction::F32Copysign => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => self.context.push(Value::F32(x.copysign(y))),
                    _ => return Err("Type mismatch for f32.copysign".to_string()),
                }
            }

            // f32 comparison
            Instruction::F32Eq => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::I32(if x == y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f32.eq".to_string()),
                }
            }
            Instruction::F32Ne => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::I32(if x != y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f32.ne".to_string()),
                }
            }
            Instruction::F32Lt => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::I32(if x < y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f32.lt".to_string()),
                }
            }
            Instruction::F32Gt => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::I32(if x > y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f32.gt".to_string()),
                }
            }
            Instruction::F32Le => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::I32(if x <= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f32.le".to_string()),
                }
            }
            Instruction::F32Ge => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::I32(if x >= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f32.ge".to_string()),
                }
            }

            // f64 arithmetic
            Instruction::F64Add => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x + y)),
                    _ => return Err("Type mismatch for f64.add".to_string()),
                }
            }
            Instruction::F64Sub => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x - y)),
                    _ => return Err("Type mismatch for f64.sub".to_string()),
                }
            }
            Instruction::F64Mul => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x * y)),
                    _ => return Err("Type mismatch for f64.mul".to_string()),
                }
            }
            Instruction::F64Div => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x / y)),
                    _ => return Err("Type mismatch for f64.div".to_string()),
                }
            }
            Instruction::F64Sqrt => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(x.sqrt())),
                    _ => return Err("Type mismatch for f64.sqrt".to_string()),
                }
            }
            Instruction::F64Min => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x.min(y))),
                    _ => return Err("Type mismatch for f64.min".to_string()),
                }
            }
            Instruction::F64Max => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x.max(y))),
                    _ => return Err("Type mismatch for f64.max".to_string()),
                }
            }
            Instruction::F64Ceil => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(x.ceil())),
                    _ => return Err("Type mismatch for f64.ceil".to_string()),
                }
            }
            Instruction::F64Floor => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(x.floor())),
                    _ => return Err("Type mismatch for f64.floor".to_string()),
                }
            }
            Instruction::F64Trunc => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(x.trunc())),
                    _ => return Err("Type mismatch for f64.trunc".to_string()),
                }
            }
            Instruction::F64Nearest => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(x.round())),
                    _ => return Err("Type mismatch for f64.nearest".to_string()),
                }
            }
            Instruction::F64Abs => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(x.abs())),
                    _ => return Err("Type mismatch for f64.abs".to_string()),
                }
            }
            Instruction::F64Neg => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F64(-x)),
                    _ => return Err("Type mismatch for f64.neg".to_string()),
                }
            }
            Instruction::F64Copysign => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => self.context.push(Value::F64(x.copysign(y))),
                    _ => return Err("Type mismatch for f64.copysign".to_string()),
                }
            }

            // f64 comparison
            Instruction::F64Eq => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::I32(if x == y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f64.eq".to_string()),
                }
            }
            Instruction::F64Ne => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::I32(if x != y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f64.ne".to_string()),
                }
            }
            Instruction::F64Lt => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::I32(if x < y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f64.lt".to_string()),
                }
            }
            Instruction::F64Gt => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::I32(if x > y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f64.gt".to_string()),
                }
            }
            Instruction::F64Le => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::I32(if x <= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f64.le".to_string()),
                }
            }
            Instruction::F64Ge => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::I32(if x >= y { 1 } else { 0 }));
                    }
                    _ => return Err("Type mismatch for f64.ge".to_string()),
                }
            }

            // Control flow - basic ones first
            Instruction::Nop => {}
            Instruction::Unreachable => return Err("Unreachable instruction executed".to_string()),
            Instruction::Return => return Ok(ControlFlow::Return),
            Instruction::End => {
                // End of block/loop/if - pop the block frame
                if let Ok(block) = self.context.pop_block() {
                    // If block has a result type, the result value should be on stack
                    // Nothing to do here - value stays on stack
                    if block.block_type.is_some() {
                        // Result value is already on the operand stack
                    }
                }
            }
            Instruction::Drop => {
                self.context.pop()?;
            }

            // Call instruction - implement function invocation
            Instruction::Call(func_idx) => {
                self.call_function(func_idx)?;
            }

            // CallIndirect - call function through table
            Instruction::CallIndirect(type_idx) => {
                let func_idx = self.context.pop()?;
                if let Value::I32(idx) = func_idx {
                    self.call_function_indirect(idx as u32, type_idx)?;
                } else {
                    return Err("CallIndirect requires i32 function index on stack".to_string());
                }
            }

            // Global operations
            Instruction::GlobalGet(idx) => {
                let val = self
                    .context
                    .globals
                    .get(idx as usize)
                    .copied()
                    .ok_or_else(|| format!("Global index {idx} out of bounds"))?;
                self.context.push(val);
            }
            Instruction::GlobalSet(idx) => {
                let val = self.context.pop()?;
                let global = self
                    .context
                    .globals
                    .get_mut(idx as usize)
                    .ok_or_else(|| format!("Global index {idx} out of bounds"))?;
                *global = val;
            }

            // Memory load operations
            Instruction::I32Load => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i32(addr)?;
                self.context.push(Value::I32(val));
            }
            Instruction::I64Load => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i64(addr)?;
                self.context.push(Value::I64(val));
            }
            Instruction::F32Load => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_f32(addr)?;
                self.context.push(Value::F32(val));
            }
            Instruction::F64Load => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_f64(addr)?;
                self.context.push(Value::F64(val));
            }
            Instruction::I32Load8S => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i8(addr)? as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I32Load8U => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = (self.context.memory.read_u8(addr)? as u32) as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I32Load16S => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i16(addr)? as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I32Load16U => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = (self.context.memory.read_u16(addr)? as u32) as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I64Load8S => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i8(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load8U => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_u8(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load16S => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i16(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load16U => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_u16(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load32S => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i32(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load32U => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = (self.context.memory.read_i32(addr)? as u32) as i64;
                self.context.push(Value::I64(val));
            }

            // Memory store operations
            Instruction::I32Store => {
                let val = match self.context.pop()? {
                    Value::I32(v) => v,
                    _ => return Err("Value must be i32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_i32(addr, val)?;
            }
            Instruction::I64Store => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_i64(addr, val)?;
            }
            Instruction::F32Store => {
                let val = match self.context.pop()? {
                    Value::F32(v) => v,
                    _ => return Err("Value must be f32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_f32(addr, val)?;
            }
            Instruction::F64Store => {
                let val = match self.context.pop()? {
                    Value::F64(v) => v,
                    _ => return Err("Value must be f64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_f64(addr, val)?;
            }
            Instruction::I32Store8 => {
                let val = match self.context.pop()? {
                    Value::I32(v) => v as u8,
                    _ => return Err("Value must be i32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u8(addr, val)?;
            }
            Instruction::I32Store16 => {
                let val = match self.context.pop()? {
                    Value::I32(v) => v as u16,
                    _ => return Err("Value must be i32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u16(addr, val)?;
            }
            Instruction::I64Store8 => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v as u8,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u8(addr, val)?;
            }
            Instruction::I64Store16 => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v as u16,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u16(addr, val)?;
            }
            Instruction::I64Store32 => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v as u32 as i32,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_i32(addr, val)?;
            }

            // Memory size
            Instruction::MemorySize => {
                let pages = self.context.memory.pages() as i32;
                self.context.push(Value::I32(pages));
            }

            // Memory grow
            Instruction::MemoryGrow => {
                let delta = match self.context.pop()? {
                    Value::I32(n) => n as u32,
                    _ => return Err("Memory grow delta must be i32".to_string()),
                };
                let old_pages = self.context.memory.pages();
                match self.context.memory.grow(delta) {
                    Ok(_) => self.context.push(Value::I32(old_pages as i32)),
                    Err(_) => self.context.push(Value::I32(-1)), // Failure indicated by -1
                }
            }

            // Control flow - proper implementation
            Instruction::Block(block_type) => {
                // Push block frame with current position
                let pos = cursor.position() as usize;
                self.context.push_block(block_type, false, pos, 0, false);
            }
            Instruction::Loop(block_type) => {
                // Push loop frame with current position (for backward branching)
                let pos = cursor.position() as usize;
                self.context.push_block(block_type, true, pos, 0, false);
            }
            Instruction::If(block_type) => {
                // Pop condition from stack
                let cond = self.context.pop()?;
                let cond_value = match cond {
                    Value::I32(v) => v,
                    _ => return Err("if requires i32 condition".to_string()),
                };

                if cond_value != 0 {
                    // Condition is true - execute then-branch
                    self.context.push_block(block_type, false, 0, 0, true);
                } else {
                    // Condition is false - skip to else or end
                    self.context.push_block(block_type, false, 0, 0, false);
                    self.skip_to_else_or_end(cursor)?;
                }
            }
            Instruction::Else => {
                // We're hitting else, which means we executed the then-branch
                // Skip to the matching end
                let block = self.context.current_block()?;
                if block.is_then_branch {
                    // We came from the then-branch, skip to end
                    self.skip_to_end(cursor)?;
                }
                // Else: we came from false condition, continue executing
            }
            Instruction::Br(label) => {
                self.do_branch(label, cursor)?;
            }
            Instruction::BrIf(label) => {
                let cond = self.context.pop()?;
                let cond_value = match cond {
                    Value::I32(v) => v,
                    _ => return Err("br_if requires i32 condition".to_string()),
                };

                if cond_value != 0 {
                    self.do_branch(label, cursor)?;
                }
            }
            Instruction::BrTable(targets, default) => {
                let index = match self.context.pop()? {
                    Value::I32(v) => v as u32,
                    _ => return Err("br_table index must be i32".to_string()),
                };

                let label = if (index as usize) < targets.len() {
                    targets[index as usize]
                } else {
                    default
                };

                self.do_branch(label, cursor)?;
            }

            // Type conversions
            Instruction::I32WrapI64 => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::I32(x as i32)),
                    _ => return Err("Type mismatch for i32.wrap_i64".to_string()),
                }
            }
            Instruction::I32TruncF32S => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (i32::MAX as f32 + 1.0) || x < (i32::MIN as f32) {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I32(x as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f32_s".to_string()),
                }
            }
            Instruction::I32TruncF32U => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (u32::MAX as f32 + 1.0) || x < 0.0 {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I32(x as u32 as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f32_u".to_string()),
                }
            }
            Instruction::I32TruncF64S => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (i32::MAX as f64 + 1.0) || x < (i32::MIN as f64) {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I32(x as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f64_s".to_string()),
                }
            }
            Instruction::I32TruncF64U => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (u32::MAX as f64 + 1.0) || x < 0.0 {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I32(x as u32 as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f64_u".to_string()),
                }
            }
            Instruction::I64ExtendI32S => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::I64(x as i64)),
                    _ => return Err("Type mismatch for i64.extend_i32_s".to_string()),
                }
            }
            Instruction::I64ExtendI32U => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::I64(x as u32 as i64)),
                    _ => return Err("Type mismatch for i64.extend_i32_u".to_string()),
                }
            }
            Instruction::I64TruncF32S => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (i64::MAX as f32) || x < (i64::MIN as f32) {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I64(x as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f32_s".to_string()),
                }
            }
            Instruction::I64TruncF32U => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (u64::MAX as f32) || x < 0.0 {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I64(x as u64 as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f32_u".to_string()),
                }
            }
            Instruction::I64TruncF64S => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (i64::MAX as f64) || x < (i64::MIN as f64) {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I64(x as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f64_s".to_string()),
                }
            }
            Instruction::I64TruncF64U => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        if x.is_nan() {
                            return Err("Invalid conversion to integer: NaN".to_string());
                        }
                        if x >= (u64::MAX as f64) || x < 0.0 {
                            return Err("Integer overflow in truncation".to_string());
                        }
                        self.context.push(Value::I64(x as u64 as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f64_u".to_string()),
                }
            }
            Instruction::F32ConvertI32S => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::F32(x as f32)),
                    _ => return Err("Type mismatch for f32.convert_i32_s".to_string()),
                }
            }
            Instruction::F32ConvertI32U => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::F32(x as u32 as f32)),
                    _ => return Err("Type mismatch for f32.convert_i32_u".to_string()),
                }
            }
            Instruction::F32ConvertI64S => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::F32(x as f32)),
                    _ => return Err("Type mismatch for f32.convert_i64_s".to_string()),
                }
            }
            Instruction::F32ConvertI64U => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::F32(x as u64 as f32)),
                    _ => return Err("Type mismatch for f32.convert_i64_u".to_string()),
                }
            }
            Instruction::F32DemoteF64 => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::F32(x as f32)),
                    _ => return Err("Type mismatch for f32.demote_f64".to_string()),
                }
            }
            Instruction::F64ConvertI32S => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::F64(x as f64)),
                    _ => return Err("Type mismatch for f64.convert_i32_s".to_string()),
                }
            }
            Instruction::F64ConvertI32U => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => self.context.push(Value::F64(x as u32 as f64)),
                    _ => return Err("Type mismatch for f64.convert_i32_u".to_string()),
                }
            }
            Instruction::F64ConvertI64S => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::F64(x as f64)),
                    _ => return Err("Type mismatch for f64.convert_i64_s".to_string()),
                }
            }
            Instruction::F64ConvertI64U => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => self.context.push(Value::F64(x as u64 as f64)),
                    _ => return Err("Type mismatch for f64.convert_i64_u".to_string()),
                }
            }
            Instruction::F64PromoteF32 => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::F64(x as f64)),
                    _ => return Err("Type mismatch for f64.promote_f32".to_string()),
                }
            }
            Instruction::I32Reinterpret => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => self.context.push(Value::I32(x.to_bits() as i32)),
                    _ => return Err("Type mismatch for i32.reinterpret_f32".to_string()),
                }
            }
            Instruction::I64Reinterpret => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => self.context.push(Value::I64(x.to_bits() as i64)),
                    _ => return Err("Type mismatch for i64.reinterpret_f64".to_string()),
                }
            }
            Instruction::F32Reinterpret => {
                let a = self.context.pop()?;
                match a {
                    Value::I32(x) => {
                        self.context.push(Value::F32(f32::from_bits(x as u32)));
                    }
                    _ => return Err("Type mismatch for f32.reinterpret_i32".to_string()),
                }
            }
            Instruction::F64Reinterpret => {
                let a = self.context.pop()?;
                match a {
                    Value::I64(x) => {
                        self.context.push(Value::F64(f64::from_bits(x as u64)));
                    }
                    _ => return Err("Type mismatch for f64.reinterpret_i64".to_string()),
                }
            }
            Instruction::Select => {
                let cond = self.context.pop()?;
                let val2 = self.context.pop()?;
                let val1 = self.context.pop()?;
                match cond {
                    Value::I32(c) => {
                        self.context.push(if c != 0 { val1 } else { val2 });
                    }
                    _ => return Err("Select condition must be i32".to_string()),
                }
            }
        }

        Ok(ControlFlow::Continue)
    }

    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut ExecutionContext {
        &mut self.context
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn module_mut(&mut self) -> &mut Module {
        &mut self.module
    }

    pub fn import_func_count(&self) -> usize {
        self.import_func_count
    }

    /// Check whether an error string represents a WASI proc_exit.
    pub fn is_proc_exit(err: &str) -> Option<i32> {
        err.strip_prefix(WASI_PROC_EXIT_PREFIX)
            .and_then(|code| code.parse().ok())
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

    #[test]
    fn test_i64_add() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I64(100));
        ctx.push(Value::I64(42));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I64(x), Value::I64(y)) => ctx.push(Value::I64(x.wrapping_add(y))),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I64(142));
    }

    #[test]
    fn test_i64_sub() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I64(100));
        ctx.push(Value::I64(42));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I64(x), Value::I64(y)) => ctx.push(Value::I64(x.wrapping_sub(y))),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I64(58));
    }

    #[test]
    fn test_i64_mul() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I64(6));
        ctx.push(Value::I64(7));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I64(x), Value::I64(y)) => ctx.push(Value::I64(x.wrapping_mul(y))),
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I64(42));
    }

    #[test]
    fn test_i64_div_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I64(84));
        ctx.push(Value::I64(2));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I64(x), Value::I64(y)) => {
                if y != 0 {
                    ctx.push(Value::I64(x / y));
                }
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I64(42));
    }

    #[test]
    fn test_i64_eq() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I64(42));
        ctx.push(Value::I64(42));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I64(x), Value::I64(y)) => {
                ctx.push(Value::I32(if x == y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_i64_lt_s() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::I64(10));
        ctx.push(Value::I64(42));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::I64(x), Value::I64(y)) => {
                ctx.push(Value::I32(if x < y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_f32_add() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::F32(1.5));
        ctx.push(Value::F32(2.5));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::F32(x), Value::F32(y)) => ctx.push(Value::F32(x + y)),
            _ => panic!("Type mismatch"),
        }

        match ctx.pop().unwrap() {
            Value::F32(x) => assert!((x - 4.0).abs() < 0.001),
            _ => panic!("Expected f32"),
        }
    }

    #[test]
    fn test_f32_mul() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::F32(2.0));
        ctx.push(Value::F32(3.0));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::F32(x), Value::F32(y)) => ctx.push(Value::F32(x * y)),
            _ => panic!("Type mismatch"),
        }

        match ctx.pop().unwrap() {
            Value::F32(x) => assert!((x - 6.0).abs() < 0.001),
            _ => panic!("Expected f32"),
        }
    }

    #[test]
    fn test_f32_eq() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::F32(std::f32::consts::PI));
        ctx.push(Value::F32(std::f32::consts::PI));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::F32(x), Value::F32(y)) => {
                ctx.push(Value::I32(if x == y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_f64_add() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::F64(1.5));
        ctx.push(Value::F64(2.5));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::F64(x), Value::F64(y)) => ctx.push(Value::F64(x + y)),
            _ => panic!("Type mismatch"),
        }

        match ctx.pop().unwrap() {
            Value::F64(x) => assert!((x - 4.0).abs() < 0.001),
            _ => panic!("Expected f64"),
        }
    }

    #[test]
    fn test_f64_div() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::F64(10.0));
        ctx.push(Value::F64(2.0));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::F64(x), Value::F64(y)) => ctx.push(Value::F64(x / y)),
            _ => panic!("Type mismatch"),
        }

        match ctx.pop().unwrap() {
            Value::F64(x) => assert!((x - 5.0).abs() < 0.001),
            _ => panic!("Expected f64"),
        }
    }

    #[test]
    fn test_f64_lt() {
        let mut ctx = ExecutionContext::new(1, None).unwrap();
        ctx.push(Value::F64(1.5));
        ctx.push(Value::F64(2.5));

        let b = ctx.pop().unwrap();
        let a = ctx.pop().unwrap();
        match (a, b) {
            (Value::F64(x), Value::F64(y)) => {
                ctx.push(Value::I32(if x < y { 1 } else { 0 }));
            }
            _ => panic!("Type mismatch"),
        }

        assert_eq!(ctx.pop().unwrap(), Value::I32(1));
    }

    #[test]
    fn test_function_call_simple() {
        use crate::runtime::core::module::{Function, FunctionType};

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![ValueType::I32, ValueType::I32],
                results: vec![ValueType::I32],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code: vec![0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b],
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(10));
        executor.context.push(Value::I32(5));

        executor.call_function(0).unwrap();

        let result = executor.context.pop().unwrap();
        assert_eq!(result, Value::I32(15));
    }

    #[test]
    fn test_function_call_with_locals() {
        use crate::runtime::core::module::{Function, FunctionType};

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![ValueType::I32],
                results: vec![ValueType::I32],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![(1, ValueType::I32)],
                code: vec![0x41, 0x05, 0x21, 0x01, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b],
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(10));

        executor.call_function(0).unwrap();

        let result = executor.context.pop().unwrap();
        assert_eq!(result, Value::I32(15));
    }

    #[test]
    fn test_multiple_return_values() {
        use crate::runtime::core::module::{Function, FunctionType};

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![ValueType::I32, ValueType::I32],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code: vec![0x41, 0x0a, 0x41, 0x05, 0x0b],
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Value::I32(10));
        assert_eq!(results[1], Value::I32(5));
    }

    // Global operations tests
    #[test]
    fn test_global_get_set() {
        use crate::runtime::core::module::{Function, FunctionType, GlobalValue};

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![ValueType::I32],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code: vec![0x23, 0x00, 0x0b],
            }],
            tables: vec![],
            memory: None,
            globals: vec![GlobalValue {
                mutable: true,
                value_type: ValueType::I32,
                init_expr: vec![],
            }],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();
        executor.context.globals[0] = Value::I32(42);

        let results = executor.execute(0).unwrap();
        assert_eq!(results[0], Value::I32(42));
    }

    // Memory operations tests via context
    #[test]
    fn test_memory_direct_i32_store_load() {
        use crate::runtime::core::module::MemoryType;

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: None,
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();

        executor.context.memory.write_i32(100, 12345).unwrap();
        let val = executor.context.memory.read_i32(100).unwrap();
        assert_eq!(val, 12345);
    }

    #[test]
    fn test_memory_size() {
        use crate::runtime::core::module::MemoryType;

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 3,
                max: Some(5),
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let executor = Executor::new(module).unwrap();
        assert_eq!(executor.context.memory.pages(), 3);
    }

    #[test]
    fn test_memory_i8_operations() {
        use crate::runtime::core::module::MemoryType;

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: None,
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();

        executor.context.memory.write_i8(50, -42).unwrap();
        let val = executor.context.memory.read_i8(50).unwrap();
        assert_eq!(val, -42);

        executor.context.memory.write_u8(100, 255).unwrap();
        let val = executor.context.memory.read_u8(100).unwrap();
        assert_eq!(val, 255);
    }

    #[test]
    fn test_memory_i16_operations() {
        use crate::runtime::core::module::MemoryType;

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: None,
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();

        executor.context.memory.write_i16(200, -1000).unwrap();
        let val = executor.context.memory.read_i16(200).unwrap();
        assert_eq!(val, -1000);

        executor.context.memory.write_u16(300, 65535).unwrap();
        let val = executor.context.memory.read_u16(300).unwrap();
        assert_eq!(val, 65535);
    }

    // ===== v0.16.0 Tests: Data Section Initialization =====

    #[test]
    fn test_data_segment_string_constant() {
        use crate::runtime::core::module::{DataSegment, MemoryType};

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: None,
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![DataSegment {
                offset_expr: vec![0x41, 0x10, 0x0B], // i32.const 16, end
                data: b"Hello, WASM!".to_vec(),
            }],
        };

        let executor = Executor::new(module).unwrap();
        let bytes = executor.context.memory.read_bytes(16, 12).unwrap();
        assert_eq!(&bytes, b"Hello, WASM!");
    }

    #[test]
    fn test_data_segment_multiple() {
        use crate::runtime::core::module::{DataSegment, MemoryType};

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: None,
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![
                DataSegment {
                    offset_expr: vec![0x41, 0x00, 0x0B], // i32.const 0
                    data: vec![0xDE, 0xAD],
                },
                DataSegment {
                    offset_expr: vec![0x41, 0x10, 0x0B], // i32.const 16
                    data: vec![0xBE, 0xEF],
                },
            ],
        };

        let executor = Executor::new(module).unwrap();
        assert_eq!(executor.context.memory.read_u8(0).unwrap(), 0xDE);
        assert_eq!(executor.context.memory.read_u8(1).unwrap(), 0xAD);
        assert_eq!(executor.context.memory.read_u8(16).unwrap(), 0xBE);
        assert_eq!(executor.context.memory.read_u8(17).unwrap(), 0xEF);
    }

    #[test]
    fn test_data_segment_out_of_bounds() {
        use crate::runtime::core::module::{DataSegment, MemoryType};

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: Some(1),
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![DataSegment {
                offset_expr: vec![0x41, 0xFF, 0xFF, 0x03, 0x0B], // i32.const 65535
                data: vec![0x00, 0x01], // 2 bytes at offset 65535 overflows 1 page
            }],
        };

        let result = Executor::new(module);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_segment_passive_skipped() {
        use crate::runtime::core::module::{DataSegment, MemoryType};

        let module = Module {
            version: 1,
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memory: Some(MemoryType {
                initial: 1,
                max: None,
            }),
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![DataSegment {
                offset_expr: vec![], // passive segment (empty offset)
                data: vec![0xFF; 100],
            }],
        };

        let executor = Executor::new(module).unwrap();
        assert_eq!(executor.context.memory.read_u8(0).unwrap(), 0x00);
    }

    // ===== v0.16.0 Tests: Type Conversion Instructions =====

    #[test]
    fn test_i32_wrap_i64() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I64(0x1_0000_002A)); // wraps to 42
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xA7, 0x0B]; // i32.wrap_i64, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::I32(42));
    }

    #[test]
    fn test_i64_extend_i32_s() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(-1));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xAC, 0x0B]; // i64.extend_i32_s, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::I64(-1));
    }

    #[test]
    fn test_i64_extend_i32_u() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(-1)); // 0xFFFFFFFF unsigned
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xAD, 0x0B]; // i64.extend_i32_u, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::I64(0xFFFF_FFFF_i64));
    }

    #[test]
    fn test_f32_convert_i32_s() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(-42));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xB2, 0x0B]; // f32.convert_i32_s, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::F32(-42.0));
    }

    #[test]
    fn test_f64_promote_f32() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::F32(1.5));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xBB, 0x0B]; // f64.promote_f32, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        match executor.context.pop().unwrap() {
            Value::F64(x) => assert!((x - 1.5).abs() < 0.001),
            other => panic!("Expected F64, got {other:?}"),
        }
    }

    #[test]
    fn test_f32_demote_f64() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::F64(2.5));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xB6, 0x0B]; // f32.demote_f64, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        match executor.context.pop().unwrap() {
            Value::F32(x) => assert!((x - 2.5).abs() < 0.001),
            other => panic!("Expected F32, got {other:?}"),
        }
    }

    #[test]
    fn test_i32_reinterpret_f32() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::F32(1.0));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xBC, 0x0B]; // i32.reinterpret_f32, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(
            executor.context.pop().unwrap(),
            Value::I32(0x3F80_0000_u32 as i32)
        ); // IEEE 754 for 1.0
    }

    #[test]
    fn test_f32_reinterpret_i32() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(0x3F80_0000_u32 as i32));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xBE, 0x0B]; // f32.reinterpret_i32, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::F32(1.0));
    }

    #[test]
    fn test_i32_trunc_f64_s_nan() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::F64(f64::NAN));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xAA, 0x0B]; // i32.trunc_f64_s, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        let result = executor.execute_bytecode(&mut cursor);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("NaN"));
    }

    #[test]
    fn test_i32_trunc_f32_u_overflow() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::F32(-1.0));
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0xA9, 0x0B]; // i32.trunc_f32_u, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        let result = executor.execute_bytecode(&mut cursor);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_select_true() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(10)); // val1
        executor.context.push(Value::I32(20)); // val2
        executor.context.push(Value::I32(1)); // cond (true)
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0x1B, 0x0B]; // select, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::I32(10));
    }

    #[test]
    fn test_select_false() {
        let module = Module::new();
        let mut executor = Executor::new(module).unwrap();
        executor.context.push(Value::I32(10)); // val1
        executor.context.push(Value::I32(20)); // val2
        executor.context.push(Value::I32(0)); // cond (false)
        let frame = Frame::new(0, vec![], 0);
        executor.context.push_frame(frame);

        let bytecode = vec![0x1B, 0x0B]; // select, end
        let mut cursor = Cursor::new(bytecode.as_slice());
        executor.execute_bytecode(&mut cursor).unwrap();
        executor.context.pop_frame().unwrap();
        assert_eq!(executor.context.pop().unwrap(), Value::I32(20));
    }

    // ===== v0.16.0 Tests: br_table =====

    #[test]
    fn test_br_table_case0() {
        use crate::runtime::core::module::{Function, FunctionType};

        // block $b0
        //   block $b1
        //     block $b2
        //       local.get 0
        //       br_table 0 1 2   ;; targets: [$b2, $b1, $b0], default=$b0
        //     end $b2
        //     i32.const 20
        //     return
        //   end $b1
        //   i32.const 10
        //   return
        // end $b0
        // i32.const 30
        let code = vec![
            0x02, 0x40, // block void
            0x02, 0x40, // block void
            0x02, 0x40, // block void
            0x20, 0x00, // local.get 0
            0x0E, 0x02, 0x00, 0x01, 0x02, // br_table [0, 1] default=2
            0x0B, // end (innermost)
            0x41, 0x14, // i32.const 20
            0x0F, // return
            0x0B, // end (middle)
            0x41, 0x0A, // i32.const 10
            0x0F, // return
            0x0B, // end (outer)
            0x41, 0x1E, // i32.const 30
            0x0B, // end (function)
        ];

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![ValueType::I32],
                results: vec![ValueType::I32],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code,
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();

        // Case 0 → falls through innermost block → returns 20
        let results = executor.execute_with_args(0, vec![Value::I32(0)]).unwrap();
        assert_eq!(results[0], Value::I32(20));
    }

    #[test]
    fn test_br_table_default() {
        use crate::runtime::core::module::{Function, FunctionType};

        let code = vec![
            0x02, 0x40, // block void
            0x02, 0x40, // block void
            0x02, 0x40, // block void
            0x20, 0x00, // local.get 0
            0x0E, 0x02, 0x00, 0x01, 0x02, // br_table [0, 1] default=2
            0x0B, // end (innermost)
            0x41, 0x14, // i32.const 20
            0x0F, // return
            0x0B, // end (middle)
            0x41, 0x0A, // i32.const 10
            0x0F, // return
            0x0B, // end (outer)
            0x41, 0x1E, // i32.const 30
            0x0B, // end (function)
        ];

        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![ValueType::I32],
                results: vec![ValueType::I32],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code,
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: std::collections::HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };

        let mut executor = Executor::new(module).unwrap();

        // Out-of-range index (99) → uses default (label 2 → outer block) → returns 30
        let results = executor.execute_with_args(0, vec![Value::I32(99)]).unwrap();
        assert_eq!(results[0], Value::I32(30));
    }

    #[test]
    fn test_evaluate_const_expr_i32() {
        let expr = vec![0x41, 0x2A, 0x0B]; // i32.const 42, end
        assert_eq!(evaluate_const_expr(&expr).unwrap(), 42);
    }

    #[test]
    fn test_evaluate_const_expr_i64() {
        let expr = vec![0x42, 0x80, 0x01, 0x0B]; // i64.const 128, end
        assert_eq!(evaluate_const_expr(&expr).unwrap(), 128);
    }

    #[test]
    fn test_decode_type_conversion_opcodes() {
        let cases: Vec<(u8, &str)> = vec![
            (0xA7, "I32WrapI64"),
            (0xA8, "I32TruncF32S"),
            (0xAC, "I64ExtendI32S"),
            (0xAD, "I64ExtendI32U"),
            (0xB2, "F32ConvertI32S"),
            (0xB6, "F32DemoteF64"),
            (0xB7, "F64ConvertI32S"),
            (0xBB, "F64PromoteF32"),
            (0xBC, "I32Reinterpret"),
            (0xBD, "I64Reinterpret"),
            (0xBE, "F32Reinterpret"),
            (0xBF, "F64Reinterpret"),
        ];
        for (opcode, name) in cases {
            let bytecode = vec![opcode];
            let mut cursor = Cursor::new(bytecode.as_slice());
            let instr = decode_instruction(&mut cursor);
            assert!(
                instr.is_ok(),
                "Failed to decode opcode 0x{opcode:02X} ({name})"
            );
        }
    }

    #[test]
    fn test_decode_f32_unary_opcodes() {
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x8B].as_slice())).unwrap(),
            Instruction::F32Abs
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x8C].as_slice())).unwrap(),
            Instruction::F32Neg
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x91].as_slice())).unwrap(),
            Instruction::F32Sqrt
        ));
    }

    #[test]
    fn test_decode_f64_unary_opcodes() {
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x99].as_slice())).unwrap(),
            Instruction::F64Abs
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x9F].as_slice())).unwrap(),
            Instruction::F64Sqrt
        ));
    }
}
