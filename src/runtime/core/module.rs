use std::collections::HashMap;
use std::io::{Cursor, Read};

const WASM_MAGIC_BYTES: &[u8; 4] = b"\0asm";
const WASM_VERSION: u32 = 1;

/// Function signature describing parameter and return types
#[derive(Debug, Clone)]
pub struct FunctionType {
    pub params: Vec<ValueType>,
    pub results: Vec<ValueType>,
}

/// Value types in WASM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
    // Vector types (SIMD extension)
    V128 = 0x7B,
    // Reference types (WASM spec extension)
    FuncRef = 0x70,
    ExternRef = 0x6F,
}

impl ValueType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x7F => Some(ValueType::I32),
            0x7E => Some(ValueType::I64),
            0x7D => Some(ValueType::F32),
            0x7C => Some(ValueType::F64),
            0x7B => Some(ValueType::V128),
            0x70 => Some(ValueType::FuncRef),
            0x6F => Some(ValueType::ExternRef),
            _ => None,
        }
    }
}

/// Import description
#[derive(Debug, Clone)]
pub struct ImportDesc {
    pub module: String,
    pub name: String,
    pub kind: ImportKind,
}

#[derive(Debug, Clone)]
pub enum ImportKind {
    Function(u32), // type index
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
}

/// Export description
#[derive(Debug, Clone)]
pub struct ExportDesc {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

#[derive(Debug, Clone)]
pub enum ExportKind {
    Function,
    Table,
    Memory,
    Global,
}

/// Function definition
#[derive(Debug, Clone)]
pub struct Function {
    pub type_index: u32,
    pub locals: Vec<(u32, ValueType)>, // (count, type)
    pub code: Vec<u8>,
}

/// Memory type specification
#[derive(Debug, Clone)]
pub struct MemoryType {
    pub initial: u32,
    pub max: Option<u32>,
}

/// Table type specification
#[derive(Debug, Clone)]
pub struct TableType {
    pub initial: u32,
    pub max: Option<u32>,
}

/// Global variable with value and mutability
#[derive(Debug, Clone)]
pub struct GlobalValue {
    pub mutable: bool,
    pub value_type: ValueType,
    pub init_expr: Vec<u8>,
}

/// Data segment for memory initialization
#[derive(Debug, Clone)]
pub struct DataSegment {
    pub offset_expr: Vec<u8>,
    pub data: Vec<u8>,
}

/// Element segment for table initialization
#[derive(Debug, Clone)]
pub struct ElementSegment {
    pub offset_expr: Vec<u8>,
    pub function_indices: Vec<u32>,
}

/// Parsed WASM module
#[derive(Debug)]
pub struct Module {
    pub version: u32,
    pub types: Vec<FunctionType>,
    pub imports: Vec<ImportDesc>,
    pub functions: Vec<Function>,
    pub tables: Vec<TableType>,
    pub memory: Option<MemoryType>,
    pub globals: Vec<GlobalValue>,
    pub exports: HashMap<String, ExportDesc>,
    pub start: Option<u32>,
    pub elements: Vec<ElementSegment>,
    pub data: Vec<DataSegment>,
}

impl Module {
    /// Parse a WASM module from bytes
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor::new(bytes);
        let mut module = Module {
            version: 0,
            types: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memory: None,
            globals: Vec::new(),
            exports: HashMap::new(),
            start: None,
            elements: Vec::new(),
            data: Vec::new(),
        };

        // Verify magic bytes and version
        let mut magic = [0u8; 4];
        cursor
            .read_exact(&mut magic)
            .map_err(|_| "File too small")?;
        if &magic != WASM_MAGIC_BYTES {
            return Err("Invalid WASM magic bytes".to_string());
        }

        // Version is 4 fixed bytes (little-endian u32), not LEB128!
        let mut version_bytes = [0u8; 4];
        cursor
            .read_exact(&mut version_bytes)
            .map_err(|_| "File too small - missing version")?;
        module.version = u32::from_le_bytes(version_bytes);
        if module.version != WASM_VERSION {
            return Err(format!("Unsupported WASM version: {}", module.version));
        }

