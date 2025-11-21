use crate::cli::CommandValidator;
use crate::config::WASM_MAGIC_BYTES;
use crate::error::{Result, WasmError, WasmrunError};
use crate::runtime::core::module::Module;
use crate::utils::PathResolver;
use crate::commands::{module_display, issue_detector};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

/// WASM section
#[derive(Debug)]
pub struct WasmSection {
    pub id: u32,
    pub size: usize,
    pub name: String,
}

/// Verification results
#[derive(Debug)]
pub struct VerificationResult {
    pub valid_magic: bool,
    pub file_size: usize,
    pub section_count: usize,
    pub sections: Vec<WasmSection>,
    pub has_export_section: bool,
    pub export_names: Vec<String>,
    pub has_start_section: bool,
    pub start_function_index: Option<u32>,
    pub has_memory_section: bool,
    pub memory_limits: Option<(u32, Option<u32>)>,
    pub has_table_section: bool,
    pub function_count: usize,
}

/// Handle verify command
pub fn handle_verify_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    detailed: bool,
) -> Result<()> {
    let wasm_path = resolve_and_validate_wasm_path(path, positional_path)?;

    println!("🔍 Verifying WebAssembly file: {wasm_path}");

    let result =
        verify_wasm(&wasm_path).map_err(|e| WasmrunError::Wasm(WasmError::validation_failed(e)))?;

    print_verification_results(&wasm_path, &result, detailed);

    if !result.valid_magic {
        return Err(WasmrunError::Wasm(WasmError::InvalidMagicBytes {
            path: wasm_path,
        }));
    }

    if result.section_count == 0 {
        return Err(WasmrunError::Wasm(WasmError::validation_failed(
            "No sections found in WASM file",
        )));
    }

    // Show detailed module analysis if requested
    if detailed {
        if let Ok(wasm_bytes) = fs::read(&wasm_path) {
            if let Ok(module) = Module::parse(&wasm_bytes) {
                module_display::display_module_summary(&module);
            }
        }
    }

    Ok(())
}

