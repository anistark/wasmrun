/// WASM instruction executor
/// Handles execution context, stack, call frames, and instruction dispatch
use super::linker::Linker;
use super::memory::LinearMemory;
use super::module::{ImportKind, Module, ValueType};
use super::values::Value;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Sentinel prefix for proc_exit errors so callers can extract the exit code.
pub const WASI_PROC_EXIT_PREFIX: &str = "__wasi_proc_exit:";

/// Sentinel error returned when an execution exhausts its instruction budget
/// ("fuel"). Callers can detect this to surface a clear resource-limit error.
pub const FUEL_EXHAUSTED_ERROR: &str = "__wasmrun_fuel_exhausted__";

/// Default ceiling on how deeply guest code may nest calls.
///
/// The interpreter runs each guest call on a host stack frame of its own, so
/// unbounded guest recursion overflows the *host* stack. That is not a
/// catchable panic: the process aborts, which in agent mode takes the server
/// down for every tenant rather than failing the one request.
///
/// The number is calibrated for a release build, where a guest frame costs
/// about 1.1 KB of host stack: 1024 of them fit inside the 2 MB a spawned
/// worker thread gets, with room to spare. An unoptimized build is a different
/// machine entirely, with frames around 70 KB, so only a few dozen fit and this
/// ceiling is far too high to catch anything there. Code that deliberately
/// recurses under `cargo test` should set its own limit with
/// [`Executor::set_max_call_depth`] rather than rely on this one.
pub const DEFAULT_MAX_CALL_DEPTH: usize = 1024;

/// Sentinel error returned when an execution exceeds its call-depth ceiling.
/// The spec calls this "call stack exhausted"; it is a trap, not a crash.
pub const CALL_DEPTH_EXCEEDED_ERROR: &str = "call stack exhausted";

/// Sentinel error returned when an execution is cancelled via its cancel token
/// (e.g. the agent server tripping it on wall-clock timeout). Callers detect
/// this to distinguish a deliberate halt from a program error.
pub const EXECUTION_CANCELLED_ERROR: &str = "__wasmrun_execution_cancelled__";

/// Result of instruction dispatch for control flow signaling
#[derive(Debug, Clone, PartialEq)]
enum ControlFlow {
    Continue,
    Return,
}

/// WASM instruction representation
/// Covers all instruction types from the WebAssembly specification
#[derive(Debug, Clone, PartialEq)]
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

    // Memory — each carries the offset immediate from the instruction encoding.
    // Effective address = stack_addr + offset.
    I32Load(u32),
    I64Load(u32),
    F32Load(u32),
    F64Load(u32),
    I32Load8S(u32),
    I32Load8U(u32),
    I32Load16S(u32),
    I32Load16U(u32),
    I64Load8S(u32),
    I64Load8U(u32),
    I64Load16S(u32),
    I64Load16U(u32),
    I64Load32S(u32),
    I64Load32U(u32),
    I32Store(u32),
    I64Store(u32),
    F32Store(u32),
    F64Store(u32),
    I32Store8(u32),
    I32Store16(u32),
    I64Store8(u32),
    I64Store16(u32),
    I64Store32(u32),
    MemorySize,
    MemoryGrow,

    // Sign-extension operators (WASM sign-extension proposal)
    I32Extend8S,
    I32Extend16S,
    I64Extend8S,
    I64Extend16S,
    I64Extend32S,

    // Saturating float-to-int truncation (0xFC prefix) — WASM
    // non-trapping-float-to-int proposal
    I32TruncSatF32S,
    I32TruncSatF32U,
    I32TruncSatF64S,
    I32TruncSatF64U,
    I64TruncSatF32S,
    I64TruncSatF32U,
    I64TruncSatF64S,
    I64TruncSatF64U,

    // Bulk-memory (0xFC prefix) — WASM bulk-memory extension
    MemoryCopy,
    MemoryFill,
    MemoryInit(u32), // data segment index
    DataDrop(u32),   // data segment index

    // Local/Global
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    GlobalGet(u32),
    GlobalSet(u32),

    // Reference types (WASM reference-types proposal)
    RefNull(ValueType), // null reference of the given ref type (funcref/externref)
    RefIsNull,
    RefFunc(u32), // function index

    // Table operations (reference-types / bulk-memory proposals)
    TableGet(u32),       // table index
    TableSet(u32),       // table index
    TableInit(u32, u32), // (element segment index, table index)
    ElemDrop(u32),       // element segment index
    TableCopy(u32, u32), // (dst table index, src table index)
    TableGrow(u32),      // table index
    TableSize(u32),      // table index
    TableFill(u32),      // table index

    // Control flow
    Nop,
    Unreachable,
    Block(BlockType),
    Loop(BlockType),
    If(BlockType),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable(Vec<u32>, u32),
    Return,
    Call(u32),
    CallIndirect(u32, u32),
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

/// The type of a `block`, `loop` or `if`.
///
/// Three encodings share one field in the binary: `0x40` for a block that
/// produces nothing, a value type byte for the single-result shorthand, and a
/// non-negative index into the type section for the general form, which is the
/// only one that can carry parameters or more than one result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    Empty,
    Value(ValueType),
    FuncType(u32),
}

/// Decode block type (for block, loop, if instructions).
///
/// The field is a signed LEB128 in the s33 range: negative values are the
/// single-byte shorthands, non-negative values are type indices. A type index
/// above 63 needs more than one byte, so it cannot be read as a plain byte.
fn decode_block_type(cursor: &mut Cursor<&[u8]>) -> Result<BlockType, String> {
    let raw = decode_s33_leb128(cursor)?;
    if raw >= 0 {
        return Ok(BlockType::FuncType(raw as u32));
    }
    // Negative: the low seven bits are the value type byte, 0x40 meaning empty.
    let byte = (raw & 0x7F) as u8;
    match byte {
        0x40 => Ok(BlockType::Empty),
        _ => ValueType::from_byte(byte)
            .map(BlockType::Value)
            .ok_or_else(|| format!("Invalid block type: 0x{byte:02X}")),
    }
}

/// Decode a signed LEB128 in the s33 range, used only by the block type field.
fn decode_s33_leb128(cursor: &mut Cursor<&[u8]>) -> Result<i64, String> {
    let mut result: i64 = 0;
    let mut shift = 0;
    loop {
        let byte = read_u8(cursor)?;
        result |= ((byte & 0x7F) as i64) << shift;
        shift += 7;
        if (byte & 0x80) == 0 {
            if shift < 64 && (byte & 0x40) != 0 {
                result |= -1i64 << shift;
            }
            return Ok(result);
        }
        if shift >= 35 {
            return Err("LEB128 value too large for a block type".to_string());
        }
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
            // Sign extend if the sign bit in the final LEB128 byte is set and
            // shift+7 is still within the i32 range. At shift=28, the 5th byte
            // already contributes bit 31 directly, so no extension is needed.
            if shift + 7 < 32 && (byte & 0x40) != 0 {
                result |= (-1i32) << (shift + 7);
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
            // Sign extend if the sign bit in the final LEB128 byte is set and
            // shift+7 is still within the i64 range.
            if shift + 7 < 64 && (byte & 0x40) != 0 {
                result |= (-1i64) << (shift + 7);
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
        // offset is an immediate added to the stack address: effective_addr = stack_val + offset
        0x28 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load(offset))
        }
        0x29 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load(offset))
        }
        0x2A => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F32Load(offset))
        }
        0x2B => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F64Load(offset))
        }
        0x2C => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load8S(offset))
        }
        0x2D => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load8U(offset))
        }
        0x2E => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load16S(offset))
        }
        0x2F => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Load16U(offset))
        }
        0x30 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load8S(offset))
        }
        0x31 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load8U(offset))
        }
        0x32 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load16S(offset))
        }
        0x33 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load16U(offset))
        }
        0x34 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load32S(offset))
        }
        0x35 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Load32U(offset))
        }
        0x36 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Store(offset))
        }
        0x37 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store(offset))
        }
        0x38 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F32Store(offset))
        }
        0x39 => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::F64Store(offset))
        }
        0x3A => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Store8(offset))
        }
        0x3B => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I32Store16(offset))
        }
        0x3C => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store8(offset))
        }
        0x3D => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store16(offset))
        }
        0x3E => {
            let _align = decode_u32_leb128(cursor)?;
            let offset = decode_u32_leb128(cursor)?;
            Ok(Instruction::I64Store32(offset))
        }
        0x3F => {
            // memory.size - memory index, always 0 with a single memory
            let _mem_idx = decode_u32_leb128(cursor)?;
            Ok(Instruction::MemorySize)
        }
        0x40 => {
            // memory.grow - memory index, always 0 with a single memory
            let _mem_idx = decode_u32_leb128(cursor)?;
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
            let table_idx = decode_u32_leb128(cursor)?;
            Ok(Instruction::CallIndirect(type_idx, table_idx))
        }
        0x1A => Ok(Instruction::Drop),
        0x1B => Ok(Instruction::Select),
        0x1C => {
            // select with explicit result types (reference-types proposal).
            // The type annotation only matters for validation; runtime behavior
            // is identical to untyped select, so decode and discard the types.
            let n = decode_u32_leb128(cursor)? as usize;
            for _ in 0..n {
                let _ty = read_u8(cursor)?;
            }
            Ok(Instruction::Select)
        }

        // Reference types
        0x25 => Ok(Instruction::TableGet(decode_u32_leb128(cursor)?)),
        0x26 => Ok(Instruction::TableSet(decode_u32_leb128(cursor)?)),
        0xD0 => {
            // ref.null <reftype>
            let ty = read_u8(cursor)?;
            let ref_type = ValueType::from_byte(ty)
                .filter(|t| matches!(t, ValueType::FuncRef | ValueType::ExternRef))
                .ok_or_else(|| format!("Invalid ref.null type: 0x{ty:02X}"))?;
            Ok(Instruction::RefNull(ref_type))
        }
        0xD1 => Ok(Instruction::RefIsNull),
        0xD2 => Ok(Instruction::RefFunc(decode_u32_leb128(cursor)?)),

        // Sign-extension operators (WASM sign-extension proposal)
        0xC0 => Ok(Instruction::I32Extend8S),
        0xC1 => Ok(Instruction::I32Extend16S),
        0xC2 => Ok(Instruction::I64Extend8S),
        0xC3 => Ok(Instruction::I64Extend16S),
        0xC4 => Ok(Instruction::I64Extend32S),

        // Bulk-memory prefix byte (WASM bulk-memory extension)
        0xFC => {
            let op = decode_u32_leb128(cursor)?;
            match op {
                0 => Ok(Instruction::I32TruncSatF32S),
                1 => Ok(Instruction::I32TruncSatF32U),
                2 => Ok(Instruction::I32TruncSatF64S),
                3 => Ok(Instruction::I32TruncSatF64U),
                4 => Ok(Instruction::I64TruncSatF32S),
                5 => Ok(Instruction::I64TruncSatF32U),
                6 => Ok(Instruction::I64TruncSatF64S),
                7 => Ok(Instruction::I64TruncSatF64U),
                8 => {
                    // memory.init: seg_idx mem_idx
                    let seg_idx = decode_u32_leb128(cursor)?;
                    let _mem_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::MemoryInit(seg_idx))
                }
                9 => {
                    // data.drop: seg_idx
                    let seg_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::DataDrop(seg_idx))
                }
                10 => {
                    // memory.copy: dst_mem src_mem
                    let _dst = decode_u32_leb128(cursor)?;
                    let _src = decode_u32_leb128(cursor)?;
                    Ok(Instruction::MemoryCopy)
                }
                11 => {
                    // memory.fill: mem_idx
                    let _mem_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::MemoryFill)
                }
                12 => {
                    // table.init: elem_idx table_idx
                    let elem_idx = decode_u32_leb128(cursor)?;
                    let table_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::TableInit(elem_idx, table_idx))
                }
                13 => {
                    // elem.drop: elem_idx
                    let elem_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::ElemDrop(elem_idx))
                }
                14 => {
                    // table.copy: dst_table src_table
                    let dst = decode_u32_leb128(cursor)?;
                    let src = decode_u32_leb128(cursor)?;
                    Ok(Instruction::TableCopy(dst, src))
                }
                15 => {
                    // table.grow: table_idx
                    let table_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::TableGrow(table_idx))
                }
                16 => {
                    // table.size: table_idx
                    let table_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::TableSize(table_idx))
                }
                17 => {
                    // table.fill: table_idx
                    let table_idx = decode_u32_leb128(cursor)?;
                    Ok(Instruction::TableFill(table_idx))
                }
                _ => Err(format!("Unknown 0xFC sub-opcode: {op}")),
            }
        }

        _ => Err(format!("Unknown instruction: 0x{byte:02X}")),
    }
}