        // Parse sections
        let bytes = bytes.to_vec();
        let total_len = bytes.len();
        let mut pos = cursor.position() as usize;

        while pos < total_len {
            // Read section ID
            let mut section_cursor = Cursor::new(bytes[pos..].to_vec());
            let section_id = read_leb128_u32(&mut section_cursor)?;
            pos += section_cursor.position() as usize;

            // Read section size
            section_cursor = Cursor::new(bytes[pos..].to_vec());
            let section_size = read_leb128_u32(&mut section_cursor)? as usize;
            pos += section_cursor.position() as usize;

            let section_end = pos + section_size;
            if section_end > total_len {
                return Err(format!(
                    "Section {section_id} extends beyond end of module (pos={pos}, size={section_size}, total={total_len})"
                ));
            }

            let section_data = &bytes[pos..section_end];

            match section_id {
                0 => {
                    // Custom section - skip
                }
                1 => {
                    // Type section
                    module.types = parse_type_section(section_data)?;
                }
                2 => {
                    // Import section
                    module.imports = parse_import_section(section_data, &module.types)?;
                }
                3 => {
                    // Function section
                    module.functions = parse_function_section(section_data)?;
                }
                4 => {
                    // Table section
                    module.tables = parse_table_section(section_data)?;
                }
                5 => {
                    // Memory section
                    module.memory = parse_memory_section(section_data)?;
                }
                6 => {
                    // Global section
                    module.globals = parse_global_section(section_data)?;
                }
                7 => {
                    // Export section
                    module.exports = parse_export_section(section_data)?;
                }
                8 => {
                    // Start section
                    let mut c = Cursor::new(section_data.to_vec());
                    module.start = Some(read_leb128_u32(&mut c)?);
                }
                9 => {
                    // Element section
                    module.elements = parse_element_section(section_data)?;
                }
                10 => {
                    // Code section - merge with function section
                    let code_bodies = parse_code_section(section_data)?;
                    for (i, body) in code_bodies.into_iter().enumerate() {
                        if i < module.functions.len() {
                            module.functions[i].code = body;
                        }
                    }
                }
                11 => {
                    // Data section
                    module.data = parse_data_section(section_data)?;
                }
                12 => {
                    // DataCount section - skip for now
                }
                _ => {
                    // Unknown section - skip
                }
            }

            pos = section_end;
        }

        Ok(module)
    }

    /// Create an empty module (useful for testing)
    pub fn new() -> Self {
        Module {
            version: 1,
            types: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memory: None,
            globals: Vec::new(),
            exports: HashMap::new(),
            start: None,
            elements: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Get function by index, accounting for imported functions
    pub fn get_function(&self, idx: u32) -> Option<&Function> {
        let import_count = self
            .imports
            .iter()
            .filter(|i| matches!(i.kind, ImportKind::Function(_)))
            .count();
        if (idx as usize) < import_count {
            None // Imported function
        } else {
            self.functions.get((idx as usize) - import_count)
        }
    }

    /// Find entry point: look for _start, then main, then first export
    pub fn find_entry_point(&self) -> Option<u32> {
        // First check for start section
        if let Some(start) = self.start {
            return Some(start);
        }

        // Look for _start or main in exports
        for (name, export) in &self.exports {
            if matches!(export.kind, ExportKind::Function) && (name == "_start" || name == "main") {
                return Some(export.index);
            }
        }

        None
    }
}

/// Parse Type section (function signatures)
fn parse_type_section(data: &[u8]) -> Result<Vec<FunctionType>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut types = Vec::with_capacity(count);
    for _ in 0..count {
        let form = read_u8(&mut cursor)?;
        if form != 0x60 {
            return Err("Invalid function type form".to_string());
        }

        let param_count = read_leb128_u32(&mut cursor)? as usize;
        let mut params = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            let val_type = read_value_type(&mut cursor)?;
            params.push(val_type);
        }

        let result_count = read_leb128_u32(&mut cursor)? as usize;
        let mut results = Vec::with_capacity(result_count);
        for _ in 0..result_count {
            let val_type = read_value_type(&mut cursor)?;
            results.push(val_type);
        }

        types.push(FunctionType { params, results });
    }

