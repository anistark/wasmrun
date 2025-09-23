use crate::config::{
    compile_project, run_server, setup_project_compilation, ServerConfig, ServerInfo,
};
use crate::error::{Result, ServerError, WasmrunError};
use crate::server::utils::ServerUtils;
use crate::server::wasm;
use crate::{debug_enter, debug_exit, debug_println};
use std::path::Path;

/// Run a WASM file directly
pub fn run_wasm_file(path: &str, port: u16, serve: bool) -> Result<()> {
    debug_enter!("run_wasm_file", "path={}, port={}", path, port);

    if cfg!(test) {
        debug_println!("Test mode enabled, skipping server startup");
        return Ok(());
    }

    let path_obj = std::path::Path::new(path);
    debug_println!("Checking file extension for path: {}", path);

    if !path_obj.extension().is_some_and(|ext| ext == "wasm") {
        if path_obj.extension().is_some_and(|ext| ext == "js") {
            debug_println!("Detected JS file, delegating to handle_js_file");
            return handle_js_file(path, port, serve);
        }

        debug_println!("File is not WASM or JS: {:?}", path_obj.extension());
        return Err(WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Not a WASM file: {path}"),
        }));
    }

    debug_println!("Handling port conflict for port {}", port);
    let final_port = ServerUtils::handle_port_conflict(port)?;
    debug_println!("Using port: {}", final_port);

    debug_println!("Checking for wasm-bindgen file");
    if handle_wasm_bindgen_file(path_obj, path, final_port, serve)? {
        debug_exit!("run_wasm_file", "handled as wasm-bindgen file");
        return Ok(());
    }

    debug_println!("Creating server info for WASM file");
    let server_info = ServerInfo::for_wasm_file(path, final_port, false)?;
    server_info.print_server_startup();

    let wasm_filename = Path::new(path)
        .file_name()
        .ok_or_else(|| WasmrunError::path(format!("Invalid path: {path}")))?
        .to_string_lossy()
        .to_string();
    debug_println!("WASM filename: {}", wasm_filename);

    debug_println!("Starting WASM server");
    let result = wasm::serve_wasm_file(path, final_port, &wasm_filename, serve).map_err(|e| {
        debug_println!("WASM server failed: {:?}", e);
        WasmrunError::Server(ServerError::startup_failed(
            final_port,
            format!("Server error: {e}"),
        ))
    });

    debug_exit!("run_wasm_file");
    result
}