/// The parameter and result counts a block type resolves to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockArity {
    pub params: usize,
    pub results: usize,
}

impl BlockArity {
    pub const EMPTY: BlockArity = BlockArity {
        params: 0,
        results: 0,
    };

    fn results_only(results: usize) -> Self {
        BlockArity { params: 0, results }
    }
}

/// WASM's `min`/`max` are not Rust's. Rust returns the non-NaN operand when one
/// side is NaN; WASM propagates the NaN. Rust also leaves the sign of a zero
/// result unspecified when both operands are zero, where WASM pins it: `min`
/// gives -0 and `max` gives +0.
fn nan_result_f32(x: f32, y: f32) -> f32 {
    // Any NaN satisfies `nan:arithmetic`, but `nan:canonical` requires a
    // payload of zero, so a NaN carrying one has to be propagated rather than
    // flattened into the canonical value.
    for v in [x, y] {
        if v.is_nan() && v.to_bits() & 0x003f_ffff != 0 {
            return v;
        }
    }
    f32::NAN
}

fn nan_result_f64(x: f64, y: f64) -> f64 {
    for v in [x, y] {
        if v.is_nan() && v.to_bits() & 0x0007_ffff_ffff_ffff != 0 {
            return v;
        }
    }
    f64::NAN
}

fn wasm_min_f32(x: f32, y: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        nan_result_f32(x, y)
    } else if x == 0.0 && y == 0.0 {
        if x.is_sign_negative() {
            x
        } else {
            y
        }
    } else if x < y {
        x
    } else {
        y
    }
}

fn wasm_max_f32(x: f32, y: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        nan_result_f32(x, y)
    } else if x == 0.0 && y == 0.0 {
        if x.is_sign_positive() {
            x
        } else {
            y
        }
    } else if x > y {
        x
    } else {
        y
    }
}

fn wasm_min_f64(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        nan_result_f64(x, y)
    } else if x == 0.0 && y == 0.0 {
        if x.is_sign_negative() {
            x
        } else {
            y
        }
    } else if x < y {
        x
    } else {
        y
    }
}

fn wasm_max_f64(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        nan_result_f64(x, y)
    } else if x == 0.0 && y == 0.0 {
        if x.is_sign_positive() {
            x
        } else {
            y
        }
    } else if x > y {
        x
    } else {
        y
    }
}

/// Check that `start .. start + n` lies inside something of length `len`.
///
/// The bulk table and memory operations are all-or-nothing: the spec has them
/// trap before any element is written, not partway through. Writing as you go
/// and trapping on the first bad index leaves the prefix modified, which a
/// program that catches the trap can then observe.
fn check_bulk_range(start: u32, n: u32, len: u32, what: &str) -> Result<(), String> {
    let end = start
        .checked_add(n)
        .ok_or_else(|| format!("{what}: range {start}+{n} overflows"))?;
    if end > len {
        return Err(format!(
            "{what}: out of bounds access at {start}..{end} (size {len})"
        ));
    }
    Ok(())
}

/// Range check for the trapping float-to-int conversions.
///
/// The operand traps only when its *truncated* value falls outside the target,
/// so a fractional part reaching past a bound is still fine: `i32.trunc_f64_s`
/// of -2147483648.9 is `i32::MIN`, not a trap. Comparing the raw operand
/// against `MIN`/`MAX` cast to the float type gets both ends wrong, because
/// neither bound is always representable: `i32::MAX as f32` rounds *up* to
/// 2^31. Truncating first and widening to f64 makes the comparison exact for
/// f32 operands and unchanged for f64 ones.
fn trunc_in_range(x: f64, low: f64, high_exclusive: f64) -> Result<f64, String> {
    if x.is_nan() {
        return Err("Invalid conversion to integer: NaN".to_string());
    }
    let t = x.trunc();
    if t < low || t >= high_exclusive {
        return Err("Integer overflow in truncation".to_string());
    }
    Ok(t)
}

/// Represents a single function call frame on the call stack
/// Control flow block state for branching
#[derive(Debug, Clone)]
pub struct BlockFrame {
    /// How many values the block takes from the stack on entry. Non-zero only
    /// for a block typed by a type-section index with parameters
    pub param_count: usize,
    /// How many values the block leaves on the stack when it ends normally
    pub result_count: usize,
    /// Stack depth at block entry, below the block's own parameters
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
    /// Operand stack depth at function entry (after popping args).
    /// Used by `return` to restore the stack to the correct depth.
    pub base_stack_depth: usize,
}