    Ok(types)
}

/// Parse Import section
fn parse_import_section(data: &[u8], _types: &[FunctionType]) -> Result<Vec<ImportDesc>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut imports = Vec::with_capacity(count);
    for _ in 0..count {
        let module = read_string(&mut cursor)?;
        let name = read_string(&mut cursor)?;
        let kind_byte = read_u8(&mut cursor)?;

        let kind = match kind_byte {
            0x00 => {
                // Function import
                let type_idx = read_leb128_u32(&mut cursor)?;
                ImportKind::Function(type_idx)
            }
            0x01 => {
                // Table import
                let elem_type = read_u8(&mut cursor)?;
                // Accept 0x70 (funcref) and 0x6f (externref)
                if elem_type != 0x70 && elem_type != 0x6f {
                    return Err(format!("Invalid element type for table: 0x{elem_type:02x}"));
                }
                let limits = read_limits(&mut cursor)?;
                ImportKind::Table(TableType {
                    initial: limits.0,
                    max: limits.1,
                })
            }
            0x02 => {
                // Memory import
                let limits = read_limits(&mut cursor)?;
                ImportKind::Memory(MemoryType {
                    initial: limits.0,
                    max: limits.1,
                })
            }
            0x03 => {
                // Global import
                let byte = read_u8(&mut cursor)?;
                let val_type = ValueType::from_byte(byte)
                    .ok_or_else(|| format!("Invalid value type in global import: 0x{byte:02x}"))?;
                let mutable = read_u8(&mut cursor)? != 0;
                ImportKind::Global(GlobalType {
                    value_type: val_type,
                    mutable,
                })
            }
            _ => return Err(format!("Invalid import kind: {kind_byte}")),
        };

        imports.push(ImportDesc { module, name, kind });
    }

    Ok(imports)
}

/// Parse Function section (type indices only; code comes from Code section)
fn parse_function_section(data: &[u8]) -> Result<Vec<Function>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut functions = Vec::with_capacity(count);
    for _ in 0..count {
        let type_index = read_leb128_u32(&mut cursor)?;
        // Don't parse locals here - they're in the Code section
        functions.push(Function {
            type_index,
            locals: Vec::new(),
            code: Vec::new(),
        });
    }

    Ok(functions)
}

/// Parse Code section (function bodies)
fn parse_code_section(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut bodies = Vec::with_capacity(count);
    for _ in 0..count {
        let body_size = read_leb128_u32(&mut cursor)? as usize;
        let body_start = cursor.position() as usize;
        let body_end = body_start + body_size;

        if body_end > data.len() {
            return Err("Code section overflow".to_string());
        }

        // Just save the entire body for now, including locals
        // We'll parse locals when executing
        bodies.push(data[body_start..body_end].to_vec());

        cursor.set_position(body_end as u64);
    }

    Ok(bodies)
}

/// Parse Table section
fn parse_table_section(data: &[u8]) -> Result<Vec<TableType>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut tables = Vec::with_capacity(count);
    for _ in 0..count {
        let _elem_type = read_u8(&mut cursor)?; // 0x70 for funcref, 0x6f for externref, etc.
        let limits = read_limits(&mut cursor)?;
        tables.push(TableType {
            initial: limits.0,
            max: limits.1,
        });
    }

    Ok(tables)
}

/// Parse Memory section
fn parse_memory_section(data: &[u8]) -> Result<Option<MemoryType>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)? as usize;

    if count == 0 {
        return Ok(None);
    }

    if count > 1 {
        return Err("Multiple memories not supported".to_string());
    }

    let limits = read_limits(&mut cursor)?;
    Ok(Some(MemoryType {
        initial: limits.0,
        max: limits.1,
    }))
}

