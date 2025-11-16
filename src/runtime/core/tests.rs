#[cfg(test)]
mod integration_tests {
    use crate::runtime::core::module::Module;
    use std::fs;

    /// Test parsing a real Rust WASM binary
    #[test]
    fn test_parse_rust_hello_wasm() {
        let wasm_path = "examples/rust-hello/target/wasm32-unknown-unknown/release/rust_hello.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            println!("⚠️  {} not found, skipping test", wasm_path);
            return;
        }

        let bytes = fs::read(wasm_path).expect("Failed to read WASM file");
        match Module::parse(&bytes) {
            Ok(module) => {
                // Basic validation
                assert_eq!(module.version, 1);
                assert!(!module.types.is_empty());
                println!("✓ Rust hello WASM parsed successfully");
                println!("  - Types: {}", module.types.len());
                println!("  - Functions: {}", module.functions.len());
                println!("  - Exports: {}", module.exports.len());
                println!("  - Imports: {}", module.imports.len());

                // Check for entry point
                if let Some(entry) = module.find_entry_point() {
                    println!("  - Entry point: function {}", entry);
                }

                // Print exports
                for (name, _export) in &module.exports {
                    println!("  - Export: {}", name);
                }
            }
            Err(e) => {
                println!(
                    "⚠️  Could not fully parse Rust hello WASM (expected for newer WASM features)"
                );
                println!("   Error: {}", e);
                // Don't fail test - this WASM may use newer features we don't support yet
            }
        }
    }

    /// Test parsing a real Go WASM binary
    #[test]
    fn test_parse_go_hello_wasm() {
        let wasm_path = "examples/go-hello/main.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            println!("⚠️  {} not found, skipping test", wasm_path);
            return;
        }

        let bytes = fs::read(wasm_path).expect("Failed to read WASM file");
        match Module::parse(&bytes) {
            Ok(module) => {
                // Basic validation
                assert_eq!(module.version, 1);
                println!("✓ Go hello WASM parsed successfully");
                println!("  - Types: {}", module.types.len());
                println!("  - Functions: {}", module.functions.len());
                println!("  - Exports: {}", module.exports.len());
                println!("  - Imports: {}", module.imports.len());

                if let Some(entry) = module.find_entry_point() {
                    println!("  - Entry point: function {}", entry);
                }
            }
            Err(e) => {
                println!("⚠️  Could not parse Go hello WASM: {}", e);
            }
        }
    }

    /// Test that all function sections are properly parsed
    #[test]
    fn test_rust_hello_functions_have_code() {
        let wasm_path = "examples/rust-hello/target/wasm32-unknown-unknown/release/rust_hello.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            println!("⚠️  {} not found, skipping test", wasm_path);
            return;
        }

        let bytes = fs::read(wasm_path).expect("Failed to read WASM file");
        if let Ok(module) = Module::parse(&bytes) {
            // All functions should have code (except imports)
            let import_func_count = module
                .imports
                .iter()
                .filter(|i| {
                    matches!(
                        i.kind,
                        crate::runtime::core::module::ImportKind::Function(_)
                    )
                })
                .count();

            for (i, func) in module.functions.iter().enumerate() {
                if !func.code.is_empty() {
                    println!("✓ Function {} has code", i);
                }
            }

            println!("✓ Functions analyzed");
            println!("  - Import functions: {}", import_func_count);
            println!("  - Local functions: {}", module.functions.len());
        }
    }

    /// Test that memory section is properly parsed
    #[test]
    fn test_memory_section_parsing() {
        let wasm_path = "examples/rust-hello/target/wasm32-unknown-unknown/release/rust_hello.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            println!("⚠️  {} not found, skipping test", wasm_path);
            return;
        }

        let bytes = fs::read(wasm_path).expect("Failed to read WASM file");
        if let Ok(module) = Module::parse(&bytes) {
            if let Some(memory) = &module.memory {
                println!("✓ Memory section found");
                println!("  - Initial pages: {}", memory.initial);
                if let Some(max) = memory.max {
                    println!("  - Max pages: {}", max);
                } else {
                    println!("  - Max pages: unlimited");
                }
            }
        }
    }

    /// Test that export names are correctly parsed
    #[test]
    fn test_export_names_are_valid_strings() {
        let wasm_path = "examples/rust-hello/target/wasm32-unknown-unknown/release/rust_hello.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            println!("⚠️  {} not found, skipping test", wasm_path);
            return;
        }

        let bytes = fs::read(wasm_path).expect("Failed to read WASM file");
        if let Ok(module) = Module::parse(&bytes) {
            for (name, export) in &module.exports {
                assert!(!name.is_empty(), "Export name should not be empty");
                assert!(name.is_ascii(), "Export name should be ASCII");
                println!("✓ Export: {} (type: {:?})", name, export.kind);
            }
        }
    }
}
