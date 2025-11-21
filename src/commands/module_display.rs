//! Display helpers for formatting WASM module information

use crate::runtime::core::module::{ExportKind, ImportKind, Module, ValueType};

/// Format a value type for display
pub fn format_value_type(vt: ValueType) -> &'static str {
    match vt {
        ValueType::I32 => "i32",
        ValueType::I64 => "i64",
        ValueType::F32 => "f32",
        ValueType::F64 => "f64",
        ValueType::V128 => "v128",
        ValueType::FuncRef => "funcref",
        ValueType::ExternRef => "externref",
    }
}

/// Format a function signature
pub fn format_function_signature(params: &[ValueType], results: &[ValueType]) -> String {
    let param_str = if params.is_empty() {
        "".to_string()
    } else {
        params
            .iter()
            .map(|&t| format_value_type(t))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let result_str = if results.is_empty() {
        "void".to_string()
    } else {
        results
            .iter()
            .map(|&t| format_value_type(t))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!("({param_str}) -> {result_str}")
}

/// Display module types with signatures
pub fn display_types(module: &Module) {
    if module.types.is_empty() {
        println!("  📋 Function Types: 0");
        return;
    }

    println!("  📋 Function Types: {}", module.types.len());
    for (idx, func_type) in module.types.iter().enumerate() {
        let sig = format_function_signature(&func_type.params, &func_type.results);
        println!("     type[{idx}] {sig}");
    }
}

/// Display module imports
pub fn display_imports(module: &Module) {
    if module.imports.is_empty() {
        println!("  📥 Imports: 0");
        return;
    }

    println!("  📥 Imports: {}", module.imports.len());

    // Group by kind
    let mut functions = 0;
    let mut tables = 0;
    let mut memory = 0;
    let mut globals = 0;

    for import in &module.imports {
        match import.kind {
            ImportKind::Function(_) => functions += 1,
            ImportKind::Table(_) => tables += 1,
            ImportKind::Memory(_) => memory += 1,
            ImportKind::Global(_) => globals += 1,
        }
    }

    if functions > 0 {
        println!("     ├─ Functions: {functions}");
    }
    if tables > 0 {
        println!("     ├─ Tables: {tables}");
    }
    if memory > 0 {
        println!("     ├─ Memory: {memory}");
    }
    if globals > 0 {
        println!("     └─ Globals: {globals}");
    }

    // Show first few imports
    for (idx, import) in module.imports.iter().enumerate().take(5) {
        let kind_str = match import.kind {
            ImportKind::Function(_) => "func",
            ImportKind::Table(_) => "table",
            ImportKind::Memory(_) => "memory",
            ImportKind::Global(_) => "global",
        };
        println!(
            "     [{idx}] {} from {}.{}",
            kind_str, import.module, import.name
        );
    }

    if module.imports.len() > 5 {
        println!("     ... and {} more", module.imports.len() - 5);
    }
}

/// Display module functions
pub fn display_functions(module: &Module) {
    let function_count = module.functions.len();
    if function_count == 0 {
        println!("  🔧 Functions: 0");
        return;
    }

    println!("  🔧 Functions: {function_count}");

    // Calculate code statistics
    let total_code_size: usize = module.functions.iter().map(|f| f.code.len()).sum();
    let avg_size = if function_count > 0 {
        total_code_size / function_count
    } else {
        0
    };

    let max_size = module
        .functions
        .iter()
        .map(|f| f.code.len())
        .max()
        .unwrap_or(0);
    let min_size = module
        .functions
        .iter()
        .map(|f| f.code.len())
        .min()
        .unwrap_or(0);

    println!("     ├─ Code size: {total_code_size} bytes");
    println!("     ├─ Average function size: {avg_size} bytes");
    println!("     ├─ Largest function: {max_size} bytes");
    println!("     └─ Smallest function: {min_size} bytes");
}

/// Display module exports
pub fn display_exports(module: &Module) {
    if module.exports.is_empty() {
        println!("  📤 Exports: 0");
        return;
    }

    println!("  📤 Exports: {}", module.exports.len());

    // Group by kind
    let mut func_exports = Vec::new();
    let mut table_exports = Vec::new();
    let mut memory_exports = Vec::new();
    let mut global_exports = Vec::new();

    for (name, desc) in &module.exports {
        match desc.kind {
            ExportKind::Function => func_exports.push(name.clone()),
            ExportKind::Table => table_exports.push(name.clone()),
            ExportKind::Memory => memory_exports.push(name.clone()),
            ExportKind::Global => global_exports.push(name.clone()),
        }
    }

    if !func_exports.is_empty() {
        println!(
            "     ├─ Functions ({}): {}",
            func_exports.len(),
            func_exports.join(", ")
        );
    }
    if !table_exports.is_empty() {
        println!(
            "     ├─ Tables ({}): {}",
            table_exports.len(),
            table_exports.join(", ")
        );
    }
    if !memory_exports.is_empty() {
        println!(
            "     ├─ Memory ({}): {}",
            memory_exports.len(),
            memory_exports.join(", ")
        );
    }
    if !global_exports.is_empty() {
        println!(
            "     └─ Globals ({}): {}",
            global_exports.len(),
            global_exports.join(", ")
        );
    }
}

/// Display module globals
pub fn display_globals(module: &Module) {
    if module.globals.is_empty() {
        println!("  🌐 Globals: 0");
        return;
    }

    println!("  🌐 Globals: {}", module.globals.len());
    for (idx, global) in module.globals.iter().enumerate().take(5) {
        let mutability = if global.mutable {
            "mutable"
        } else {
            "immutable"
        };
        let type_str = format_value_type(global.value_type);
        println!("     [{idx}] {type_str} ({mutability})");
    }

    if module.globals.len() > 5 {
        println!("     ... and {} more", module.globals.len() - 5);
    }
}

/// Display memory information
pub fn display_memory(module: &Module) {
    match &module.memory {
        Some(mem) => {
            println!("  💾 Memory:");
            let initial_bytes = mem.initial * 65536;
            println!(
                "     ├─ Initial: {} page(s) ({} bytes)",
                mem.initial, initial_bytes
            );
            if let Some(max) = mem.max {
                let max_bytes = max * 65536;
                println!("     └─ Maximum: {max} page(s) ({max_bytes} bytes)");
            } else {
                println!("     └─ Maximum: unbounded");
            }
        }
        None => {
            println!("  💾 Memory: none");
        }
    }
}

/// Display data segments
pub fn display_data_segments(module: &Module) {
    if module.data.is_empty() {
        println!("  📦 Data Segments: 0");
        return;
    }

    println!("  📦 Data Segments: {}", module.data.len());
    let total_data: usize = module.data.iter().map(|d| d.data.len()).sum();
    println!("     Total data: {total_data} bytes");

    for (idx, segment) in module.data.iter().enumerate().take(3) {
        println!("     [{idx}] {} bytes", segment.data.len());
    }

    if module.data.len() > 3 {
        println!("     ... and {} more", module.data.len() - 3);
    }
}

/// Display element segments
pub fn display_element_segments(module: &Module) {
    if module.elements.is_empty() {
        println!("  🔗 Element Segments: 0");
        return;
    }

    println!("  🔗 Element Segments: {}", module.elements.len());
    let total_funcs: usize = module
        .elements
        .iter()
        .map(|e| e.function_indices.len())
        .sum();
    println!("     Total function references: {total_funcs}");
}

/// Display complete module summary
pub fn display_module_summary(module: &Module) {
    println!("\n  ╭ Module Analysis");
    println!("  │");
    println!("  ├─ Version: {}", module.version);

    display_types(module);
    display_imports(module);
    display_functions(module);
    display_exports(module);
    display_globals(module);
    display_memory(module);
    display_data_segments(module);
    display_element_segments(module);

    // Summary stats
    println!("  │");
    println!(
        "  └─ Start function: {}",
        module
            .start
            .map(|idx| idx.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!("  ╰\n");
}