/// Parse Global section
fn parse_global_section(data: &[u8]) -> Result<Vec<GlobalValue>, String> {
    let mut cursor = Cursor::new(data.to_vec());
    let section_end = data.len();
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut globals = Vec::with_capacity(count);
    for idx in 0..count {
        let byte = read_u8(&mut cursor)?;
        let val_type = ValueType::from_byte(byte)
            .ok_or_else(|| format!("Invalid value type in global[{idx}]: 0x{byte:02x}"))?;
        let mutable = read_u8(&mut cursor)? != 0;

        // Read init expression until 0x0b (end) with bounds checking
        let init_expr = parse_expression(&mut cursor, section_end)?;

        globals.push(GlobalValue {
            mutable,
            value_type: val_type,
            init_expr,
        });
    }

    Ok(globals)
}

/// Parse Export section
fn parse_export_section(data: &[u8]) -> Result<HashMap<String, ExportDesc>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_leb128_u32(&mut cursor)
        .map_err(|e| format!("Failed to read export count: {e}"))? as usize;

    let mut exports = HashMap::with_capacity(count);
    for i in 0..count {
        let name = read_string(&mut cursor)
            .map_err(|e| format!("Failed to read export[{i}] name: {e}"))?;
        let kind_byte =
            read_u8(&mut cursor).map_err(|e| format!("Failed to read export[{i}] kind: {e}"))?;
        let index = read_leb128_u32(&mut cursor)
            .map_err(|e| format!("Failed to read export[{i}] index: {e}"))?;

        let kind = match kind_byte {
            0x00 => ExportKind::Function,
            0x01 => ExportKind::Table,
            0x02 => ExportKind::Memory,
            0x03 => ExportKind::Global,
            _ => return Err(format!("Invalid export kind in export[{i}]: {kind_byte}")),
        };

        exports.insert(name.clone(), ExportDesc { name, kind, index });
    }

    Ok(exports)
}

/// Parse Element section (table initialization)
fn parse_element_section(data: &[u8]) -> Result<Vec<ElementSegment>, String> {
    let mut cursor = Cursor::new(data.to_vec());
    let section_end = data.len();
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut elements = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = read_leb128_u32(&mut cursor)?;

        // If flags has bit 2 set, there's a type field
        let _type_field = if (flags & 0x04) != 0 {
            Some(read_u8(&mut cursor)?)
        } else {
            None
        };

        // Parse offset expression (unless passive segment)
        let offset_expr = if (flags & 0x01) == 0 {
            // Active segment - has offset expression
            parse_expression(&mut cursor, section_end)?
        } else {
            // Passive segment - no offset expression
            Vec::new()
        };

        // Parse indices/functions
        let count = read_leb128_u32(&mut cursor)? as usize;
        let mut function_indices = Vec::with_capacity(count);
        for _ in 0..count {
            function_indices.push(read_leb128_u32(&mut cursor)?);
        }

        elements.push(ElementSegment {
            offset_expr,
            function_indices,
        });
    }

    Ok(elements)
}

/// Parse Data section (memory initialization)
fn parse_data_section(data: &[u8]) -> Result<Vec<DataSegment>, String> {
    let mut cursor = Cursor::new(data.to_vec());
    let section_end = data.len();
    let count = read_leb128_u32(&mut cursor)? as usize;

    let mut segments = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = read_leb128_u32(&mut cursor)?;

        // Parse offset expression (unless passive segment)
        let offset_expr = if (flags & 0x01) == 0 {
            // Active segment - has offset expression
            parse_expression(&mut cursor, section_end)?
        } else {
            // Passive segment - no offset expression
            Vec::new()
        };

        // Parse data bytes
        let size = read_leb128_u32(&mut cursor)? as usize;
        let current_pos = cursor.position() as usize;

        // Validate we have enough data remaining
        if current_pos + size > section_end {
            return Err(format!(
                "Data segment size ({}) exceeds available section data (only {} bytes remaining)",
                size,
                section_end - current_pos
            ));
        }

        let mut data_bytes = vec![0u8; size];
        cursor
            .read_exact(&mut data_bytes)
            .map_err(|_| "Failed to read data segment bytes")?;

        segments.push(DataSegment {
            offset_expr,
            data: data_bytes,
        });
    }

    Ok(segments)
}