impl Frame {
    pub fn new(func_idx: u32, locals: Vec<Value>, num_returns: usize) -> Self {
        Frame {
            func_idx,
            locals,
            return_addr: 0,
            num_returns,
            base_stack_depth: 0,
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
        self.operand_stack.pop().ok_or_else(|| {
            let call_stack: Vec<u32> = self.call_stack.iter().map(|f| f.func_idx).collect();
            format!("Operand stack underflow (call_stack={call_stack:?})")
        })
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
        arity: BlockArity,
        is_loop: bool,
        start_pos: usize,
        end_pos: usize,
        is_then_branch: bool,
    ) {
        // The block's parameters are already on the stack and belong to it, so
        // the recorded depth sits below them: that is what a branch out of the
        // block, or a branch back to a loop header, restores the stack to.
        let stack_depth = self.operand_stack.len().saturating_sub(arity.params);
        self.block_stack.push(BlockFrame {
            param_count: arity.params,
            result_count: arity.results,
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

/// Evaluate a constant expression as a Value (used for global initialization).
/// Handles i32.const, i64.const, f32.const, f64.const, and global.get.
fn evaluate_const_expr_value(expr: &[u8], already_init: &[Value]) -> Result<Value, String> {
    if expr.is_empty() {
        return Ok(Value::I32(0));
    }
    let mut cursor = Cursor::new(expr);
    let instr = decode_instruction(&mut cursor)?;
    match instr {
        Instruction::I32Const(v) => Ok(Value::I32(v)),
        Instruction::I64Const(v) => Ok(Value::I64(v)),
        Instruction::F32Const(v) => Ok(Value::F32(v)),
        Instruction::F64Const(v) => Ok(Value::F64(v)),
        Instruction::RefNull(ValueType::ExternRef) => Ok(Value::ExternRef(None)),
        Instruction::RefNull(_) => Ok(Value::FuncRef(None)),
        Instruction::RefFunc(idx) => Ok(Value::FuncRef(Some(idx))),
        Instruction::GlobalGet(idx) => {
            // global.get in an init_expr refers to an already-initialized global
            already_init
                .get(idx as usize)
                .copied()
                .ok_or_else(|| format!("global.get {idx} out of bounds in const expr"))
        }
        _ => Err(format!(
            "Unsupported constant expression instruction: {instr:?}"
        )),
    }
}

/// Evaluate a constant expression as an address offset (used for data/element segment offsets).
/// Supports i32.const and i64.const followed by end.
fn evaluate_const_expr(expr: &[u8]) -> Result<usize, String> {
    match evaluate_const_expr_value(expr, &[])? {
        Value::I32(v) => Ok(v as u32 as usize),
        Value::I64(v) => Ok(v as u64 as usize),
        other => Err(format!("Expected i32/i64 in const expr, got {other:?}")),
    }
}

/// A runtime table instance: a growable vector of reference values, all of the
/// table's declared element type (`funcref` or `externref`). Function tables
/// hold `Value::FuncRef`, external tables hold `Value::ExternRef`.
#[derive(Debug, Clone)]
pub struct TableInstance {
    pub element_type: ValueType,
    pub max: Option<u32>,
    pub elements: Vec<Value>,
}

impl TableInstance {
    /// The null reference value matching `element_type`.
    fn null_value(element_type: ValueType) -> Value {
        match element_type {
            ValueType::ExternRef => Value::ExternRef(None),
            _ => Value::FuncRef(None),
        }
    }

    fn new(element_type: ValueType, initial: u32, max: Option<u32>) -> Self {
        let null = Self::null_value(element_type);
        TableInstance {
            element_type,
            max,
            elements: vec![null; initial as usize],
        }
    }

    fn size(&self) -> u32 {
        self.elements.len() as u32
    }

    fn get(&self, idx: u32) -> Result<Value, String> {
        self.elements.get(idx as usize).copied().ok_or_else(|| {
            format!(
                "table access out of bounds: index {idx} (size {})",
                self.elements.len()
            )
        })
    }

    fn set(&mut self, idx: u32, val: Value) -> Result<(), String> {
        let len = self.elements.len();
        let slot = self
            .elements
            .get_mut(idx as usize)
            .ok_or_else(|| format!("table access out of bounds: index {idx} (size {len})"))?;
        *slot = val;
        Ok(())
    }

    /// Grow the table by `n` slots filled with `init`. Returns the previous
    /// size on success, or `-1` if growth would exceed the table's maximum.
    fn grow(&mut self, n: u32, init: Value) -> i32 {
        let old = self.elements.len() as u32;
        let new_size = match old.checked_add(n) {
            Some(s) => s,
            None => return -1,
        };
        if let Some(max) = self.max {
            if new_size > max {
                return -1;
            }
        }
        self.elements.resize(new_size as usize, init);
        old as i32
    }
}

/// Runtime state of an element segment, used by `table.init` / `elem.drop`.
/// Active segments are applied at instantiation and start out dropped.
#[derive(Debug, Clone)]
/// A data segment's runtime state. `memory.init` reads through this rather
/// than the module, so that `data.drop` can take a segment out of service.
struct DataSegmentState {
    data: Vec<u8>,
    dropped: bool,
}

struct ElemSegmentState {
    /// One entry per element; `None` is a null reference.
    func_indices: Vec<Option<u32>>,
    dropped: bool,
}

/// WASM instruction executor
pub struct Executor {
    context: ExecutionContext,
    module: Module,
    linker: Option<Linker>,
    import_func_count: usize,
    /// Runtime table instances, indexed by the module's table index space
    /// (imported tables first, then module-defined tables). Each holds
    /// reference values of the table's element type.
    tables: Vec<TableInstance>,
    /// Per-element-segment state for `table.init` / `elem.drop`.
    elem_segments: Vec<ElemSegmentState>,
    data_segments: Vec<DataSegmentState>,
    /// Remaining instruction budget ("fuel"). `None` = unlimited. When `Some`,
    /// each dispatched instruction decrements it; reaching zero aborts
    /// execution with `FUEL_EXHAUSTED_ERROR`.
    fuel: Option<u64>,
    /// Cooperative cancellation flag. When set and flipped to `true`, the
    /// instruction loop aborts with `EXECUTION_CANCELLED_ERROR` at the next
    /// check. `None` = not cancellable. Shared (`Arc`) so an outside thread —
    /// e.g. the agent server on wall-clock timeout — can trip it while we run.
    max_call_depth: usize,
    cancel: Option<Arc<AtomicBool>>,
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

        // Imported globals come first in the global index space. wasmrun does
        // not resolve them from a host module, but they still have to occupy
        // their slots or every module-defined global sits at the wrong index
        // and `global.get` reads a neighbour.
        for import in &module.imports {
            if let ImportKind::Global(gt) = &import.kind {
                context.globals.push(match gt.value_type {
                    ValueType::I64 => Value::I64(0),
                    ValueType::F32 => Value::F32(0.0),
                    ValueType::F64 => Value::F64(0.0),
                    ValueType::FuncRef => Value::FuncRef(None),
                    ValueType::ExternRef => Value::ExternRef(None),
                    _ => Value::I32(0),
                });
            }
        }

        for global in &module.globals {
            let init_val = if global.init_expr.is_empty() {
                match global.value_type {
                    ValueType::I32 => Value::I32(0),
                    ValueType::I64 => Value::I64(0),
                    ValueType::F32 => Value::F32(0.0),
                    ValueType::F64 => Value::F64(0.0),
                    ValueType::FuncRef => Value::FuncRef(None),
                    ValueType::ExternRef => Value::ExternRef(None),
                    _ => return Err(format!("Unsupported global type: {:?}", global.value_type)),
                }
            } else {
                evaluate_const_expr_value(&global.init_expr, &context.globals)
                    .map_err(|e| format!("Global init expr error: {e}"))?
            };
            context.globals.push(init_val);
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

        // Passive data segments stay available to `memory.init` until
        // `data.drop`; active ones were written above and start out dropped.
        let data_segments: Vec<DataSegmentState> = module
            .data
            .iter()
            .map(|seg| DataSegmentState {
                data: if seg.offset_expr.is_empty() {
                    seg.data.clone()
                } else {
                    Vec::new()
                },
                dropped: !seg.offset_expr.is_empty(),
            })
            .collect();

        // Build table instances spanning the module's table index space:
        // imported tables first, then module-defined tables. Each is sized to
        // its declared initial length and filled with null references.
        let mut tables: Vec<TableInstance> = Vec::new();
        for import in &module.imports {
            if let ImportKind::Table(tt) = &import.kind {
                tables.push(TableInstance::new(tt.element_type, tt.initial, tt.max));
            }
        }
        for tt in &module.tables {
            tables.push(TableInstance::new(tt.element_type, tt.initial, tt.max));
        }

        // Apply element segments. Active segments are written into their target
        // table at instantiation (offset comes from a const expr) and start out
        // dropped; passive/declarative segments remain available to table.init.
        let mut elem_segments: Vec<ElemSegmentState> = Vec::with_capacity(module.elements.len());
        for seg in &module.elements {
            if seg.declarative {
                // Declarative segments only forward-declare functions for
                // `ref.func`. Nothing is written and `table.init` may not name
                // one, which is the same as starting out dropped.
                elem_segments.push(ElemSegmentState {
                    func_indices: Vec::new(),
                    dropped: true,
                });
                continue;
            }
            if seg.offset_expr.is_empty() {
                // Passive segment: filled lazily via table.init.
                elem_segments.push(ElemSegmentState {
                    func_indices: seg.function_indices.clone(),
                    dropped: false,
                });
                continue;
            }
            let offset = evaluate_const_expr(&seg.offset_expr)?;
            let end = offset + seg.function_indices.len();
            // Synthesize a funcref table 0 if none was declared, so a module
            // whose only table comes from an element segment still works.
            if tables.is_empty() {
                tables.push(TableInstance::new(ValueType::FuncRef, 0, None));
            }
            let table = tables.get_mut(seg.table_index as usize).ok_or_else(|| {
                format!(
                    "Element segment targets table {} which does not exist",
                    seg.table_index
                )
            })?;
            if end > table.elements.len() {
                let null = TableInstance::null_value(table.element_type);
                table.elements.resize(end, null);
            }
            for (i, func_idx) in seg.function_indices.iter().enumerate() {
                table.elements[offset + i] = match func_idx {
                    Some(f) => Value::FuncRef(Some(*f)),
                    None => TableInstance::null_value(table.element_type),
                };
            }
            elem_segments.push(ElemSegmentState {
                func_indices: Vec::new(),
                dropped: true,
            });
        }

        Ok(Executor {
            context,
            module,
            linker,
            import_func_count,
            tables,
            elem_segments,
            data_segments,
            fuel: None,
            max_call_depth: DEFAULT_MAX_CALL_DEPTH,
            cancel: None,
        })
    }

    /// Set the instruction budget ("fuel") for subsequent executions.
    ///
    /// `Some(n)` aborts execution after `n` instructions with
    /// `FUEL_EXHAUSTED_ERROR`; `None` (the default) runs without a fuel cap.
    pub fn set_fuel(&mut self, fuel: Option<u64>) {
        self.fuel = fuel;
    }

    /// Check whether an error string indicates fuel exhaustion.
    pub fn is_fuel_exhausted(err: &str) -> bool {
        err.contains(FUEL_EXHAUSTED_ERROR)
    }

    /// Install a cancellation token checked during execution.
    ///
    /// When the shared flag is flipped to `true`, the instruction loop aborts
    /// with `EXECUTION_CANCELLED_ERROR` at the next check. `None` disables
    /// cancellation (the default).
    /// Set how deeply guest code may nest calls before the execution traps.
    pub fn set_max_call_depth(&mut self, depth: usize) {
        self.max_call_depth = depth;
    }

    pub fn set_cancel_token(&mut self, token: Option<Arc<AtomicBool>>) {
        self.cancel = token;
    }

    /// Check whether an error string indicates a cancelled execution.
    pub fn is_cancelled(err: &str) -> bool {
        err.contains(EXECUTION_CANCELLED_ERROR)
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
                        ValueType::FuncRef => Value::FuncRef(None),
                        ValueType::ExternRef => Value::ExternRef(None),
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
        let mut frame = Frame::new(func_idx, locals, num_returns);
        frame.base_stack_depth = self.context.operand_stack.len();
        self.context.push_frame(frame);

        let block_depth_before = self.context.block_stack.len();
        // WASM spec: every function body is implicitly wrapped in a block.
        // Push it so the function-terminating 0x0b End byte pops this frame
        // rather than accidentally popping the caller's block frames. Its
        // results are the function's, so a `br` to the outermost label carries
        // as many values as a `return` would.
        self.context
            .push_block(BlockArity::results_only(num_returns), false, 0, 0, false);

        // Execute bytecode
        let mut cursor = Cursor::new(code.as_slice());
        let result = self.execute_bytecode(&mut cursor);
        self.context.block_stack.truncate(block_depth_before);
        if result.is_err() {
            // A trap leaves the machine wherever it stopped: frames from the
            // aborted call tree still on the call stack, operands still on the
            // value stack. An `Executor` outlives the call, so without this
            // every later execution starts from that wreckage. Runaway
            // recursion was the clearest case: the trap left 1024 frames
            // behind, and the next call hit the depth ceiling immediately.
            self.context.call_stack.clear();
            self.context.operand_stack.clear();
            self.context.block_stack.clear();
        }
        result?;

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

            // Charge one unit of fuel per instruction. `fuel` is shared across
            // nested calls (each runs its own execute_bytecode against the same
            // executor), so this bounds total instructions across the whole call
            // tree, not just the current function body.
            if let Some(remaining) = self.fuel.as_mut() {
                if *remaining == 0 {
                    return Err(FUEL_EXHAUSTED_ERROR.to_string());
                }
                *remaining -= 1;
            }

            // Cooperative cancellation: an outside thread (e.g. the agent
            // server on wall-clock timeout) can trip this flag to halt a
            // runaway execution that fuel alone wouldn't stop. The Relaxed
            // atomic load is negligible next to instruction decode/dispatch.
            if let Some(flag) = self.cancel.as_ref() {
                if flag.load(Ordering::Relaxed) {
                    return Err(EXECUTION_CANCELLED_ERROR.to_string());
                }
            }

            let instr = decode_instruction(cursor)?;
            if self.dispatch_instruction(instr, cursor)? == ControlFlow::Return {
                break;
            }
        }
        Ok(())
    }

    /// Skip bytecode until we find the matching else or end instruction.
    /// Returns true if we stopped at an `else`, false if we stopped at an `end`.
    fn skip_to_else_or_end(&mut self, cursor: &mut Cursor<&[u8]>) -> Result<bool, String> {
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
                    // Found matching else — cursor is now after `else`
                    return Ok(true);
                }
                Instruction::End => {
                    if depth == 0 {
                        // Found matching end (no else branch) — cursor is now after `end`
                        return Ok(false);
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

    /// Resolve a block type to the number of values it takes and leaves.
    fn block_arity(&self, block_type: BlockType) -> Result<BlockArity, String> {
        match block_type {
            BlockType::Empty => Ok(BlockArity::EMPTY),
            BlockType::Value(_) => Ok(BlockArity::results_only(1)),
            BlockType::FuncType(idx) => {
                let ty = self
                    .module
                    .types
                    .get(idx as usize)
                    .ok_or_else(|| format!("Block type index {idx} out of bounds"))?;
                Ok(BlockArity {
                    params: ty.params.len(),
                    results: ty.results.len(),
                })
            }
        }
    }

    /// Execute a branch to the given label depth
    fn do_branch(&mut self, label: u32, cursor: &mut Cursor<&[u8]>) -> Result<(), String> {
        let label_idx = label as usize;
        if label_idx >= self.context.block_stack.len() {
            let depth = self.context.block_stack.len();
            let call_stack: Vec<u32> = self.context.call_stack.iter().map(|f| f.func_idx).collect();
            return Err(format!(
                "br: invalid label {label} (block_stack depth={depth}, call_stack={call_stack:?})"
            ));
        }

        let block_idx = self.context.block_stack.len() - 1 - label_idx;
        let target_block = &self.context.block_stack[block_idx];

        // Arity: number of values the branch carries to its target.
        // A loop label is its own header, so a branch to it supplies the loop's
        // parameters again; every other label is the block's exit, so a branch
        // to it supplies the block's results.
        let arity: usize = if target_block.is_loop {
            target_block.param_count
        } else {
            target_block.result_count
        };
        let is_loop = target_block.is_loop;

        // Restore operand stack: keep only the top `arity` values, truncate to target stack_depth
        let target_depth = target_block.stack_depth;
        let result_values: Vec<Value> =
            if arity > 0 && self.context.operand_stack.len() > target_depth {
                let cur_len = self.context.operand_stack.len();
                self.context
                    .operand_stack
                    .drain(cur_len - arity..)
                    .collect()
            } else {
                Vec::new()
            };
        if self.context.operand_stack.len() > target_depth {
            self.context.operand_stack.truncate(target_depth);
        }
        for v in result_values {
            self.context.operand_stack.push(v);
        }

        if is_loop {
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

        // Each guest call consumes a host stack frame, so this ceiling is what
        // stands between runaway guest recursion and a process abort.
        if self.context.call_stack.len() >= self.max_call_depth {
            return Err(CALL_DEPTH_EXCEEDED_ERROR.to_string());
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
        // Record stack depth AFTER popping args — this is the frame's baseline.
        // `return` uses this to restore the operand stack before leaving.
        let base_stack_depth = self.context.operand_stack.len();

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
                    ValueType::FuncRef => Value::FuncRef(None),
                    ValueType::ExternRef => Value::ExternRef(None),
                    _ => return Err(format!("Unsupported value type in locals: {value_type:?}")),
                };
                locals.push(default_value);
            }
        }

        // Create call frame and push it
        let mut frame = Frame::new(func_idx, locals, num_results);
        frame.base_stack_depth = base_stack_depth;
        self.context.push_frame(frame);

        // Snapshot block stack depth so we can restore it on return.
        // A `return` inside a block breaks out of execute_bytecode without
        // popping the intra-function blocks, which would corrupt the caller's
        // block stack.
        let block_depth_before = self.context.block_stack.len();
        // WASM spec: every function body is implicitly wrapped in a block.
        // Push it so the function-terminating 0x0b End byte pops this frame
        // rather than accidentally popping the caller's block frames. Its
        // results are the function's, so a `br` to the outermost label carries
        // as many values as a `return` would.
        self.context
            .push_block(BlockArity::results_only(num_results), false, 0, 0, false);

        // Execute function bytecode
        let mut cursor = Cursor::new(code.as_slice());
        let result = self.execute_bytecode(&mut cursor);

        // Restore block stack to the depth it had before this call.
        self.context.block_stack.truncate(block_depth_before);

        result?;

        // Ensure the operand stack is exactly base_stack_depth + num_results.
        // Normally the function body leaves the right values, but after a `return`
        // or branch cleanup we enforce correctness here.
        let cur = self.context.operand_stack.len();
        let expected = base_stack_depth + num_results;
        if cur > expected {
            let results: Vec<Value> = if num_results > 0 {
                let start = cur.saturating_sub(num_results);
                self.context.operand_stack.drain(start..).collect()
            } else {
                Vec::new()
            };
            self.context.operand_stack.truncate(base_stack_depth);
            for v in results {
                self.context.operand_stack.push(v);
            }
        }

        // Pop frame
        self.context.pop_frame()?;

        Ok(())
    }

    /// Call a function indirectly via table lookup
    /// Borrow table `idx`, erroring if the module has no such table.
    fn table(&self, idx: u32) -> Result<&TableInstance, String> {
        self.tables
            .get(idx as usize)
            .ok_or_else(|| format!("Table index {idx} out of bounds"))
    }

    /// Mutably borrow table `idx`, erroring if the module has no such table.
    fn table_mut(&mut self, idx: u32) -> Result<&mut TableInstance, String> {
        self.tables
            .get_mut(idx as usize)
            .ok_or_else(|| format!("Table index {idx} out of bounds"))
    }

    fn call_function_indirect(
        &mut self,
        elem_idx: u32,
        type_idx: u32,
        table_idx: u32,
    ) -> Result<(), String> {
        // The slot must hold a non-null funcref.
        let table = self.table(table_idx)?;
        let abs_func_idx = match table.get(elem_idx)? {
            Value::FuncRef(Some(f)) => f,
            Value::FuncRef(None) => {
                return Err(format!(
                    "call_indirect: null function reference at index {elem_idx}"
                ))
            }
            other => {
                return Err(format!(
                    "call_indirect: expected funcref in table, found {other:?}"
                ))
            }
        };

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
                        // i32::MIN % -1 has no representable quotient, so Rust
                        // panics on it. The spec defines the remainder as 0,
                        // which is what wrapping_rem returns.
                        self.context.push(Value::I32(x.wrapping_rem(y)));
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
                        // Same as i32.rem_s: i64::MIN % -1 is 0, not a panic.
                        self.context.push(Value::I64(x.wrapping_rem(y)));
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
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::F32(wasm_min_f32(x, y)))
                    }
                    _ => return Err("Type mismatch for f32.min".to_string()),
                }
            }
            Instruction::F32Max => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F32(x), Value::F32(y)) => {
                        self.context.push(Value::F32(wasm_max_f32(x, y)))
                    }
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
                    // `nearest` rounds halfway cases to even; Rust's `round`
                    // rounds them away from zero, so nearest(-0.5) came out as
                    // -1.0 where the spec says -0.0.
                    Value::F32(x) => self.context.push(Value::F32(x.round_ties_even())),
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
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::F64(wasm_min_f64(x, y)))
                    }
                    _ => return Err("Type mismatch for f64.min".to_string()),
                }
            }
            Instruction::F64Max => {
                let b = self.context.pop()?;
                let a = self.context.pop()?;
                match (a, b) {
                    (Value::F64(x), Value::F64(y)) => {
                        self.context.push(Value::F64(wasm_max_f64(x, y)))
                    }
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
                    // `nearest` rounds halfway cases to even; Rust's `round`
                    // rounds them away from zero, so nearest(-0.5) came out as
                    // -1.0 where the spec says -0.0.
                    Value::F64(x) => self.context.push(Value::F64(x.round_ties_even())),
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
            Instruction::Return => {
                // Clean up the operand stack to exactly base_stack_depth + num_returns.
                // In a valid WASM module the compiler guarantees this, but explicit cleanup
                // prevents accumulated garbage from corrupted branch handling.
                if let Ok(frame) = self.context.current_frame() {
                    let num_returns = frame.num_returns;
                    let base = frame.base_stack_depth;
                    let cur = self.context.operand_stack.len();
                    let expected = base + num_returns;
                    if cur > expected {
                        // Extra values: save the top num_returns, truncate, push back
                        let results: Vec<Value> = if num_returns > 0 {
                            let start = cur.saturating_sub(num_returns);
                            self.context.operand_stack.drain(start..).collect()
                        } else {
                            Vec::new()
                        };
                        self.context.operand_stack.truncate(base);
                        for v in results {
                            self.context.operand_stack.push(v);
                        }
                    }
                }
                return Ok(ControlFlow::Return);
            }
            Instruction::End => {
                self.context.pop_block().ok();
            }
            Instruction::Drop => {
                self.context.pop()?;
            }

            // Call instruction - implement function invocation
            Instruction::Call(func_idx) => {
                self.call_function(func_idx)?;
            }

            // CallIndirect - call function through table
            Instruction::CallIndirect(type_idx, table_idx) => {
                let func_idx = self.context.pop()?;
                if let Value::I32(idx) = func_idx {
                    self.call_function_indirect(idx as u32, type_idx, table_idx)?;
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

            // Reference instructions
            Instruction::RefNull(ref_type) => {
                self.context.push(match ref_type {
                    ValueType::ExternRef => Value::ExternRef(None),
                    _ => Value::FuncRef(None),
                });
            }
            Instruction::RefIsNull => {
                let val = self.context.pop()?;
                if !val.is_ref() {
                    return Err("ref.is_null expects a reference operand".to_string());
                }
                self.context.push(Value::I32(val.is_null_ref() as i32));
            }
            Instruction::RefFunc(func_idx) => {
                self.context.push(Value::FuncRef(Some(func_idx)));
            }

            // Table instructions
            Instruction::TableGet(table_idx) => {
                let elem = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.get index must be i32".to_string()),
                };
                let table = self.table(table_idx)?;
                let val = table.get(elem)?;
                self.context.push(val);
            }
            Instruction::TableSet(table_idx) => {
                let val = self.context.pop()?;
                let elem = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.set index must be i32".to_string()),
                };
                self.table_mut(table_idx)?.set(elem, val)?;
            }
            Instruction::TableSize(table_idx) => {
                let size = self.table(table_idx)?.size();
                self.context.push(Value::I32(size as i32));
            }
            Instruction::TableGrow(table_idx) => {
                let n = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.grow count must be i32".to_string()),
                };
                let init = self.context.pop()?;
                let prev = self.table_mut(table_idx)?.grow(n, init);
                self.context.push(Value::I32(prev));
            }
            Instruction::TableFill(table_idx) => {
                let n = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.fill count must be i32".to_string()),
                };
                let val = self.context.pop()?;
                let start = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.fill index must be i32".to_string()),
                };
                let table = self.table_mut(table_idx)?;
                check_bulk_range(start, n, table.size(), "table.fill")?;
                for i in 0..n {
                    table.set(start + i, val)?;
                }
            }
            Instruction::TableCopy(dst_table, src_table) => {
                let n = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.copy count must be i32".to_string()),
                };
                let src = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.copy src must be i32".to_string()),
                };
                let dst = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.copy dst must be i32".to_string()),
                };
                // Snapshot the source range first so an overlapping in-table
                // copy (dst_table == src_table) reads pre-copy values.
                check_bulk_range(src, n, self.table(src_table)?.size(), "table.copy")?;
                check_bulk_range(dst, n, self.table(dst_table)?.size(), "table.copy")?;
                let mut buf = Vec::with_capacity(n as usize);
                {
                    let src_t = self.table(src_table)?;
                    for i in 0..n {
                        buf.push(src_t.get(src + i)?);
                    }
                }
                let dst_t = self.table_mut(dst_table)?;
                for (i, v) in buf.into_iter().enumerate() {
                    dst_t.set(dst + i as u32, v)?;
                }
            }
            Instruction::TableInit(elem_idx, table_idx) => {
                let n = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.init count must be i32".to_string()),
                };
                let src = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.init src must be i32".to_string()),
                };
                let dst = match self.context.pop()? {
                    Value::I32(i) => i as u32,
                    _ => return Err("table.init dst must be i32".to_string()),
                };
                let seg = self.elem_segments.get(elem_idx as usize).ok_or_else(|| {
                    format!("table.init: element segment {elem_idx} out of bounds")
                })?;
                // As with memory.init, dropping empties the segment rather
                // than poisoning it, so a zero-length init still succeeds.
                check_bulk_range(src, n, seg.func_indices.len() as u32, "table.init")?;
                check_bulk_range(dst, n, self.table(table_idx)?.size(), "table.init")?;
                let seg = self.elem_segments.get(elem_idx as usize).ok_or_else(|| {
                    format!("table.init: element segment {elem_idx} out of bounds")
                })?;
                // Resolve the funcrefs first to avoid holding a borrow on
                // self.elem_segments while mutating self.tables.
                let mut refs = Vec::with_capacity(n as usize);
                for i in 0..n {
                    let f = seg.func_indices.get((src + i) as usize).ok_or_else(|| {
                        format!("table.init: source index {} out of bounds", src + i)
                    })?;
                    refs.push(match f {
                        Some(idx) => Value::FuncRef(Some(*idx)),
                        None => Value::FuncRef(None),
                    });
                }
                let table = self.table_mut(table_idx)?;
                for (i, v) in refs.into_iter().enumerate() {
                    table.set(dst + i as u32, v)?;
                }
            }
            Instruction::ElemDrop(elem_idx) => {
                let seg = self
                    .elem_segments
                    .get_mut(elem_idx as usize)
                    .ok_or_else(|| {
                        format!("elem.drop: element segment {elem_idx} out of bounds")
                    })?;
                seg.dropped = true;
                seg.func_indices = Vec::new();
            }

            // Memory load operations — effective address = stack_val + offset
            // Effective address is base + offset in wider-than-32-bit
            // arithmetic. Wrapping it into u32 let an address near 4 GiB land
            // back at the bottom of memory and read a valid byte where the spec
            // requires a trap.
            Instruction::I32Load(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i32(addr)?;
                self.context.push(Value::I32(val));
            }
            Instruction::I64Load(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i64(addr)?;
                self.context.push(Value::I64(val));
            }
            Instruction::F32Load(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_f32(addr)?;
                self.context.push(Value::F32(val));
            }
            Instruction::F64Load(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_f64(addr)?;
                self.context.push(Value::F64(val));
            }
            Instruction::I32Load8S(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i8(addr)? as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I32Load8U(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = (self.context.memory.read_u8(addr)? as u32) as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I32Load16S(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i16(addr)? as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I32Load16U(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = (self.context.memory.read_u16(addr)? as u32) as i32;
                self.context.push(Value::I32(val));
            }
            Instruction::I64Load8S(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i8(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load8U(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_u8(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load16S(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i16(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load16U(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_u16(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load32S(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = self.context.memory.read_i32(addr)? as i64;
                self.context.push(Value::I64(val));
            }
            Instruction::I64Load32U(offset) => {
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                let val = (self.context.memory.read_i32(addr)? as u32) as i64;
                self.context.push(Value::I64(val));
            }

            // Memory store operations — effective address = stack_val + offset
            Instruction::I32Store(offset) => {
                let val = match self.context.pop()? {
                    Value::I32(v) => v,
                    _ => return Err("Value must be i32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_i32(addr, val)?;
            }
            Instruction::I64Store(offset) => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_i64(addr, val)?;
            }
            Instruction::F32Store(offset) => {
                let val = match self.context.pop()? {
                    Value::F32(v) => v,
                    _ => return Err("Value must be f32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_f32(addr, val)?;
            }
            Instruction::F64Store(offset) => {
                let val = match self.context.pop()? {
                    Value::F64(v) => v,
                    _ => return Err("Value must be f64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_f64(addr, val)?;
            }
            Instruction::I32Store8(offset) => {
                let val = match self.context.pop()? {
                    Value::I32(v) => v as u8,
                    _ => return Err("Value must be i32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u8(addr, val)?;
            }
            Instruction::I32Store16(offset) => {
                let val = match self.context.pop()? {
                    Value::I32(v) => v as u16,
                    _ => return Err("Value must be i32".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u16(addr, val)?;
            }
            Instruction::I64Store8(offset) => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v as u8,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u8(addr, val)?;
            }
            Instruction::I64Store16(offset) => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v as u16,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
                    _ => return Err("Address must be i32".to_string()),
                };
                self.context.memory.write_u16(addr, val)?;
            }
            Instruction::I64Store32(offset) => {
                let val = match self.context.pop()? {
                    Value::I64(v) => v as u32 as i32,
                    _ => return Err("Value must be i64".to_string()),
                };
                let addr = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize + offset as usize,
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
                    Ok(_) => {
                        self.context.push(Value::I32(old_pages as i32));
                    }
                    Err(_) => {
                        self.context.push(Value::I32(-1));
                    }
                }
            }

            // Bulk-memory: memory.fill — memset equivalent
            // Stack: [dest: i32, val: i32, len: i32]
            Instruction::MemoryFill => {
                // Every bulk-memory operand is an unsigned i32. Casting
                // straight to usize sign-extends a negative one into a huge
                // length, which then sails past the bounds check.
                let len = match self.context.pop()? {
                    Value::I32(n) => n as u32 as usize,
                    _ => return Err("memory.fill len must be i32".to_string()),
                };
                let val = match self.context.pop()? {
                    Value::I32(v) => (v & 0xFF) as u8,
                    _ => return Err("memory.fill val must be i32".to_string()),
                };
                let dest = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("memory.fill dest must be i32".to_string()),
                };
                let end = dest
                    .checked_add(len)
                    .ok_or_else(|| format!("memory.fill: range {dest}+{len} overflows"))?;
                if end > self.context.memory.size_bytes() {
                    return Err(format!(
                        "Memory access out of bounds: fill {len} bytes at {dest} (size: {} bytes)",
                        self.context.memory.size_bytes()
                    ));
                }
                for i in 0..len {
                    self.context.memory.write_u8(dest + i, val)?;
                }
            }

            // Bulk-memory: memory.copy — memmove equivalent
            // Stack: [dst: i32, src: i32, len: i32]
            Instruction::MemoryCopy => {
                let len = match self.context.pop()? {
                    Value::I32(n) => n as u32 as usize,
                    _ => return Err("memory.copy len must be i32".to_string()),
                };
                let src = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("memory.copy src must be i32".to_string()),
                };
                let dst = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("memory.copy dst must be i32".to_string()),
                };
                // Both ends are checked before either is touched.
                let size = self.context.memory.size_bytes();
                for (start, what) in [(src, "memory.copy src"), (dst, "memory.copy dst")] {
                    let end = start
                        .checked_add(len)
                        .ok_or_else(|| format!("{what}: range {start}+{len} overflows"))?;
                    if end > size {
                        return Err(format!(
                            "Memory access out of bounds: {what} {start}..{end} (size: {size} bytes)"
                        ));
                    }
                }
                let bytes = self.context.memory.read_bytes(src, len)?;
                self.context.memory.write_bytes(dst, &bytes)?;
            }

            // Bulk-memory: memory.init — copy data segment into memory
            // Stack: [dst: i32, src_offset: i32, len: i32]
            Instruction::MemoryInit(seg_idx) => {
                let len = match self.context.pop()? {
                    Value::I32(n) => n as u32 as usize,
                    _ => return Err("memory.init len must be i32".to_string()),
                };
                let src_off = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("memory.init src_offset must be i32".to_string()),
                };
                let dst = match self.context.pop()? {
                    Value::I32(a) => a as u32 as usize,
                    _ => return Err("memory.init dst must be i32".to_string()),
                };
                let seg = self
                    .data_segments
                    .get(seg_idx as usize)
                    .ok_or_else(|| format!("memory.init: data segment {seg_idx} out of bounds"))?;
                // A dropped segment is empty rather than unusable: the range
                // check below still lets a zero-length init through, which is
                // what the spec asks for.
                let src_end = src_off
                    .checked_add(len)
                    .ok_or_else(|| format!("memory.init: src range {src_off}+{len} overflows"))?;
                if src_end > seg.data.len() {
                    return Err(format!(
                        "memory.init: src range {src_off}..{src_end} out of segment bounds ({})",
                        seg.data.len()
                    ));
                }
                let mem_size = self.context.memory.size_bytes();
                let dst_end = dst
                    .checked_add(len)
                    .ok_or_else(|| format!("memory.init: dst range {dst}+{len} overflows"))?;
                if dst_end > mem_size {
                    return Err(format!(
                        "Memory access out of bounds: memory.init {dst}..{dst_end} (size: {mem_size} bytes)"
                    ));
                }
                let bytes = seg.data[src_off..src_end].to_vec();
                self.context.memory.write_bytes(dst, &bytes)?;
            }

            // Bulk-memory: data.drop takes a data segment out of service, so
            // a later memory.init naming it traps instead of copying again.
            Instruction::DataDrop(seg_idx) => {
                let seg = self
                    .data_segments
                    .get_mut(seg_idx as usize)
                    .ok_or_else(|| format!("data.drop: data segment {seg_idx} out of bounds"))?;
                seg.data = Vec::new();
                seg.dropped = true;
            }

            // Control flow - proper implementation
            Instruction::Block(block_type) => {
                let arity = self.block_arity(block_type)?;
                let pos = cursor.position() as usize;
                self.context.push_block(arity, false, pos, 0, false);
            }
            Instruction::Loop(block_type) => {
                let arity = self.block_arity(block_type)?;
                let pos = cursor.position() as usize;
                self.context.push_block(arity, true, pos, 0, false);
            }
            Instruction::If(block_type) => {
                let arity = self.block_arity(block_type)?;
                // Pop condition from stack. The block's parameters sit under it
                // and stay where they are, which is why the condition has to go
                // first: push_block measures the stack from the top.
                let cond = self.context.pop()?;
                let cond_value = match cond {
                    Value::I32(v) => v,
                    _ => return Err("if requires i32 condition".to_string()),
                };

                if cond_value != 0 {
                    // Condition is true — push frame, execute then-branch.
                    // If there's no else, the End instruction will pop this frame.
                    // If there IS an else, the Else handler will skip to end and pop this frame.
                    self.context.push_block(arity, false, 0, 0, true);
                } else {
                    // Condition is false — scan forward to find else or end.
                    let found_else = self.skip_to_else_or_end(cursor)?;
                    if found_else {
                        // Cursor is now after `else`; push frame for the else-body.
                        // The End at the close of this if will pop this frame.
                        self.context.push_block(arity, false, 0, 0, false);
                    }
                    // If found_else is false, `end` was already consumed — don't
                    // push a frame. An if with no else is only valid when its
                    // parameters and results have the same types, so the values
                    // already on the stack are the block's results and are left
                    // exactly as they are.
                }
            }
            Instruction::Else => {
                // We just finished the then-branch. Skip to the matching `end`
                // (which would otherwise be processed by the End handler, but we
                // consume it here so we must also pop the block frame ourselves).
                self.skip_to_end(cursor)?;
                self.context.pop_block()?;
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
                        let t = trunc_in_range(x as f64, -2147483648.0, 2147483648.0)?;
                        self.context.push(Value::I32(t as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f32_s".to_string()),
                }
            }
            Instruction::I32TruncF32U => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => {
                        let t = trunc_in_range(x as f64, 0.0, 4294967296.0)?;
                        self.context.push(Value::I32(t as u32 as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f32_u".to_string()),
                }
            }
            Instruction::I32TruncF64S => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        let t = trunc_in_range(x, -2147483648.0, 2147483648.0)?;
                        self.context.push(Value::I32(t as i32));
                    }
                    _ => return Err("Type mismatch for i32.trunc_f64_s".to_string()),
                }
            }
            Instruction::I32TruncF64U => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        let t = trunc_in_range(x, 0.0, 4294967296.0)?;
                        self.context.push(Value::I32(t as u32 as i32));
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
                        let t = trunc_in_range(
                            x as f64,
                            -9223372036854775808.0,
                            9223372036854775808.0,
                        )?;
                        self.context.push(Value::I64(t as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f32_s".to_string()),
                }
            }
            Instruction::I64TruncF32U => {
                let a = self.context.pop()?;
                match a {
                    Value::F32(x) => {
                        let t = trunc_in_range(x as f64, 0.0, 18446744073709551616.0)?;
                        self.context.push(Value::I64(t as u64 as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f32_u".to_string()),
                }
            }
            Instruction::I64TruncF64S => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        let t = trunc_in_range(x, -9223372036854775808.0, 9223372036854775808.0)?;
                        self.context.push(Value::I64(t as i64));
                    }
                    _ => return Err("Type mismatch for i64.trunc_f64_s".to_string()),
                }
            }
            Instruction::I64TruncF64U => {
                let a = self.context.pop()?;
                match a {
                    Value::F64(x) => {
                        let t = trunc_in_range(x, 0.0, 18446744073709551616.0)?;
                        self.context.push(Value::I64(t as u64 as i64));
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

            // Saturating float-to-int truncation. Where the trapping forms
            // reject NaN and out-of-range inputs, these clamp: NaN becomes 0
            // and anything past the target's range becomes its nearest bound.
            // Rust's own `as` casts between floats and integers are defined the
            // same way, so each one is a direct cast.
            Instruction::I32TruncSatF32S => {
                let a = match self.context.pop()? {
                    Value::F32(v) => v as i32,
                    _ => return Err("i32.trunc_sat_f32_s: expected f32".to_string()),
                };
                self.context.push(Value::I32(a));
            }
            Instruction::I32TruncSatF32U => {
                let a = match self.context.pop()? {
                    Value::F32(v) => v as u32,
                    _ => return Err("i32.trunc_sat_f32_u: expected f32".to_string()),
                };
                self.context.push(Value::I32(a as i32));
            }
            Instruction::I32TruncSatF64S => {
                let a = match self.context.pop()? {
                    Value::F64(v) => v as i32,
                    _ => return Err("i32.trunc_sat_f64_s: expected f64".to_string()),
                };
                self.context.push(Value::I32(a));
            }
            Instruction::I32TruncSatF64U => {
                let a = match self.context.pop()? {
                    Value::F64(v) => v as u32,
                    _ => return Err("i32.trunc_sat_f64_u: expected f64".to_string()),
                };
                self.context.push(Value::I32(a as i32));
            }
            Instruction::I64TruncSatF32S => {
                let a = match self.context.pop()? {
                    Value::F32(v) => v as i64,
                    _ => return Err("i64.trunc_sat_f32_s: expected f32".to_string()),
                };
                self.context.push(Value::I64(a));
            }
            Instruction::I64TruncSatF32U => {
                let a = match self.context.pop()? {
                    Value::F32(v) => v as u64,
                    _ => return Err("i64.trunc_sat_f32_u: expected f32".to_string()),
                };
                self.context.push(Value::I64(a as i64));
            }
            Instruction::I64TruncSatF64S => {
                let a = match self.context.pop()? {
                    Value::F64(v) => v as i64,
                    _ => return Err("i64.trunc_sat_f64_s: expected f64".to_string()),
                };
                self.context.push(Value::I64(a));
            }
            Instruction::I64TruncSatF64U => {
                let a = match self.context.pop()? {
                    Value::F64(v) => v as u64,
                    _ => return Err("i64.trunc_sat_f64_u: expected f64".to_string()),
                };
                self.context.push(Value::I64(a as i64));
            }

            // Sign-extension operators
            Instruction::I32Extend8S => {
                let a = match self.context.pop()? {
                    Value::I32(v) => v as i8 as i32,
                    _ => return Err("i32.extend8_s: expected i32".to_string()),
                };
                self.context.push(Value::I32(a));
            }
            Instruction::I32Extend16S => {
                let a = match self.context.pop()? {
                    Value::I32(v) => v as i16 as i32,
                    _ => return Err("i32.extend16_s: expected i32".to_string()),
                };
                self.context.push(Value::I32(a));
            }
            Instruction::I64Extend8S => {
                let a = match self.context.pop()? {
                    Value::I64(v) => v as i8 as i64,
                    _ => return Err("i64.extend8_s: expected i64".to_string()),
                };
                self.context.push(Value::I64(a));
            }
            Instruction::I64Extend16S => {
                let a = match self.context.pop()? {
                    Value::I64(v) => v as i16 as i64,
                    _ => return Err("i64.extend16_s: expected i64".to_string()),
                };
                self.context.push(Value::I64(a));
            }
            Instruction::I64Extend32S => {
                let a = match self.context.pop()? {
                    Value::I64(v) => v as i32 as i64,
                    _ => return Err("i64.extend32_s: expected i64".to_string()),
                };
                self.context.push(Value::I64(a));
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
    use super::super::module::{Function, FunctionType, TableType};
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Build a module whose only function is an infinite `loop { br 0 }`.
    fn infinite_loop_module() -> Module {
        Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                // loop (empty type); br 0; end (loop); end (func)
                code: vec![0x03, 0x40, 0x0c, 0x00, 0x0b, 0x0b],
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        }
    }

    #[test]
    fn test_fuel_aborts_infinite_loop() {
        let mut executor = Executor::new(infinite_loop_module()).unwrap();
        executor.set_fuel(Some(1000));
        let err = executor
            .execute_with_args(0, vec![])
            .expect_err("infinite loop should exhaust fuel");
        assert!(
            Executor::is_fuel_exhausted(&err),
            "expected fuel-exhausted error, got: {err}"
        );
    }

    #[test]
    fn test_no_fuel_cap_does_not_abort_finite_program() {
        // A function that simply returns; with no fuel cap it completes.
        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code: vec![0x0b], // end
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        executor.set_fuel(None);
        assert!(executor.execute_with_args(0, vec![]).is_ok());
    }

    #[test]
    fn test_cancel_token_preset_aborts_infinite_loop() {
        // A token already tripped before execution aborts at the first check —
        // deterministic, no timing involved. Note: no fuel cap is set, proving
        // cancellation halts a runaway loop that fuel alone would let run.
        let mut executor = Executor::new(infinite_loop_module()).unwrap();
        executor.set_cancel_token(Some(Arc::new(AtomicBool::new(true))));
        let err = executor
            .execute_with_args(0, vec![])
            .expect_err("pre-cancelled run should error");
        assert!(
            Executor::is_cancelled(&err),
            "expected cancellation error, got: {err}"
        );
    }

    #[test]
    fn test_cancel_token_aborts_running_infinite_loop() {
        // Trip the flag from a watcher thread while the loop runs on this
        // thread (so the test never depends on Executor being Send).
        let mut executor = Executor::new(infinite_loop_module()).unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        executor.set_cancel_token(Some(flag.clone()));
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            flag.store(true, Ordering::Relaxed);
        });
        let err = executor
            .execute_with_args(0, vec![])
            .expect_err("cancelled run should error");
        assert!(
            Executor::is_cancelled(&err),
            "expected cancellation error, got: {err}"
        );
    }

    #[test]
    fn test_untripped_cancel_token_allows_completion() {
        // An installed-but-untripped token must not affect a finite program.
        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![],
            }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code: vec![0x0b], // end
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        executor.set_cancel_token(Some(Arc::new(AtomicBool::new(false))));
        assert!(executor.execute_with_args(0, vec![]).is_ok());
    }

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

    // ----- Reference types (#93) -----

    /// Build a single-function module from a type signature, locals, body, and
    /// table declarations. Keeps the reference-type tests below terse.
    fn ref_module(
        params: Vec<ValueType>,
        results: Vec<ValueType>,
        locals: Vec<(u32, ValueType)>,
        code: Vec<u8>,
        tables: Vec<TableType>,
    ) -> Module {
        Module {
            version: 1,
            types: vec![FunctionType { params, results }],
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals,
                code,
            }],
            tables,
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        }
    }

    #[test]
    fn test_value_ref_helpers() {
        assert!(Value::null_funcref().is_ref());
        assert!(Value::null_externref().is_null_ref());
        assert!(!Value::I32(0).is_ref());
        assert!(!Value::I32(0).is_null_ref());
        assert!(!Value::FuncRef(Some(3)).is_null_ref());
        assert_eq!(Value::FuncRef(Some(7)).as_func_idx(), Some(7));
        assert_eq!(Value::FuncRef(None).as_func_idx(), None);
        assert_eq!(Value::ExternRef(Some(7)).as_func_idx(), None);
    }

    #[test]
    fn test_decode_ref_instructions() {
        // ref.null funcref / externref
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xD0, 0x70].as_slice())).unwrap(),
            Instruction::RefNull(ValueType::FuncRef)
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xD0, 0x6F].as_slice())).unwrap(),
            Instruction::RefNull(ValueType::ExternRef)
        ));
        // ref.is_null
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xD1].as_slice())).unwrap(),
            Instruction::RefIsNull
        ));
        // ref.func 5
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xD2, 0x05].as_slice())).unwrap(),
            Instruction::RefFunc(5)
        ));
        // ref.null with a non-reference type is rejected
        assert!(decode_instruction(&mut Cursor::new([0xD0, 0x7F].as_slice())).is_err());
    }

    #[test]
    fn test_decode_table_instructions() {
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x25, 0x00].as_slice())).unwrap(),
            Instruction::TableGet(0)
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x26, 0x01].as_slice())).unwrap(),
            Instruction::TableSet(1)
        ));
        // 0xFC table ops
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xFC, 0x10, 0x00].as_slice())).unwrap(),
            Instruction::TableSize(0)
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xFC, 0x0F, 0x00].as_slice())).unwrap(),
            Instruction::TableGrow(0)
        ));
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0xFC, 0x0C, 0x02, 0x01].as_slice())).unwrap(),
            Instruction::TableInit(2, 1)
        ));
        // typed select decodes (and discards) its type vector
        assert!(matches!(
            decode_instruction(&mut Cursor::new([0x1C, 0x01, 0x7F].as_slice())).unwrap(),
            Instruction::Select
        ));
    }

    #[test]
    fn test_ref_null_is_null() {
        // () -> i32 : ref.null func; ref.is_null  => 1
        let module = ref_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![0xD0, 0x70, 0xD1, 0x0b],
            vec![],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();
        assert_eq!(results, vec![Value::I32(1)]);
    }

    #[test]
    fn test_ref_func_is_not_null() {
        // () -> i32 : ref.func 0; ref.is_null  => 0
        let module = ref_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![0xD2, 0x00, 0xD1, 0x0b],
            vec![],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();
        assert_eq!(results, vec![Value::I32(0)]);
    }

    #[test]
    fn test_externref_param_roundtrip() {
        // (externref) -> externref : local.get 0  => same handle back
        let module = ref_module(
            vec![ValueType::ExternRef],
            vec![ValueType::ExternRef],
            vec![],
            vec![0x20, 0x00, 0x0b],
            vec![],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor
            .execute_with_args(0, vec![Value::ExternRef(Some(42))])
            .unwrap();
        assert_eq!(results, vec![Value::ExternRef(Some(42))]);
    }

    #[test]
    fn test_externref_table_set_get_roundtrip() {
        // (externref) -> externref :
        //   i32.const 0; local.get 0; table.set 0; i32.const 0; table.get 0
        let table = TableType {
            initial: 2,
            max: None,
            element_type: ValueType::ExternRef,
        };
        let module = ref_module(
            vec![ValueType::ExternRef],
            vec![ValueType::ExternRef],
            vec![],
            vec![
                0x41, 0x00, // i32.const 0  (table index)
                0x20, 0x00, // local.get 0  (value)
                0x26, 0x00, // table.set 0
                0x41, 0x00, // i32.const 0
                0x25, 0x00, // table.get 0
                0x0b, // end
            ],
            vec![table],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor
            .execute_with_args(0, vec![Value::ExternRef(Some(99))])
            .unwrap();
        assert_eq!(results, vec![Value::ExternRef(Some(99))]);
    }

    #[test]
    fn test_table_size_and_grow() {
        // () -> i32 : ref.null func; i32.const 3; table.grow 0; drop; table.size 0 => 5
        let table = TableType {
            initial: 2,
            max: None,
            element_type: ValueType::FuncRef,
        };
        let module = ref_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![
                0xD0, 0x70, // ref.null func  (grow init value)
                0x41, 0x03, // i32.const 3    (grow count)
                0xFC, 0x0F, 0x00, // table.grow 0  => pushes prev size 2
                0x1A, // drop
                0xFC, 0x10, 0x00, // table.size 0  => 5
                0x0b, // end
            ],
            vec![table],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();
        assert_eq!(results, vec![Value::I32(5)]);
    }

    #[test]
    fn test_table_grow_respects_max() {
        // Growing past `max` returns -1 and leaves the size unchanged.
        let table = TableType {
            initial: 1,
            max: Some(2),
            element_type: ValueType::FuncRef,
        };
        let module = ref_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![
                0xD0, 0x70, // ref.null func
                0x41, 0x05, // i32.const 5  (over max)
                0xFC, 0x0F, 0x00, // table.grow 0 => -1
                0x0b,
            ],
            vec![table],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();
        assert_eq!(results, vec![Value::I32(-1)]);
    }

    #[test]
    fn test_call_indirect_through_funcref_table() {
        use crate::runtime::core::module::ElementSegment;
        // Two functions: f0 returns 111, f1 returns 222. An element segment
        // installs [f0, f1] into table 0. The entry function call_indirects
        // table slot 1, expecting 222 — exercising the funcref table path.
        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![ValueType::I32],
            }],
            imports: vec![],
            functions: vec![
                Function {
                    type_index: 0,
                    locals: vec![],
                    code: vec![0x41, 0x6F, 0x0b], // i32.const 111; end
                },
                Function {
                    type_index: 0,
                    locals: vec![],
                    code: vec![0x41, 0xDE, 0x01, 0x0b], // i32.const 222; end
                },
                Function {
                    type_index: 0,
                    locals: vec![],
                    // i32.const 1; call_indirect type 0 table 0; end
                    code: vec![0x41, 0x01, 0x11, 0x00, 0x00, 0x0b],
                },
            ],
            tables: vec![TableType {
                initial: 2,
                max: None,
                element_type: ValueType::FuncRef,
            }],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![ElementSegment {
                offset_expr: vec![0x41, 0x00, 0x0b], // i32.const 0
                table_index: 0,
                declarative: false,
                function_indices: vec![Some(0), Some(1)],
            }],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(2).unwrap();
        assert_eq!(results, vec![Value::I32(222)]);
    }

    #[test]
    fn test_table_fill() {
        // () -> i32 : fill table[0..3] with ref.func 0, then read slot 2 and
        // check it is non-null. table.fill pops (n, val, start) top-down.
        let table = TableType {
            initial: 3,
            max: None,
            element_type: ValueType::FuncRef,
        };
        let module = ref_module(
            vec![],
            vec![ValueType::I32],
            vec![],
            vec![
                0x41, 0x00, // i32.const 0  (start)
                0xD2, 0x00, // ref.func 0   (fill value)
                0x41, 0x03, // i32.const 3  (count)
                0xFC, 0x11, 0x00, // table.fill 0
                0x41, 0x02, // i32.const 2
                0x25, 0x00, // table.get 0
                0xD1, // ref.is_null => 0
                0x0b,
            ],
            vec![table],
        );
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();
        assert_eq!(results, vec![Value::I32(0)]);
    }

    #[test]
    fn test_table_init_from_passive_segment() {
        use crate::runtime::core::module::ElementSegment;
        // A passive element segment [f0] is copied into table slot 0 via
        // table.init, then read back as a non-null funcref. table.init pops
        // (n, src, dst) top-down.
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
                code: vec![
                    0x41, 0x00, // i32.const 0  (dst)
                    0x41, 0x00, // i32.const 0  (src)
                    0x41, 0x01, // i32.const 1  (n)
                    0xFC, 0x0C, 0x00, 0x00, // table.init elem 0 table 0
                    0x41, 0x00, // i32.const 0
                    0x25, 0x00, // table.get 0
                    0xD1, // ref.is_null => 0
                    0x0b,
                ],
            }],
            tables: vec![TableType {
                initial: 2,
                max: None,
                element_type: ValueType::FuncRef,
            }],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            // Passive segment: empty offset_expr.
            elements: vec![ElementSegment {
                offset_expr: vec![],
                table_index: 0,
                declarative: false,
                function_indices: vec![Some(0)],
            }],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        let results = executor.execute(0).unwrap();
        assert_eq!(results, vec![Value::I32(0)]);

        // Dropping empties the segment rather than poisoning it, so a
        // non-zero table.init out of it now reads past its end. (A zero-length
        // init out of a dropped segment stays legal, which is why this is a
        // range error rather than a "segment dropped" one.)
        executor.elem_segments[0].func_indices.clear();
        executor.elem_segments[0].dropped = true;
        let err = executor.execute(0).unwrap_err();
        assert!(err.contains("out of bounds"), "got: {err}");
    }

    // ---- Multi-value blocks (0.23.1) ----

    /// Build a module with a caller function (index 0) plus the extra types a
    /// multi-value block needs to name. `types[0]` is the function's own
    /// signature; the rest are block signatures referenced by type index.
    fn multivalue_module(
        params: Vec<ValueType>,
        results: Vec<ValueType>,
        block_types: Vec<FunctionType>,
        locals: Vec<(u32, ValueType)>,
        code: Vec<u8>,
    ) -> Module {
        let mut types = vec![FunctionType { params, results }];
        types.extend(block_types);
        Module {
            version: 1,
            types,
            imports: vec![],
            functions: vec![Function {
                type_index: 0,
                locals,
                code,
            }],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        }
    }

    // ---- Conformance fixes surfaced by the spec suite (0.23.3) ----

    #[test]
    fn test_runaway_recursion_traps_instead_of_aborting() {
        // Unbounded guest recursion used to overflow the host stack, which is
        // a process abort rather than a catchable panic: in agent mode that
        // takes the server down for every tenant, not just the one request.
        //
        // () -> (): call 0 ;; itself, forever
        let module = multivalue_module(vec![], vec![], vec![], vec![], vec![0x10, 0x00, 0x0b]);
        let mut executor = Executor::new(module).unwrap();
        // Far under the default: these tests run unoptimized on a 2 MB test
        // thread, where a guest frame costs about 70 KB of host stack.
        executor.set_max_call_depth(16);
        let err = executor.execute(0).unwrap_err();
        assert!(err.contains("call stack exhausted"), "got: {err}");
    }

    #[test]
    fn test_a_trap_does_not_poison_the_next_execution() {
        // The trap above leaves a full call stack behind. An Executor outlives
        // the call, so anything reusing one has to see a clean machine.
        let module = Module {
            version: 1,
            types: vec![
                FunctionType {
                    params: vec![],
                    results: vec![],
                },
                FunctionType {
                    params: vec![],
                    results: vec![ValueType::I32],
                },
            ],
            imports: vec![],
            functions: vec![
                Function {
                    type_index: 0,
                    locals: vec![],
                    code: vec![0x10, 0x00, 0x0b], // call 0, forever
                },
                Function {
                    type_index: 1,
                    locals: vec![],
                    code: vec![0x41, 0x07, 0x0b], // i32.const 7
                },
            ],
            tables: vec![],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        executor.set_max_call_depth(16);
        assert!(executor.execute(0).is_err());
        assert_eq!(executor.execute(1).unwrap(), vec![Value::I32(7)]);
    }

    #[test]
    fn test_rem_s_of_min_by_minus_one_is_zero() {
        // i32::MIN % -1 has no representable quotient and panics in Rust. The
        // spec defines the remainder as 0.
        let module = multivalue_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![],
            vec![0x20, 0x00, 0x20, 0x01, 0x6f, 0x0b], // local.get 0/1; i32.rem_s
        );
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor
                .execute_with_args(0, vec![Value::I32(i32::MIN), Value::I32(-1)])
                .unwrap(),
            vec![Value::I32(0)]
        );
        // div_s on the same operands still traps, which is what the spec says.
        let div = multivalue_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![],
            vec![0x20, 0x00, 0x20, 0x01, 0x6d, 0x0b],
        );
        let mut executor = Executor::new(div).unwrap();
        assert!(executor
            .execute_with_args(0, vec![Value::I32(i32::MIN), Value::I32(-1)])
            .is_err());
    }

    #[test]
    fn test_effective_address_does_not_wrap_into_memory() {
        // base + offset is computed in wider-than-32-bit arithmetic. Wrapping
        // it let an address just under 4 GiB land back at the bottom of memory
        // and read a valid byte where the spec requires a trap.
        let mut module = multivalue_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![],
            vec![],
            // local.get 0; i32.load8_u offset=1
            vec![0x20, 0x00, 0x2d, 0x00, 0x01, 0x0b],
        );
        module.memory = Some(super::super::module::MemoryType {
            initial: 1,
            max: None,
        });
        let mut executor = Executor::new(module).unwrap();
        // 0xFFFFFFFF + 1 wraps to 0, which is in bounds. It must still trap.
        let err = executor
            .execute_with_args(0, vec![Value::I32(-1)])
            .unwrap_err();
        assert!(err.contains("out of bounds"), "got: {err}");
        // An ordinary address still works.
        assert_eq!(
            executor.execute_with_args(0, vec![Value::I32(0)]).unwrap(),
            vec![Value::I32(0)]
        );
    }

    #[test]
    fn test_bulk_memory_lengths_are_unsigned() {
        // A negative length used to sign-extend into a usize near 2^64, which
        // sailed past the bounds check and reached an allocation.
        let mut module = multivalue_module(
            vec![ValueType::I32],
            vec![],
            vec![],
            vec![],
            // i32.const 0 (dst); i32.const 0 (src); local.get 0 (len); memory.copy
            vec![
                0x41, 0x00, 0x41, 0x00, 0x20, 0x00, 0xFC, 0x0A, 0x00, 0x00, 0x0b,
            ],
        );
        module.memory = Some(super::super::module::MemoryType {
            initial: 1,
            max: None,
        });
        let mut executor = Executor::new(module).unwrap();
        let err = executor
            .execute_with_args(0, vec![Value::I32(-1)])
            .unwrap_err();
        assert!(err.contains("out of bounds"), "got: {err}");
    }

    #[test]
    fn test_min_max_propagate_nan_and_pin_zero_signs() {
        // Rust's min/max return the non-NaN operand and leave the sign of a
        // zero result unspecified; WASM propagates the NaN and pins the sign.
        assert!(wasm_min_f32(f32::NAN, 1.0).is_nan());
        assert!(wasm_max_f32(1.0, f32::NAN).is_nan());
        assert!(wasm_min_f64(f64::NAN, 1.0).is_nan());
        assert!(wasm_max_f64(1.0, f64::NAN).is_nan());

        assert!(wasm_min_f32(0.0, -0.0).is_sign_negative());
        assert!(wasm_min_f32(-0.0, 0.0).is_sign_negative());
        assert!(wasm_max_f32(0.0, -0.0).is_sign_positive());
        assert!(wasm_max_f32(-0.0, 0.0).is_sign_positive());

        // A NaN carrying a payload is propagated rather than flattened, since
        // a canonical input must not produce an arithmetic result.
        let payload = f32::from_bits(0x7fc0_0001);
        assert_eq!(wasm_min_f32(payload, 1.0).to_bits(), payload.to_bits());
        // Two canonical NaNs give a canonical NaN.
        assert_eq!(wasm_min_f32(f32::NAN, f32::NAN).to_bits() & 0x003f_ffff, 0);
    }

    #[test]
    fn test_trunc_range_is_exact_at_the_boundaries() {
        // The trap boundary is where the *truncated* value leaves range, so a
        // fractional part reaching past a bound is still fine.
        assert_eq!(
            trunc_in_range(-2147483648.9, -2147483648.0, 2147483648.0).unwrap() as i32,
            i32::MIN
        );
        assert_eq!(
            trunc_in_range(2147483647.9, -2147483648.0, 2147483648.0).unwrap() as i32,
            i32::MAX
        );
        // -0.9 truncates to zero, which is in range for an unsigned target.
        assert_eq!(trunc_in_range(-0.9, 0.0, 4294967296.0).unwrap() as u32, 0);
        // A whole step past the bound does trap.
        assert!(trunc_in_range(-2147483649.0, -2147483648.0, 2147483648.0).is_err());
        assert!(trunc_in_range(2147483648.0, -2147483648.0, 2147483648.0).is_err());
        assert!(trunc_in_range(-1.0, 0.0, 4294967296.0).is_err());
        assert!(trunc_in_range(f64::NAN, 0.0, 1.0).is_err());
    }

    #[test]
    fn test_nearest_rounds_halves_to_even() {
        // Rust's `round` rounds halves away from zero; `nearest` rounds to even,
        // so nearest(-0.5) is -0.0 and nearest(2.5) is 2.0.
        let module = multivalue_module(
            vec![ValueType::F64],
            vec![ValueType::F64],
            vec![],
            vec![],
            vec![0x20, 0x00, 0x9e, 0x0b], // local.get 0; f64.nearest
        );
        let mut executor = Executor::new(module).unwrap();
        let run = |e: &mut Executor, v: f64| match e
            .execute_with_args(0, vec![Value::F64(v)])
            .unwrap()[0]
        {
            Value::F64(r) => r,
            other => panic!("expected f64, got {other:?}"),
        };
        assert_eq!(run(&mut executor, 2.5), 2.0);
        assert_eq!(run(&mut executor, 3.5), 4.0);
        assert_eq!(run(&mut executor, 1.5), 2.0);
        let neg_half = run(&mut executor, -0.5);
        assert_eq!(neg_half, 0.0);
        assert!(
            neg_half.is_sign_negative(),
            "nearest(-0.5) must keep its sign"
        );
    }

    #[test]
    fn test_bulk_range_check_is_all_or_nothing() {
        assert!(check_bulk_range(8, 3, 10, "table.fill").is_err());
        assert!(check_bulk_range(8, 2, 10, "table.fill").is_ok());
        // A zero-length operation past the end still traps.
        assert!(check_bulk_range(11, 0, 10, "table.fill").is_err());
        assert!(check_bulk_range(10, 0, 10, "table.fill").is_ok());
        // The sum is checked rather than wrapped.
        assert!(check_bulk_range(u32::MAX, 2, 10, "table.fill").is_err());
    }

    #[test]
    fn test_memory_grow_stops_at_the_four_gigabyte_limit() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        // 0x10001 pages past the first would exceed the 65536-page ceiling.
        assert!(mem.grow(0x10001).is_err());
        assert_eq!(mem.size(), 1);
        assert!(mem.grow(1).is_ok());
        assert_eq!(mem.size(), 2);
    }

    #[test]
    fn test_imported_globals_take_their_index_slots() {
        // Imported globals come first in the index space. Skipping them shifted
        // every module-defined global, so `global.get` read a neighbour.
        use crate::runtime::core::module::{GlobalType, GlobalValue, ImportDesc};
        let module = Module {
            version: 1,
            types: vec![FunctionType {
                params: vec![],
                results: vec![ValueType::I32],
            }],
            imports: vec![ImportDesc {
                module: "env".to_string(),
                name: "imported".to_string(),
                kind: ImportKind::Global(GlobalType {
                    value_type: ValueType::I32,
                    mutable: false,
                }),
            }],
            functions: vec![Function {
                type_index: 0,
                locals: vec![],
                code: vec![0x23, 0x01, 0x0b], // global.get 1
            }],
            tables: vec![],
            memory: None,
            globals: vec![GlobalValue {
                value_type: ValueType::I32,
                mutable: false,
                init_expr: vec![0x41, 0x2a, 0x0b], // i32.const 42
            }],
            exports: HashMap::new(),
            start: None,
            elements: vec![],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        // Global 1 is the module's own; global 0 belongs to the import.
        assert_eq!(executor.execute(0).unwrap(), vec![Value::I32(42)]);
    }

    #[test]
    fn test_decode_block_type_forms() {
        // 0x40: empty
        assert_eq!(
            decode_block_type(&mut Cursor::new([0x40].as_slice())).unwrap(),
            BlockType::Empty
        );
        // Value-type shorthands, including the reference types
        assert_eq!(
            decode_block_type(&mut Cursor::new([0x7F].as_slice())).unwrap(),
            BlockType::Value(ValueType::I32)
        );
        assert_eq!(
            decode_block_type(&mut Cursor::new([0x7C].as_slice())).unwrap(),
            BlockType::Value(ValueType::F64)
        );
        assert_eq!(
            decode_block_type(&mut Cursor::new([0x70].as_slice())).unwrap(),
            BlockType::Value(ValueType::FuncRef)
        );
        // A small type index fits in one byte
        assert_eq!(
            decode_block_type(&mut Cursor::new([0x07].as_slice())).unwrap(),
            BlockType::FuncType(7)
        );
        // Index 64 needs two bytes as a signed LEB128, which the old
        // single-byte read decoded as the value type 0xC0
        assert_eq!(
            decode_block_type(&mut Cursor::new([0xC0, 0x00].as_slice())).unwrap(),
            BlockType::FuncType(64)
        );
        assert_eq!(
            decode_block_type(&mut Cursor::new([0x80, 0x02].as_slice())).unwrap(),
            BlockType::FuncType(256)
        );
        // A negative value that is not a value type is rejected
        assert!(decode_block_type(&mut Cursor::new([0x6E].as_slice())).is_err());
    }

    #[test]
    fn test_block_returning_two_values() {
        // () -> (i32, i64)
        //   block (type 1: [] -> [i32 i64])
        //     i32.const 7; i64.const 9
        //   end
        let module = multivalue_module(
            vec![],
            vec![ValueType::I32, ValueType::I64],
            vec![FunctionType {
                params: vec![],
                results: vec![ValueType::I32, ValueType::I64],
            }],
            vec![],
            vec![
                0x02, 0x01, // block (type 1)
                0x41, 0x07, // i32.const 7
                0x42, 0x09, // i64.const 9
                0x0b, // end block
                0x0b, // end function
            ],
        );
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor.execute(0).unwrap(),
            vec![Value::I32(7), Value::I64(9)]
        );
    }

    #[test]
    fn test_block_with_parameters() {
        // (i32) -> i32
        //   local.get 0; i32.const 10
        //   block (type 1: [i32 i32] -> [i32])
        //     i32.add
        //   end
        let module = multivalue_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![FunctionType {
                params: vec![ValueType::I32, ValueType::I32],
                results: vec![ValueType::I32],
            }],
            vec![],
            vec![
                0x20, 0x00, // local.get 0
                0x41, 0x0a, // i32.const 10
                0x02, 0x01, // block (type 1)
                0x6a, // i32.add
                0x0b, // end block
                0x0b, // end function
            ],
        );
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor.execute_with_args(0, vec![Value::I32(5)]).unwrap(),
            vec![Value::I32(15)]
        );
    }

    #[test]
    fn test_br_out_of_multivalue_block_keeps_both_results() {
        // () -> (i32, i32): a br carrying two values past dead code
        //   block (type 1: [] -> [i32 i32])
        //     i32.const 1; i32.const 2; br 0
        //     i32.const 99          ;; unreachable, and would be left behind
        //   end                      ;; if the branch preserved only one value
        let module = multivalue_module(
            vec![],
            vec![ValueType::I32, ValueType::I32],
            vec![FunctionType {
                params: vec![],
                results: vec![ValueType::I32, ValueType::I32],
            }],
            vec![],
            vec![
                0x02, 0x01, // block (type 1)
                0x41, 0x01, // i32.const 1
                0x41, 0x02, // i32.const 2
                0x0c, 0x00, // br 0
                0x41, 0x63, // i32.const 99 (dead)
                0x0b, // end block
                0x0b, // end function
            ],
        );
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor.execute(0).unwrap(),
            vec![Value::I32(1), Value::I32(2)]
        );
    }

    #[test]
    fn test_loop_with_parameters_carries_them_around() {
        // (i32 n) -> i32: sum 1..=n, with the accumulator and the counter as
        // the loop's own parameters. Each `br_if 0` back to the header has to
        // carry both of them; with the old fixed arity of 0 the loop would
        // restart on an empty stack.
        //
        //   i32.const 0; local.get 0
        //   loop (type 1: [i32 i32] -> [i32])   ;; [acc, i]
        //     local.set 1                        ;; local1 = i, [acc]
        //     local.get 1; i32.add               ;; [acc + i]
        //     local.get 1; i32.const 1; i32.sub
        //     local.tee 2                        ;; [acc', i']
        //     local.get 2; br_if 0               ;; back to the header with both
        //     drop                               ;; i' is 0 here, leave acc'
        //   end
        let module = multivalue_module(
            vec![ValueType::I32],
            vec![ValueType::I32],
            vec![FunctionType {
                params: vec![ValueType::I32, ValueType::I32],
                results: vec![ValueType::I32],
            }],
            vec![(2, ValueType::I32)],
            vec![
                0x41, 0x00, // i32.const 0
                0x20, 0x00, // local.get 0
                0x03, 0x01, // loop (type 1)
                0x21, 0x01, // local.set 1
                0x20, 0x01, // local.get 1
                0x6a, // i32.add
                0x20, 0x01, // local.get 1
                0x41, 0x01, // i32.const 1
                0x6b, // i32.sub
                0x22, 0x02, // local.tee 2
                0x20, 0x02, // local.get 2
                0x0d, 0x00, // br_if 0
                0x1a, // drop
                0x0b, // end loop
                0x0b, // end function
            ],
        );
        // The body counts down and exits when the counter reaches zero, so it
        // is written for n >= 1; n = 0 would wrap past the guard.
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor.execute_with_args(0, vec![Value::I32(4)]).unwrap(),
            vec![Value::I32(10)]
        );
        assert_eq!(
            executor.execute_with_args(0, vec![Value::I32(1)]).unwrap(),
            vec![Value::I32(1)]
        );
    }

    #[test]
    fn test_if_with_parameters_in_both_branches() {
        // (i32, i32, i32) -> i32
        //   local.get 0; local.get 1     ;; the if's two parameters
        //   local.get 2                  ;; the condition
        //   if (type 1: [i32 i32] -> [i32])
        //     i32.add
        //   else
        //     i32.sub
        //   end
        let module = multivalue_module(
            vec![ValueType::I32, ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![FunctionType {
                params: vec![ValueType::I32, ValueType::I32],
                results: vec![ValueType::I32],
            }],
            vec![],
            vec![
                0x20, 0x00, // local.get 0
                0x20, 0x01, // local.get 1
                0x20, 0x02, // local.get 2 (condition)
                0x04, 0x01, // if (type 1)
                0x6a, // i32.add
                0x05, // else
                0x6b, // i32.sub
                0x0b, // end if
                0x0b, // end function
            ],
        );
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor
                .execute_with_args(0, vec![Value::I32(9), Value::I32(4), Value::I32(1)])
                .unwrap(),
            vec![Value::I32(13)]
        );
        assert_eq!(
            executor
                .execute_with_args(0, vec![Value::I32(9), Value::I32(4), Value::I32(0)])
                .unwrap(),
            vec![Value::I32(5)]
        );
    }

    #[test]
    fn test_if_without_else_passes_parameters_through() {
        // (i32, i32) -> i32: an if with no else is only valid when its
        // parameters and results match, so a false condition must leave the
        // parameter on the stack as the block's result.
        //   local.get 0; local.get 1
        //   if (type 1: [i32] -> [i32])
        //     i32.const 2; i32.mul
        //   end
        let module = multivalue_module(
            vec![ValueType::I32, ValueType::I32],
            vec![ValueType::I32],
            vec![FunctionType {
                params: vec![ValueType::I32],
                results: vec![ValueType::I32],
            }],
            vec![],
            vec![
                0x20, 0x00, // local.get 0 (parameter)
                0x20, 0x01, // local.get 1 (condition)
                0x04, 0x01, // if (type 1)
                0x41, 0x02, // i32.const 2
                0x6c, // i32.mul
                0x0b, // end if
                0x0b, // end function
            ],
        );
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(
            executor
                .execute_with_args(0, vec![Value::I32(21), Value::I32(1)])
                .unwrap(),
            vec![Value::I32(42)]
        );
        assert_eq!(
            executor
                .execute_with_args(0, vec![Value::I32(21), Value::I32(0)])
                .unwrap(),
            vec![Value::I32(21)]
        );
    }

    // ---- Overlong LEB128 immediates and saturating truncation (0.23.1) ----

    #[test]
    fn test_decode_call_indirect_with_padded_table_index() {
        // wasm-ld leaves the indices it relocates encoded at their full five
        // bytes rather than compacting them, so `call_indirect (type 4)` in a
        // linked binary reads 0x11 <5-byte type> <5-byte table>. Reading the
        // table index as a single byte left four bytes of the padding to be
        // decoded as instructions, which is what made every linked rustc
        // binary die inside core::fmt::write.
        let padded = [
            0x11, // call_indirect
            0x84, 0x80, 0x80, 0x80, 0x00, // type index 4, padded
            0x80, 0x80, 0x80, 0x80, 0x00, // table index 0, padded
        ];
        let mut cursor = Cursor::new(padded.as_slice());
        assert_eq!(
            decode_instruction(&mut cursor).unwrap(),
            Instruction::CallIndirect(4, 0)
        );
        // The whole immediate must be consumed, or the next decode starts
        // mid-number and everything after it is garbage.
        assert_eq!(cursor.position(), padded.len() as u64);
    }

    #[test]
    fn test_decode_memory_ops_with_padded_indices() {
        // The same padding reaches every memory index.
        let cases: Vec<(Vec<u8>, Instruction)> = vec![
            (
                vec![0x3F, 0x80, 0x80, 0x80, 0x80, 0x00],
                Instruction::MemorySize,
            ),
            (
                vec![0x40, 0x80, 0x80, 0x80, 0x80, 0x00],
                Instruction::MemoryGrow,
            ),
            (
                vec![
                    0xFC, 0x0A, 0x80, 0x80, 0x80, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00,
                ],
                Instruction::MemoryCopy,
            ),
            (
                vec![0xFC, 0x0B, 0x80, 0x80, 0x80, 0x80, 0x00],
                Instruction::MemoryFill,
            ),
            (
                vec![0xFC, 0x08, 0x03, 0x80, 0x80, 0x80, 0x80, 0x00],
                Instruction::MemoryInit(3),
            ),
        ];
        for (bytes, expected) in cases {
            let mut cursor = Cursor::new(bytes.as_slice());
            assert_eq!(decode_instruction(&mut cursor).unwrap(), expected);
            assert_eq!(
                cursor.position(),
                bytes.len() as u64,
                "immediate not fully consumed for {expected:?}"
            );
        }
    }

    #[test]
    fn test_call_indirect_executes_with_padded_immediates() {
        use crate::runtime::core::module::ElementSegment;
        // The end-to-end form of the linked-binary bug: function 1 is reached
        // through call_indirect whose type and table immediates are both padded
        // to five bytes. Before the fix the decoder consumed one byte of the
        // table index and then read the remaining padding as instructions,
        // which is how every linked rustc binary derailed.
        let module = Module {
            version: 1,
            types: vec![
                // type 0: the caller, () -> i32
                FunctionType {
                    params: vec![],
                    results: vec![ValueType::I32],
                },
                // type 1: the callee's signature, () -> i32
                FunctionType {
                    params: vec![],
                    results: vec![ValueType::I32],
                },
            ],
            imports: vec![],
            functions: vec![
                Function {
                    type_index: 0,
                    locals: vec![],
                    code: vec![
                        0x41, 0x00, // i32.const 0 (table slot)
                        0x11, // call_indirect
                        0x81, 0x80, 0x80, 0x80, 0x00, // type index 1, padded
                        0x80, 0x80, 0x80, 0x80, 0x00, // table index 0, padded
                        0x0b, // end
                    ],
                },
                Function {
                    type_index: 1,
                    locals: vec![],
                    code: vec![0x41, 0x2a, 0x0b], // i32.const 42; end
                },
            ],
            tables: vec![TableType {
                initial: 1,
                max: None,
                element_type: ValueType::FuncRef,
            }],
            memory: None,
            globals: vec![],
            exports: HashMap::new(),
            start: None,
            // Active segment placing function 1 in slot 0.
            elements: vec![ElementSegment {
                offset_expr: vec![0x41, 0x00, 0x0b],
                table_index: 0,
                declarative: false,
                function_indices: vec![Some(1)],
            }],
            data: vec![],
        };
        let mut executor = Executor::new(module).unwrap();
        assert_eq!(executor.execute(0).unwrap(), vec![Value::I32(42)]);
    }

    #[test]
    fn test_decode_trunc_sat() {
        let cases = [
            (0x00, Instruction::I32TruncSatF32S),
            (0x01, Instruction::I32TruncSatF32U),
            (0x02, Instruction::I32TruncSatF64S),
            (0x03, Instruction::I32TruncSatF64U),
            (0x04, Instruction::I64TruncSatF32S),
            (0x05, Instruction::I64TruncSatF32U),
            (0x06, Instruction::I64TruncSatF64S),
            (0x07, Instruction::I64TruncSatF64U),
        ];
        for (op, expected) in cases {
            assert_eq!(
                decode_instruction(&mut Cursor::new([0xFC, op].as_slice())).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn test_trunc_sat_clamps_instead_of_trapping() {
        // (f64) -> i32 : f64.const arg; i32.trunc_sat_f64_s
        fn run_i32_s(input: f64) -> Value {
            let module = multivalue_module(
                vec![ValueType::F64],
                vec![ValueType::I32],
                vec![],
                vec![],
                vec![
                    0x20, 0x00, // local.get 0
                    0xFC, 0x02, // i32.trunc_sat_f64_s
                    0x0b, // end
                ],
            );
            let mut executor = Executor::new(module).unwrap();
            executor
                .execute_with_args(0, vec![Value::F64(input)])
                .unwrap()[0]
        }

        assert_eq!(run_i32_s(3.9), Value::I32(3));
        assert_eq!(run_i32_s(-3.9), Value::I32(-3));
        // NaN saturates to zero rather than trapping, which is the whole point
        assert_eq!(run_i32_s(f64::NAN), Value::I32(0));
        // Out of range clamps to the bounds
        assert_eq!(run_i32_s(1e30), Value::I32(i32::MAX));
        assert_eq!(run_i32_s(-1e30), Value::I32(i32::MIN));
        assert_eq!(run_i32_s(f64::INFINITY), Value::I32(i32::MAX));
        assert_eq!(run_i32_s(f64::NEG_INFINITY), Value::I32(i32::MIN));

        // The trapping form still rejects what the saturating one accepts
        let trapping = multivalue_module(
            vec![ValueType::F64],
            vec![ValueType::I32],
            vec![],
            vec![],
            vec![0x20, 0x00, 0xAA, 0x0b], // local.get 0; i32.trunc_f64_s
        );
        let mut executor = Executor::new(trapping).unwrap();
        assert!(executor
            .execute_with_args(0, vec![Value::F64(f64::NAN)])
            .is_err());
    }

    #[test]
    fn test_trunc_sat_unsigned_forms() {
        fn run(opcode: u8, input: f64, result_is_i64: bool) -> Value {
            let module = multivalue_module(
                vec![ValueType::F64],
                vec![if result_is_i64 {
                    ValueType::I64
                } else {
                    ValueType::I32
                }],
                vec![],
                vec![],
                vec![0x20, 0x00, 0xFC, opcode, 0x0b],
            );
            let mut executor = Executor::new(module).unwrap();
            executor
                .execute_with_args(0, vec![Value::F64(input)])
                .unwrap()[0]
        }

        // i32.trunc_sat_f64_u: negatives clamp to 0, overflow to u32::MAX
        assert_eq!(run(0x03, -3.9, false), Value::I32(0));
        assert_eq!(run(0x03, 1e30, false), Value::I32(u32::MAX as i32));
        // i64.trunc_sat_f64_u
        assert_eq!(run(0x07, -1.0, true), Value::I64(0));
        assert_eq!(run(0x07, 1e30, true), Value::I64(u64::MAX as i64));
        // i64.trunc_sat_f64_s
        assert_eq!(run(0x06, -1e30, true), Value::I64(i64::MIN));
    }

    #[test]
    fn test_block_type_index_out_of_bounds_is_rejected() {
        let module = multivalue_module(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![0x02, 0x09, 0x0b, 0x0b], // block (type 9), which does not exist
        );
        let mut executor = Executor::new(module).unwrap();
        let err = executor.execute(0).unwrap_err();
        assert!(
            err.contains("Block type index 9 out of bounds"),
            "got: {err}"
        );
    }
}
