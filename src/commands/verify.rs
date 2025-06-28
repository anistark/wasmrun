use crate::cli::CommandValidator;
use crate::error::{Result, WasmError, WasmrunError};
use crate::utils::PathResolver;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

// Magic bytes for WebAssembly binary format
const WASM_MAGIC_BYTES: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

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

    println!("🔍 Verifying WebAssembly file: {}", wasm_path);

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

    Ok(())
}

/// Handle inspect command
pub fn handle_inspect_command(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<()> {
    let resolved_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
    println!("Resolved path: {:?}", resolved_path);

    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    PathResolver::validate_wasm_file(&wasm_path)?;

    println!("🔍 Inspecting WebAssembly file: {}", wasm_path);

    print_detailed_binary_info(&wasm_path)
        .map_err(|e| WasmrunError::Wasm(WasmError::validation_failed(e)))?;

    println!("Inspection completed successfully.");
    Ok(())
}

/// Verify a WebAssembly file
pub fn verify_wasm(path: &str) -> std::result::Result<VerificationResult, String> {
    if !Path::new(path).exists() {
        return Err(format!("File not found: {}", path));
    }

    let wasm_bytes = fs::read(path).map_err(|e| format!("Error reading file: {}", e))?;

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
                format!("Unknown ({})", section_id)
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
    println!("  📄 \x1b[1;34mFile:\x1b[0m \x1b[1;33m{}\x1b[0m", filename);

    let size_str = if results.file_size < 1024 {
        format!("{} bytes", results.file_size)
    } else if results.file_size < 1024 * 1024 {
        format!("{:.2} KB", results.file_size as f64 / 1024.0)
    } else {
        format!("{:.2} MB", results.file_size as f64 / (1024.0 * 1024.0))
    };

    println!("  💾 \x1b[1;34mSize:\x1b[0m \x1b[1;33m{}\x1b[0m", size_str);

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
                println!("  ✅ \x1b[1;32mFound entry point: '{}'\x1b[0m", name);
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

            println!("     \x1b[1;34mInitial size:\x1b[0m \x1b[1;33m{} pages\x1b[0m (\x1b[1;33m{} bytes\x1b[0m)", 
                initial_pages, initial_bytes);

            if let Some(max) = maximum {
                let max_pages = max;
                let max_bytes = max * 64 * 1024;
                println!("     \x1b[1;34mMaximum size:\x1b[0m \x1b[1;33m{} pages\x1b[0m (\x1b[1;33m{} bytes\x1b[0m)", 
                    max_pages, max_bytes);
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
                        "       \x1b[0;90mModule has a start section with function index {}\x1b[0m",
                        index
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
                        .map(|s| format!("'{}'", s))
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
        println!("     \x1b[1;37mwasmrun --wasm --path {}\x1b[0m", path);
    }

    println!("\x1b[1;34m╰\x1b[0m");
}

pub fn print_detailed_binary_info(path: &str) -> std::result::Result<(), String> {
    let wasm_bytes = fs::read(path).map_err(|e| format!("Error reading file: {}", e))?;

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
    println!(
        "  📊 \x1b[1;34mWASM version:\x1b[0m \x1b[1;33m{}\x1b[0m",
        version
    );

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
                            println!("     \x1b[1;33mNumber of memories: {}\x1b[0m", num_memories);
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

    println!(
        "\n  📊 \x1b[1;34mTotal sections found:\x1b[0m \x1b[1;33m{}\x1b[0m",
        section_count
    );

    if section_count > 0 {
        println!("  ✅ \x1b[1;32mWASM file structure seems valid\x1b[0m");
    } else {
        println!("  ❌ \x1b[1;31mNo sections found in WASM file\x1b[0m");
    }

    // Diagnostic advice.
    println!("\n  🔍 \x1b[1;34mPossible Issues:\x1b[0m");
    println!("     \x1b[0;37m• Sections in incorrect order (browsers require specific section ordering)\x1b[0m");
    println!("     \x1b[0;37m• Memory section with invalid configuration\x1b[0m");
    println!("     \x1b[0;37m• Binary compiled with incompatible flags or WASM features\x1b[0m");
    println!("     \x1b[0;37m• File corruption\x1b[0m");
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
                    println!(
                        "⚠️  Warning: Large WASM file ({}) - verification may take time",
                        size
                    );
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