/// Helper: Global type for imports
#[derive(Debug, Clone)]
pub struct GlobalType {
    pub value_type: ValueType,
    pub mutable: bool,
}

// Helper functions

fn read_u8<T: Read>(cursor: &mut T) -> Result<u8, String> {
    let mut byte = [0u8; 1];
    cursor.read_exact(&mut byte).map_err(|_| "Unexpected EOF")?;
    Ok(byte[0])
}

fn read_leb128_u32<T: Read>(cursor: &mut T) -> Result<u32, String> {
    let mut result = 0u32;
    let mut shift = 0;

    loop {
        let byte = read_u8(cursor)?;
        result |= ((byte & 0x7F) as u32) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }

        if shift >= 32 {
            return Err("Invalid LEB128 encoding".to_string());
        }
    }

    Ok(result)
}

fn read_string<T: Read>(cursor: &mut T) -> Result<String, String> {
    let len = read_leb128_u32(cursor)? as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).map_err(|_| "Unexpected EOF")?;
    String::from_utf8(buf).map_err(|_| "Invalid UTF-8 in string".to_string())
}

fn read_value_type<T: Read>(cursor: &mut T) -> Result<ValueType, String> {
    let byte = read_u8(cursor)?;
    ValueType::from_byte(byte)
        .ok_or_else(|| format!("Invalid/unsupported value type: 0x{byte:02x}. Note: Some modern WASM features may not be fully supported yet."))
}

fn read_limits<T: Read>(cursor: &mut T) -> Result<(u32, Option<u32>), String> {
    let flags = read_u8(cursor)?;
    let initial = read_leb128_u32(cursor)?;
    let max = if flags & 0x01 != 0 {
        Some(read_leb128_u32(cursor)?)
    } else {
        None
    };
    Ok((initial, max))
}

/// Safely parse an expression with bounds checking
/// Expressions are terminated by 0x0b (END opcode)
fn parse_expression(cursor: &mut Cursor<Vec<u8>>, section_end: usize) -> Result<Vec<u8>, String> {
    let mut expr = Vec::new();
    const MAX_EXPR_SIZE: usize = 16384; // Reasonable limit for expressions

    loop {
        let current_pos = cursor.position() as usize;

        // Check if we've hit the section boundary
        if current_pos >= section_end {
            return Err(
                "Expression parsing exceeded section boundary - missing END marker (0x0b)"
                    .to_string(),
            );
        }

        // Check if expression is getting too large (likely infinite loop)
        if expr.len() > MAX_EXPR_SIZE {
            return Err(format!(
                "Expression too large ({} bytes) - likely missing END marker (0x0b)",
                expr.len()
            ));
        }

        let byte = read_u8(cursor)?;
        expr.push(byte);

        // 0x0b is the END opcode that terminates all expressions
        if byte == 0x0b {
            break;
        }
    }

    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_wasm() {
        // Minimal valid WASM: just magic bytes and version with no sections
        let mut bytes = vec![0x00, 0x61, 0x73, 0x6d]; // magic: \0asm
        // Version is 4 fixed bytes in little-endian format (0x00, 0x01, 0x00, 0x00 = version 1)
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

        let module = Module::parse(&bytes).expect("Should parse minimal WASM");
        assert_eq!(module.version, 1);
        assert_eq!(module.types.len(), 0);
        assert_eq!(module.imports.len(), 0);
    }

    #[test]
    fn test_invalid_magic_bytes() {
        let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00];
        let result = Module::parse(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_value_type_from_byte() {
        assert_eq!(ValueType::from_byte(0x7F), Some(ValueType::I32));
        assert_eq!(ValueType::from_byte(0x7E), Some(ValueType::I64));
        assert_eq!(ValueType::from_byte(0x7D), Some(ValueType::F32));
        assert_eq!(ValueType::from_byte(0x7C), Some(ValueType::F64));
        assert_eq!(ValueType::from_byte(0xFF), None);
    }
}