/// Handle inspect command
pub fn handle_inspect_command(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<()> {
    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    PathResolver::validate_wasm_file(&wasm_path)?;

    println!("🔍 Inspecting WebAssembly file: {wasm_path}\n");

    // Show binary information
    print_detailed_binary_info(&wasm_path)
        .map_err(|e| WasmrunError::Wasm(WasmError::validation_failed(e)))?;

    // Also show parsed module analysis
    if let Ok(wasm_bytes) = fs::read(&wasm_path) {
        if let Ok(module) = Module::parse(&wasm_bytes) {
            println!("\n📊 Parsed Module Analysis:");
            module_display::display_module_summary(&module);
        }
    }

    Ok(())
}

/// Verify a WebAssembly file
pub fn verify_wasm(path: &str) -> std::result::Result<VerificationResult, String> {
    if !Path::new(path).exists() {
        return Err(format!("File not found: {path}"));
    }

    let wasm_bytes = fs::read(path).map_err(|e| format!("Error reading file: {e}"))?;

    if wasm_bytes.len() < 8 {
        return Err("File is too small to be a valid WASM module".to_string());
    }

    let valid_magic = wasm_bytes.starts_with(&WASM_MAGIC_BYTES);

    if !valid_magic {
        return Ok(VerificationResult {
            valid_magic: false,
            file_size: wasm_bytes.len(),
            section_count: 0,
            sections: vec![],
            has_export_section: false,
            export_names: vec![],
            has_start_section: false,
            start_function_index: None,
            has_memory_section: false,
            memory_limits: None,
            has_table_section: false,
            function_count: 0,
        });
    }

    let mut reader = Cursor::new(wasm_bytes.clone());

    reader.set_position(8);

    let mut sections = Vec::new();
    let mut has_export_section = false;
    let mut export_names = Vec::new();
    let mut has_start_section = false;
    let mut start_function_index = None;
    let mut has_memory_section = false;
    let mut memory_limits = None;
    let mut has_table_section = false;
    let mut function_count = 0;

    // Section names
    // TODO: Move to a constant or config
    let section_names = [
        "Custom",    // 0
        "Type",      // 1
        "Import",    // 2
        "Function",  // 3
        "Table",     // 4
        "Memory",    // 5
        "Global",    // 6
        "Export",    // 7
        "Start",     // 8
        "Element",   // 9
        "Code",      // 10
        "Data",      // 11
        "DataCount", // 12
    ];

    while reader.position() < wasm_bytes.len() as u64 {
        if let Ok(section_id) = read_leb128_u32(&mut reader) {
            let section_size = read_leb128_u32(&mut reader).unwrap_or(0);
            let section_start = reader.position();

            let section_name = if section_id < section_names.len() as u32 {
                section_names[section_id as usize].to_string()
            } else {
                format!("Unknown ({section_id})")
            };

            sections.push(WasmSection {
                id: section_id,
                size: section_size as usize,
                name: section_name,
            });

            match section_id {
                3 => {
                    // Function section
                    if let Ok(count) = read_leb128_u32(&mut reader) {
                        function_count = count as usize;

                        // Skip function indices
                        reader.set_position(section_start + section_size as u64);
                    }
                }
                4 => {
                    // Table section
                    has_table_section = true;
                    reader.set_position(section_start + section_size as u64);
                }
                5 => {
                    // Memory section
                    has_memory_section = true;

                    if let Ok(memory_count) = read_leb128_u32(&mut reader) {
                        if memory_count > 0 {
                            if let Ok(flags) = reader.read_u8() {
                                if let Ok(initial) = read_leb128_u32(&mut reader) {
                                    let max = if flags & 0x01 != 0 {
                                        read_leb128_u32(&mut reader).ok()
                                    } else {
                                        None
                                    };

                                    memory_limits = Some((initial, max));
                                }
                            }
                        }
                    }

                    reader.set_position(section_start + section_size as u64);
                }
                7 => {
                    // Export section
                    has_export_section = true;

                    if let Ok(export_count) = read_leb128_u32(&mut reader) {
                        for _ in 0..export_count {
                            if let Ok(name_length) = read_leb128_u32(&mut reader) {
                                let mut name_buffer = vec![0u8; name_length as usize];
                                if reader.read_exact(&mut name_buffer).is_ok() {
                                    if let Ok(name) = String::from_utf8(name_buffer) {
                                        export_names.push(name);
                                    }
                                }

                                let _ = reader.read_u8();
                                let _ = read_leb128_u32(&mut reader);
                            }
                        }
                    }

                    reader.set_position(section_start + section_size as u64);
                }
                8 => {
                    // Start section
                    has_start_section = true;

                    if let Ok(index) = read_leb128_u32(&mut reader) {
                        start_function_index = Some(index);
                    }

                    reader.set_position(section_start + section_size as u64);
                }
                10 => {
                    // Code section
                    reader.set_position(section_start + section_size as u64);
                }
                _ => {
                    // Skip other sections
                    reader.set_position(section_start + section_size as u64);
                }
            }
        } else {
            break;
        }
    }

    Ok(VerificationResult {
        valid_magic,
        file_size: wasm_bytes.len(),
        section_count: sections.len(),
        sections,
        has_export_section,
        export_names,
        has_start_section,
        start_function_index,
        has_memory_section,
        memory_limits,
        has_table_section,
        function_count,
    })
}

/// Print verification results
pub fn print_verification_results(path: &str, results: &VerificationResult, detailed: bool) {
    let filename = Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🔍 \x1b[1;36mWASM Verification Results\x1b[0m\n");
    println!("  📄 \x1b[1;34mFile:\x1b[0m \x1b[1;33m{filename}\x1b[0m");

    let size_str = if results.file_size < 1024 {
        format!("{} bytes", results.file_size)
    } else if results.file_size < 1024 * 1024 {
        format!("{:.2} KB", results.file_size as f64 / 1024.0)
    } else {
        format!("{:.2} MB", results.file_size as f64 / (1024.0 * 1024.0))
    };

    println!("  💾 \x1b[1;34mSize:\x1b[0m \x1b[1;33m{size_str}\x1b[0m");

    if results.valid_magic {
        println!("  ✅ \x1b[1;32mValid WebAssembly format\x1b[0m");
    } else {
        println!("  ❌ \x1b[1;31mInvalid WebAssembly format\x1b[0m");
        println!("     \x1b[0;90mMissing magic bytes '\\0asm'\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m");
        return;
    }

    println!(
        "  📦 \x1b[1;34mSections:\x1b[0m \x1b[1;33m{}\x1b[0m",
        results.section_count
    );
    if detailed {
        println!("\n  📋 \x1b[1;34mSection Types:\x1b[0m");
        println!("     \x1b[1;36mType\x1b[0m     - Function signatures");
        println!("     \x1b[1;36mImport\x1b[0m   - External functions/memory/tables");
        println!("     \x1b[1;36mFunction\x1b[0m - Function type references");
        println!("     \x1b[1;36mTable\x1b[0m    - Indirect function tables");
        println!("     \x1b[1;36mMemory\x1b[0m   - Memory specifications");
        println!("     \x1b[1;36mGlobal\x1b[0m   - Global variables");
        println!("     \x1b[1;36mExport\x1b[0m   - Exported functions/memory/tables");
        println!("     \x1b[1;36mStart\x1b[0m    - Entry point function");
        println!("     \x1b[1;36mElement\x1b[0m  - Table element initializers");
        println!("     \x1b[1;36mCode\x1b[0m     - Function bodies");
        println!("     \x1b[1;36mData\x1b[0m     - Memory data initializers");
    }

    if results.has_start_section {
        println!("  🚀 \x1b[1;32mHas start section\x1b[0m");
    } else {
        println!("  ℹ️  \x1b[0;90mNo start section\x1b[0m");
    }

    if results.has_export_section {
        println!(
            "  🔄 \x1b[1;34mExports:\x1b[0m \x1b[1;33m{}\x1b[0m",
            results.export_names.len()
        );

        let mut found_entry = false;
        for name in &results.export_names {
            if is_entry_point(name) {
                println!("  ✅ \x1b[1;32mFound entry point: '{name}'\x1b[0m");
                found_entry = true;
            }
        }

        if !found_entry {
            println!("  ⚠️  \x1b[1;33mNo standard entry point found\x1b[0m");
        }

        if !results.export_names.is_empty() {
            println!(
                "\n  📋 \x1b[1;34mExported functions {}:\x1b[0m",
                if detailed { "" } else { "(sample)" }
            );

            let max_to_show = if detailed {
                results.export_names.len()
            } else {
                std::cmp::min(results.export_names.len(), 5)
            };

            for (i, name) in results.export_names.iter().take(max_to_show).enumerate() {
                println!("     \x1b[1;36m{}.\x1b[0m \x1b[1;37m{}\x1b[0m", i + 1, name);
            }

            if !detailed && results.export_names.len() > max_to_show {
                println!(
                    "     \x1b[0;90m... and {} more\x1b[0m",
                    results.export_names.len() - max_to_show
                );
                println!("     \x1b[0;90mUse --detailed flag to see all exports\x1b[0m");
            }
        }
    } else {
        println!("  ⚠️  \x1b[1;33mNo exports found\x1b[0m");
    }

    if results.has_memory_section {
        println!("  💾 \x1b[1;32mWebAssembly memory detected\x1b[0m");

        if let Some((initial, maximum)) = results.memory_limits {
            let initial_pages = initial;
            let initial_bytes = initial * 64 * 1024;

            println!("     \x1b[1;34mInitial size:\x1b[0m \x1b[1;33m{initial_pages} pages\x1b[0m (\x1b[1;33m{initial_bytes} bytes\x1b[0m)");

            if let Some(max) = maximum {
                let max_pages = max;
                let max_bytes = max * 64 * 1024;
                println!("     \x1b[1;34mMaximum size:\x1b[0m \x1b[1;33m{max_pages} pages\x1b[0m (\x1b[1;33m{max_bytes} bytes\x1b[0m)");
            } else {
                println!("     \x1b[1;34mMaximum size:\x1b[0m \x1b[1;33munlimited\x1b[0m");
            }
        }
    }

    if results.has_table_section {
        println!("  📊 \x1b[1;32mWebAssembly table detected\x1b[0m");
        if detailed {
            println!("     \x1b[0;90mThe table section contains function references\x1b[0m");
            println!("     \x1b[0;90mfor indirect function calls (call_indirect)\x1b[0m");
        }
    }

    if results.function_count > 0 {
        println!(
            "  🧩 \x1b[1;34mFunction count:\x1b[0m \x1b[1;33m{}\x1b[0m",
            results.function_count
        );
    }

    if detailed && !results.sections.is_empty() {
        println!("\n  📊 \x1b[1;34mSection Details:\x1b[0m");

        for section in &results.sections {
            let size_str = if section.size < 1024 {
                format!("{} bytes", section.size)
            } else {
                format!("{:.2} KB", section.size as f64 / 1024.0)
            };

            println!(
                "     \x1b[1;36m{:2}.\x1b[0m \x1b[1;37m{:10}\x1b[0m \x1b[0;90m({})\x1b[0m",
                section.id, section.name, size_str
            );
        }
    }

    println!("\n  📊 \x1b[1;34mWasmrun Conclusion:\x1b[0m");
    if results.has_export_section && !results.export_names.is_empty() {
        let has_entry = results.export_names.iter().any(|name| is_entry_point(name));

        if has_entry || results.has_start_section {
            println!("     \x1b[1;32m✓ WASM file should run with Wasmrun\x1b[0m");

            if results.has_start_section {
                if let Some(index) = results.start_function_index {
                    println!(
                        "       \x1b[0;90mModule has a start section with function index {index}\x1b[0m"
                    );
                } else {
                    println!("       \x1b[0;90mModule has a start section\x1b[0m");
                }
            }

            if has_entry {
                let entry_points: Vec<&String> = results
                    .export_names
                    .iter()
                    .filter(|name| is_entry_point(name))
                    .collect();

                println!(
                    "       \x1b[0;90mFound exported entry point{}: {}\x1b[0m",
                    if entry_points.len() > 1 { "s" } else { "" },
                    entry_points
                        .iter()
                        .map(|s| format!("'{s}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        } else {
            println!(
                "     \x1b[1;33m⚠️ WASM file is valid but may need custom initialization\x1b[0m"
            );
            println!("       \x1b[0;90mNo standard entry point found\x1b[0m");
        }
    } else if !results.has_export_section {
        println!("     \x1b[1;31m❌ WASM file has no exports and cannot be used\x1b[0m");
    } else {
        println!("     \x1b[1;33m⚠️ WASM file structure is unusual\x1b[0m");
    }

    if results.valid_magic && (results.has_export_section || results.has_start_section) {
        println!("\n  🚀 \x1b[1;33mRun with Wasmrun:\x1b[0m");
        println!("     \x1b[1;37mwasmrun --wasm --path {path}\x1b[0m");
    }

    println!("\x1b[1;34m╰\x1b[0m");
}

pub fn print_detailed_binary_info(path: &str) -> std::result::Result<(), String> {
    let wasm_bytes = fs::read(path).map_err(|e| format!("Error reading file: {e}"))?;

    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🔬 \x1b[1;36mDetailed WASM Binary Analysis\x1b[0m\n");
    println!(
        "  📄 \x1b[1;34mFile:\x1b[0m \x1b[1;33m{}\x1b[0m",
        Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    println!(
        "  💾 \x1b[1;34mSize:\x1b[0m \x1b[1;33m{} bytes\x1b[0m",
        wasm_bytes.len()
    );

    if wasm_bytes.len() < 8 {
        println!("  ❌ \x1b[1;31mFile too small to be a valid WASM module\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m");
        return Err("File too small".to_string());
    }

    let magic_bytes = &wasm_bytes[0..4];
    println!(
        "  🔑 \x1b[1;34mMagic bytes:\x1b[0m \x1b[1;33m{:02X} {:02X} {:02X} {:02X}\x1b[0m",
        magic_bytes[0], magic_bytes[1], magic_bytes[2], magic_bytes[3]
    );

    if magic_bytes == b"\0asm" {
        println!("  ✅ \x1b[1;32mValid WebAssembly magic bytes\x1b[0m");
    } else {
        println!("  ❌ \x1b[1;31mInvalid magic bytes, not a WASM file\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m");
        return Err("Invalid magic bytes".to_string());
    }

    let version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
    println!("  📊 \x1b[1;34mWASM version:\x1b[0m \x1b[1;33m{version}\x1b[0m");

    if version != 1 {
        println!("  ⚠️ \x1b[1;33mUnexpected WASM version (expected 1)\x1b[0m");
    }

    let mut _offset = 8;
    let _view = std::io::Cursor::new(&wasm_bytes);

    println!("\n  📋 \x1b[1;34mSection analysis:\x1b[0m");
    let mut section_count = 0;

    // Known section types
    // TODO: Move to a constant or config and read from there.
    let section_names = [
        "Custom",
        "Type",
        "Import",
        "Function",
        "Table",
        "Memory",
        "Global",
        "Export",
        "Start",
        "Element",
        "Code",
        "Data",
        "DataCount",
    ];

    let mut reader = Cursor::new(wasm_bytes.clone());
    reader.set_position(8);

    while (reader.position() as usize) < wasm_bytes.len() {
        section_count += 1;

        let section_start_offset = reader.position() as usize;

        if let Ok(section_id) = read_leb128_u32(&mut reader) {
            let section_size = read_leb128_u32(&mut reader).unwrap_or(0);

            let section_name = if section_id < section_names.len() as u32 {
                section_names[section_id as usize]
            } else {
                "Unknown"
            };

            let section_start = reader.position() as usize;
            let section_end = section_start + section_size as usize;

            println!("  \x1b[1;36m{:2}.\x1b[0m \x1b[1;37m{:10}\x1b[0m ID: {:2}, Size: {:6} bytes, Offset: 0x{:08X}-0x{:08X}",
                section_count, section_name, section_id, section_size, section_start_offset, section_end - 1);

            if section_start_offset <= 130 && section_end > 126 {
                println!("     \x1b[1;31m⚠️  WARNING: This section contains offset 128 where errors commonly occur!\x1b[0m");

                if section_id == 5 {
                    println!("     \x1b[1;33mChecking Memory section details:\x1b[0m");
                    let current_pos = reader.position();
                    if section_size >= 1 && (current_pos as usize) < wasm_bytes.len() {
                        if let Ok(num_memories) = read_leb128_u32(&mut reader) {
                            println!("     \x1b[1;33mNumber of memories: {num_memories}\x1b[0m");
                        }
                    }
                    reader.set_position(current_pos);
                }
            }

            // Skip. Revisit.
            reader.set_position((section_start + section_size as usize) as u64);
        } else {
            break;
        }
    }

    println!("\n  📊 \x1b[1;34mTotal sections found:\x1b[0m \x1b[1;33m{section_count}\x1b[0m");

    if section_count > 0 {
        println!("  ✅ \x1b[1;32mWASM file structure seems valid\x1b[0m");
    } else {
        println!("  ❌ \x1b[1;31mNo sections found in WASM file\x1b[0m");
    }

    // Analyze the module for issues
    println!();
    if let Ok(wasm_bytes) = fs::read(path) {
        if let Ok(module) = Module::parse(&wasm_bytes) {
            let issues = issue_detector::detect_issues(&module);
            issue_detector::display_issues(&issues);
        }
    }

    println!("\x1b[1;34m╰\x1b[0m");

    Ok(())
}

/// Check if a function name is a known entry point
pub fn is_entry_point(name: &str) -> bool {
    matches!(
        name,
        "main" | "_start" | "start" | "init" | "run" | "execute" | "_initialize"
    )
}

/// Read unsigned LEB128 encoded 32-bit value
fn read_leb128_u32(reader: &mut Cursor<Vec<u8>>) -> std::result::Result<u32, String> {
    let mut result = 0u32;
    let mut shift = 0;

    loop {
        let mut byte = [0u8; 1];
        if reader.read_exact(&mut byte).is_err() {
            return Err("Unexpected end of file".to_string());
        }

        result |= ((byte[0] & 0x7F) as u32) << shift;
        shift += 7;

        if byte[0] & 0x80 == 0 {
            break;
        }

        if shift >= 32 {
            return Err("Invalid LEB128 encoding".to_string());
        }
    }

    Ok(result)
}

/// Read a single byte as u8
trait CursorExt {
    fn read_u8(&mut self) -> std::result::Result<u8, String>;
}

impl CursorExt for Cursor<Vec<u8>> {
    fn read_u8(&mut self) -> std::result::Result<u8, String> {
        let mut byte = [0u8; 1];
        if self.read_exact(&mut byte).is_err() {
            return Err("Unexpected end of file".to_string());
        }
        Ok(byte[0])
    }
}

/// Resolve and validate WASM file path
fn resolve_and_validate_wasm_path(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<String> {
    let resolved_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());

    CommandValidator::validate_verify_args(path, positional_path)?;

    PathResolver::validate_wasm_file(&resolved_path)?;

    match PathResolver::get_file_size_human(&resolved_path) {
        Ok(size) => {
            if let Ok(metadata) = std::fs::metadata(&resolved_path) {
                let size_bytes = metadata.len();
                if size_bytes > 100 * 1024 * 1024 {
                    println!("⚠️  Warning: Large WASM file ({size}) - verification may take time");
                } else if size_bytes == 0 {
                    return Err(WasmrunError::Wasm(WasmError::validation_failed(
                        "WASM file is empty",
                    )));
                }
            }
        }
        Err(_) => {
            // Continue anyway, but note that we couldn't get size info
        }
    }

    Ok(resolved_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const VALID_WASM_BYTES: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
    const INVALID_WASM_BYTES: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];

    fn create_wasm_file(content: &[u8]) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(content).unwrap();
        temp_file
    }

    fn create_wasm_file_with_extension(content: &[u8]) -> tempfile::NamedTempFile {
        let mut temp_file = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
        temp_file.write_all(content).unwrap();
        temp_file
    }

    #[test]
    fn test_is_entry_point() {
        assert!(is_entry_point("main"));
        assert!(is_entry_point("_start"));
        assert!(is_entry_point("start"));
        assert!(is_entry_point("init"));
        assert!(is_entry_point("run"));
        assert!(is_entry_point("execute"));
        assert!(is_entry_point("_initialize"));
        assert!(!is_entry_point("other_function"));
        assert!(!is_entry_point(""));
    }

    #[test]
    fn test_verify_wasm_valid_magic() {
        let temp_file = create_wasm_file(&VALID_WASM_BYTES);
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(verification.valid_magic);
        assert_eq!(verification.file_size, 8);
    }

    #[test]
    fn test_verify_wasm_invalid_magic() {
        let temp_file = create_wasm_file(&INVALID_WASM_BYTES);
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(!verification.valid_magic);
        assert_eq!(verification.file_size, 8);
        assert_eq!(verification.section_count, 0);
    }

    #[test]
    fn test_verify_wasm_file_not_found() {
        let result = verify_wasm("/nonexistent/file.wasm");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("File not found"));
    }

    #[test]
    fn test_verify_wasm_file_too_small() {
        let temp_file = create_wasm_file(&[0x00, 0x61, 0x73]); // Only 3 bytes
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too small"));
    }

    #[test]
    fn test_verify_wasm_with_sections() {
        // Create a more complex WASM with type section
        let mut wasm_content = VALID_WASM_BYTES.to_vec();
        // Add a minimal type section (id=1, size=1, empty content)
        wasm_content.extend_from_slice(&[0x01, 0x01, 0x00]);

        let temp_file = create_wasm_file(&wasm_content);
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(verification.valid_magic);
        assert_eq!(verification.section_count, 1);
    }

    #[test]
    fn test_verify_wasm_with_export_section() {
        // Create WASM with export section containing "main" export
        let mut wasm_content = VALID_WASM_BYTES.to_vec();
        // Add export section (id=7) with "main" function export
        let export_section = [
            0x07, // Export section id
            0x08, // Section size
            0x01, // Export count = 1
            0x04, // Name length = 4
            b'm', b'a', b'i', b'n', // Name = "main"
            0x00, // Export kind = function
            0x00, // Function index = 0
        ];
        wasm_content.extend_from_slice(&export_section);

        let temp_file = create_wasm_file(&wasm_content);
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(verification.valid_magic);
        assert!(verification.has_export_section);
        assert_eq!(verification.export_names.len(), 1);
        assert_eq!(verification.export_names[0], "main");
    }

    #[test]
    fn test_verify_wasm_with_start_section() {
        // Create WASM with start section
        let mut wasm_content = VALID_WASM_BYTES.to_vec();
        // Add start section (id=8) with function index 0
        let start_section = [
            0x08, // Start section id
            0x01, // Section size
            0x00, // Function index = 0
        ];
        wasm_content.extend_from_slice(&start_section);

        let temp_file = create_wasm_file(&wasm_content);
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(verification.valid_magic);
        assert!(verification.has_start_section);
        assert_eq!(verification.start_function_index, Some(0));
    }

    #[test]
    fn test_verify_wasm_with_memory_section() {
        // Create WASM with memory section
        let mut wasm_content = VALID_WASM_BYTES.to_vec();
        // Add memory section (id=5) with initial size
        let memory_section = [
            0x05, // Memory section id
            0x03, // Section size
            0x01, // Memory count = 1
            0x00, // Flags (no maximum)
            0x01, // Initial size = 1 page
        ];
        wasm_content.extend_from_slice(&memory_section);

        let temp_file = create_wasm_file(&wasm_content);
        let result = verify_wasm(temp_file.path().to_str().unwrap());

        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(verification.valid_magic);
        assert!(verification.has_memory_section);
        assert_eq!(verification.memory_limits, Some((1, None)));
    }

    #[test]
    fn test_read_leb128_u32() {
        let data = vec![0x80, 0x01]; // 128 in LEB128 format
        let mut cursor = Cursor::new(data);
        let result = read_leb128_u32(&mut cursor);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 128);
    }

    #[test]
    fn test_read_leb128_u32_overflow() {
        // Create a LEB128 that would overflow u32
        let data = vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        let mut cursor = Cursor::new(data);
        let result = read_leb128_u32(&mut cursor);
        assert!(result.is_err());
    }

    #[test]
    fn test_cursor_read_u8() {
        let data = vec![0x42];
        let mut cursor = Cursor::new(data);
        let result = cursor.read_u8();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x42);
    }

    #[test]
    fn test_cursor_read_u8_eof() {
        let data = vec![];
        let mut cursor = Cursor::new(data);
        let result = cursor.read_u8();
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_and_validate_wasm_path() {
        let temp_file = create_wasm_file_with_extension(&VALID_WASM_BYTES);
        let path = temp_file.path().to_str().unwrap().to_string();

        let result = resolve_and_validate_wasm_path(&Some(path.clone()), &None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), path);
    }

    #[test]
    fn test_resolve_and_validate_wasm_path_positional() {
        let temp_file = create_wasm_file_with_extension(&VALID_WASM_BYTES);
        let path = temp_file.path().to_str().unwrap().to_string();

        let result = resolve_and_validate_wasm_path(&None, &Some(path.clone()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), path);
    }

    #[test]
    fn test_resolve_and_validate_wasm_path_empty_file() {
        let temp_file = create_wasm_file_with_extension(&[]);
        let path = temp_file.path().to_str().unwrap().to_string();

        let result = resolve_and_validate_wasm_path(&Some(path), &None);
        assert!(result.is_err());
        match result {
            Err(WasmrunError::Wasm(WasmError::ValidationFailed { reason })) => {
                assert!(reason.contains("empty"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }
}