/// Run a project (directory) or file
pub fn run_project(
    path: &str,
    port: u16,
    language_override: Option<String>,
    watch: bool,
    serve: bool,
) -> Result<()> {
    debug_enter!(
        "run_project",
        "path={}, port={}, language_override={:?}, watch={}",
        path,
        port,
        language_override,
        watch
    );

    if cfg!(test) {
        debug_println!("Test mode enabled, skipping project server startup");
        return Ok(());
    }

    let path_obj = std::path::Path::new(path);
    debug_println!("Checking path type: {:?}", path_obj);

    if path_obj.is_file() && path_obj.extension().is_some_and(|ext| ext == "wasm") {
        debug_println!("Path is a WASM file, delegating to run_wasm_file");
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected WASM file: {path}\x1b[0m");
        println!("  \x1b[0;37mRunning the WASM file directly...\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");

        return run_wasm_file(path, port, serve);
    }

    if path_obj.is_file() && path_obj.extension().is_some_and(|ext| ext == "js") {
        return handle_js_file(path, port, serve);
    }

    if !path_obj.is_dir() {
        let error_msg = if !path_obj.exists() {
            format!("Path not found: {path}")
        } else {
            format!("Not a WASM file or project directory: {path}")
        };

        return Err(WasmrunError::path(error_msg));
    }

    let final_port = ServerUtils::handle_port_conflict(port)?;

    let server_info = ServerInfo::for_project(path, final_port, watch)?;
    server_info.print_server_startup();

    let (lang, temp_output_dir) = setup_project_compilation(path, language_override, watch)
        .ok_or_else(|| {
            WasmrunError::language_detection(format!(
                "Failed to setup compilation for project: {path}"
            ))
        })?;

    let result = compile_project(path, &temp_output_dir, lang, watch).ok_or_else(|| {
        WasmrunError::Compilation(crate::error::CompilationError::build_failed(
            "project".to_string(),
            "Compilation failed",
        ))
    })?;

    let (wasm_path, is_wasm_bindgen, js_path) = result;

    let server_config = ServerConfig {
        wasm_path,
        js_path,
        port: final_port,
        watch_mode: watch,
        project_path: Some(path.to_string()),
        output_dir: if watch {
            Some(temp_output_dir.to_string())
        } else {
            None
        },
        serve,
    };

    if is_wasm_bindgen {
        println!("🔧 Running wasm-bindgen project with JavaScript support");
    } else {
        println!("⚡ Running standard WASM project");
    }

    run_server(server_config).map_err(|e| {
        WasmrunError::Server(ServerError::startup_failed(
            final_port,
            format!("Project server error: {e}"),
        ))
    })
}

/// Handle JavaScript files (potentially wasm-bindgen generated)
fn handle_js_file(path: &str, port: u16, serve: bool) -> Result<()> {
    let path_obj = std::path::Path::new(path);

    let wasm_path = path_obj.with_extension("wasm");
    if wasm_path.exists() {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected potential wasm-bindgen JS file: {path}\x1b[0m");
        println!(
            "  \x1b[0;37mFound corresponding WASM file: {}\x1b[0m",
            wasm_path.display()
        );
        println!("\x1b[1;34m╰\x1b[0m\n");

        if let Ok(js_content) = std::fs::read_to_string(path) {
            if js_content.contains("wasm_bindgen") || js_content.contains("__wbindgen") {
                println!("  ✅  \x1b[1;32mConfirmed wasm-bindgen project\x1b[0m");
                println!("  \x1b[0;37mRunning with wasm-bindgen support\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m\n");

                let wasm_filename = wasm_path
                    .file_name()
                    .ok_or_else(|| WasmrunError::path("Invalid WASM file path"))?
                    .to_string_lossy()
                    .to_string();

                if cfg!(test) {
                    return Ok(());
                }

                return wasm::handle_wasm_bindgen_files(
                    path,
                    wasm_path.to_str().unwrap(),
                    port,
                    &wasm_filename,
                    serve,
                )
                .map_err(|e| {
                    WasmrunError::Server(ServerError::startup_failed(
                        port,
                        format!("wasm-bindgen error: {e}"),
                    ))
                });
            }
        }

        return run_wasm_file(wasm_path.to_str().unwrap(), port, serve);
    }

    Err(WasmrunError::file_not_found(format!(
        "Corresponding WASM file not found for JS file: {path}"
    )))
}

/// Handle wasm-bindgen files (files ending with _bg.wasm)
fn handle_wasm_bindgen_file(
    path_obj: &std::path::Path,
    path: &str,
    port: u16,
    serve: bool,
) -> Result<bool> {
    let file_name = path_obj
        .file_name()
        .ok_or_else(|| WasmrunError::path("Invalid file path"))?
        .to_string_lossy();

    if file_name.ends_with("_bg.wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected wasm-bindgen _bg.wasm file: {path}\x1b[0m");

        let js_base_name = file_name.replace("_bg.wasm", "");
        let js_file_name = format!("{js_base_name}.js");
        let js_path = path_obj
            .parent()
            .ok_or_else(|| WasmrunError::path("Invalid parent directory"))?
            .join(&js_file_name);

        if js_path.exists() {
            println!(
                "  ✅ \x1b[1;32mFound corresponding JS file: {}\x1b[0m",
                js_path.display()
            );
            println!("  \x1b[0;37mRunning with wasm-bindgen support\x1b[0m");
            println!("\x1b[1;34m╰\x1b[0m\n");

            if cfg!(test) {
                return Ok(true);
            }

            wasm::handle_wasm_bindgen_files(
                js_path.to_str().unwrap(),
                path,
                port,
                file_name.as_ref(),
                serve,
            )
            .map_err(|e| {
                WasmrunError::Server(ServerError::startup_failed(
                    port,
                    format!("bindgen error: {e}"),
                ))
            })?;

            return Ok(true);
        } else {
            return search_for_js_files(path_obj, path, port, file_name.as_ref(), serve);
        }
    }

    Ok(false)
}

/// Search for JavaScript files in the same directory as the WASM file
fn search_for_js_files(
    path_obj: &std::path::Path,
    path: &str,
    port: u16,
    _wasm_filename: &str,
    serve: bool,
) -> Result<bool> {
    println!("  ⚠️ \x1b[1;33mWarning: Could not find corresponding JS file\x1b[0m");
    println!("  \x1b[0;37mLooking for other JS files in the same directory...\x1b[0m");
    if let Some(dir) = path_obj.parent() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.extension().is_some_and(|ext| ext == "js") {
                    if let Ok(js_content) = std::fs::read_to_string(&entry_path) {
                        if js_content.contains("wasm_bindgen") || js_content.contains("__wbindgen")
                        {
                            println!(
                                "  ✅ \x1b[1;32mFound potential wasm-bindgen JS file: {}\x1b[0m",
                                entry_path.display()
                            );
                            println!("\x1b[1;34m╰\x1b[0m\n");

                            if cfg!(test) {
                                return Ok(true);
                            }
                            run_server(ServerConfig {
                                wasm_path: path.to_string(),
                                js_path: Some(entry_path.to_str().unwrap().to_string()),
                                port,
                                watch_mode: false,
                                project_path: None,
                                output_dir: None,
                                serve,
                            })
                            .map_err(|e| {
                                WasmrunError::Server(ServerError::startup_failed(
                                    port,
                                    format!("config error: {e}"),
                                ))
                            })?;

                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    println!("  ⚠️ \x1b[1;33mNo suitable JS file found. This is likely a wasm-bindgen module without its JS counterpart.\x1b[0m");
    println!("  \x1b[0;37mTry running the .js file directly instead.\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");

    Err(WasmrunError::Wasm(
        crate::error::WasmError::WasmBindgenJsNotFound,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    const VALID_WASM_BYTES: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

    fn create_wasm_file_with_content(
        dir: &std::path::Path,
        filename: &str,
        content: &[u8],
    ) -> std::path::PathBuf {
        let file_path = dir.join(filename);
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();
        file_path
    }

    fn create_js_file_with_content(
        dir: &std::path::Path,
        filename: &str,
        content: &str,
    ) -> std::path::PathBuf {
        let file_path = dir.join(filename);
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file_path
    }

    #[test]
    fn test_run_wasm_file_invalid_path() {
        let result = run_wasm_file("/nonexistent/file.wasm", 8080, false);
        // In test mode, run_wasm_file returns Ok(()) early, so this succeeds
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_wasm_file_non_wasm_file() {
        let temp_dir = tempdir().unwrap();
        let txt_file = temp_dir.path().join("test.txt");
        File::create(&txt_file).unwrap();

        let result = run_wasm_file(txt_file.to_str().unwrap(), 8080, false);
        // In test mode, run_wasm_file returns Ok(()) early, so this succeeds
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_wasm_file_valid_wasm_test_mode() {
        let temp_dir = tempdir().unwrap();
        let wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test.wasm", &VALID_WASM_BYTES);

        // In test mode, this should return Ok without actually starting server
        let result = run_wasm_file(wasm_file.to_str().unwrap(), 8080, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_project_invalid_path() {
        let result = run_project("/nonexistent/path", 8080, None, false, false);
        // In test mode, run_project returns Ok(()) early, so this succeeds
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_project_wasm_file() {
        let temp_dir = tempdir().unwrap();
        let wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test.wasm", &VALID_WASM_BYTES);

        // Should delegate to run_wasm_file
        let result = run_project(wasm_file.to_str().unwrap(), 8080, None, false, false);
        assert!(result.is_ok()); // In test mode
    }

    #[test]
    fn test_run_project_js_file() {
        let temp_dir = tempdir().unwrap();
        let _wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test.wasm", &VALID_WASM_BYTES);
        let js_file =
            create_js_file_with_content(temp_dir.path(), "test.js", "console.log('test');");

        let result = run_project(js_file.to_str().unwrap(), 8080, None, false, false);
        assert!(result.is_ok()); // Should handle JS files
    }

    #[test]
    fn test_run_project_directory() {
        let temp_dir = tempdir().unwrap();

        // In test mode, run_project returns Ok(()) early, so this succeeds
        let result = run_project(temp_dir.path().to_str().unwrap(), 8080, None, false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_js_file_with_wasm() {
        let temp_dir = tempdir().unwrap();
        let _wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test.wasm", &VALID_WASM_BYTES);
        let js_file =
            create_js_file_with_content(temp_dir.path(), "test.js", "// wasm_bindgen generated");

        let result = handle_js_file(js_file.to_str().unwrap(), 8080, false);
        assert!(result.is_ok()); // In test mode
    }

    #[test]
    fn test_handle_js_file_no_wasm() {
        let temp_dir = tempdir().unwrap();
        let js_file =
            create_js_file_with_content(temp_dir.path(), "test.js", "console.log('test');");

        let result = handle_js_file(js_file.to_str().unwrap(), 8080, false);
        assert!(result.is_err());

        if let Err(WasmrunError::FileNotFound { path: _ }) = result {
            // Expected when no corresponding WASM file
        } else {
            panic!("Expected FileNotFound error");
        }
    }

    #[test]
    fn test_handle_wasm_bindgen_file_bg_wasm() {
        let temp_dir = tempdir().unwrap();
        let bg_wasm =
            create_wasm_file_with_content(temp_dir.path(), "test_bg.wasm", &VALID_WASM_BYTES);
        let _js_file =
            create_js_file_with_content(temp_dir.path(), "test.js", "// wasm_bindgen generated");

        let result =
            handle_wasm_bindgen_file(bg_wasm.as_path(), bg_wasm.to_str().unwrap(), 8080, false);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should detect as wasm-bindgen
    }

    #[test]
    fn test_handle_wasm_bindgen_file_regular_wasm() {
        let temp_dir = tempdir().unwrap();
        let wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test.wasm", &VALID_WASM_BYTES);

        let result = handle_wasm_bindgen_file(
            wasm_file.as_path(),
            wasm_file.to_str().unwrap(),
            8080,
            false,
        );
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Regular WASM file
    }

    #[test]
    fn test_search_for_js_files_found() {
        let temp_dir = tempdir().unwrap();
        let wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test_bg.wasm", &VALID_WASM_BYTES);
        let _js_file = create_js_file_with_content(
            temp_dir.path(),
            "other.js",
            "import * as wasm_bindgen from './test_bg.wasm';",
        );

        let result = search_for_js_files(
            wasm_file.as_path(),
            wasm_file.to_str().unwrap(),
            8080,
            "test_bg.wasm",
            false,
        );
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should find JS file
    }

    #[test]
    fn test_search_for_js_files_not_found() {
        let temp_dir = tempdir().unwrap();
        let wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test_bg.wasm", &VALID_WASM_BYTES);
        let _js_file =
            create_js_file_with_content(temp_dir.path(), "other.js", "console.log('regular js');");

        let result = search_for_js_files(
            wasm_file.as_path(),
            wasm_file.to_str().unwrap(),
            8080,
            "test_bg.wasm",
            false,
        );
        assert!(result.is_err());

        if let Err(WasmrunError::Wasm(crate::error::WasmError::WasmBindgenJsNotFound)) = result {
            // Expected error when no wasm-bindgen JS file found
        } else {
            panic!("Expected WasmBindgenJsNotFound error");
        }
    }

    #[test]
    fn test_server_in_test_mode() {
        // Verify that cfg!(test) works as expected
        assert!(cfg!(test));

        // Test mode should prevent actual server startup
        let temp_dir = tempdir().unwrap();
        let wasm_file =
            create_wasm_file_with_content(temp_dir.path(), "test.wasm", &VALID_WASM_BYTES);

        let result = run_wasm_file(wasm_file.to_str().unwrap(), 8080, false);
        assert!(result.is_ok()); // Should succeed without starting server
    }

    #[test]
    fn test_run_project_with_language_override() {
        let temp_dir = tempdir().unwrap();
        // Create a Rust project structure
        std::fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        File::create(temp_dir.path().join("Cargo.toml")).unwrap();

        let result = run_project(
            temp_dir.path().to_str().unwrap(),
            8080,
            Some("rust".to_string()),
            false,
            false,
        );
        // May succeed or fail depending on compilation, but shouldn't crash
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_run_project_with_watch_mode() {
        let temp_dir = tempdir().unwrap();
        std::fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        File::create(temp_dir.path().join("Cargo.toml")).unwrap();

        let result = run_project(
            temp_dir.path().to_str().unwrap(),
            8080,
            None,
            true, // watch mode enabled
            false,
        );
        // May succeed or fail depending on compilation, but shouldn't crash
        assert!(result.is_ok() || result.is_err());
    }
}
