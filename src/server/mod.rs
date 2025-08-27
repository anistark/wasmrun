use crate::error::{Result, ServerError, WasmrunError};
use std::path::Path;

mod config;
mod handler;
mod utils;
mod wasm;

pub use config::{run_server, ServerConfig};
pub use utils::{ServerInfo, ServerUtils};

const PID_FILE: &str = "/tmp/wasmrun_server.pid";

pub fn run_wasm_file(path: &str, port: u16) -> Result<()> {
    if cfg!(test) {
        return Ok(());
    }

    let path_obj = std::path::Path::new(path);

    if !path_obj.extension().is_some_and(|ext| ext == "wasm") {
        if path_obj.extension().is_some_and(|ext| ext == "js") {
            return handle_js_file(path, port);
        }

        return Err(WasmrunError::Server(ServerError::RequestHandlingFailed {
            reason: format!("Not a WASM file: {path}"),
        }));
    }

    let final_port = ServerUtils::handle_port_conflict(port)?;

    if handle_wasm_bindgen_file(path_obj, path, final_port)? {
        return Ok(());
    }

    let server_info = ServerInfo::for_wasm_file(path, final_port, false)?;
    server_info.print_server_startup();

    let wasm_filename = Path::new(path)
        .file_name()
        .ok_or_else(|| WasmrunError::path(format!("Invalid path: {path}")))?
        .to_string_lossy()
        .to_string();

    wasm::serve_wasm_file(path, final_port, &wasm_filename).map_err(|e| {
        WasmrunError::Server(ServerError::startup_failed(
            final_port,
            format!("Server error: {e}"),
        ))
    })
}

pub fn run_project(
    path: &str,
    port: u16,
    language_override: Option<String>,
    watch: bool,
) -> Result<()> {
    if cfg!(test) {
        return Ok(());
    }

    let path_obj = std::path::Path::new(path);

    if path_obj.is_file() && path_obj.extension().is_some_and(|ext| ext == "wasm") {
        println!("\n\x1b[1;34m╭\x1b[0m");
        println!("  ℹ️  \x1b[1;34mDetected WASM file: {path}\x1b[0m");
        println!("  \x1b[0;37mRunning the WASM file directly...\x1b[0m");
        println!("\x1b[1;34m╰\x1b[0m\n");

        return run_wasm_file(path, port);
    }

    if path_obj.is_file() && path_obj.extension().is_some_and(|ext| ext == "js") {
        return handle_js_file(path, port);
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

    let (lang, temp_output_dir) = config::setup_project_compilation(path, language_override, watch)
        .ok_or_else(|| {
            WasmrunError::language_detection(format!(
                "Failed to setup compilation for project: {path}"
            ))
        })?;

    let result = config::compile_project(path, &temp_output_dir, lang, watch).ok_or_else(|| {
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
        project_path: if watch { Some(path.to_string()) } else { None },
        output_dir: if watch {
            Some(temp_output_dir.to_string())
        } else {
            None
        },
    };

    if is_wasm_bindgen {
        println!("🔧 Running wasm-bindgen project with JavaScript support");
    } else {
        println!("⚡ Running standard WASM project");
    }

    config::run_server(server_config).map_err(|e| {
        WasmrunError::Server(ServerError::startup_failed(
            final_port,
            format!("Project server error: {e}"),
        ))
    })
}

pub fn is_server_running() -> bool {
    if !std::path::Path::new(PID_FILE).exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            let ps_command = std::process::Command::new("ps")
                .arg("-p")
                .arg(pid.to_string())
                .output();

            if let Ok(output) = ps_command {
                return output.status.success()
                    && String::from_utf8_lossy(&output.stdout).lines().count() > 1;
            }
        }
    }

    false
}

pub fn stop_existing_server() -> Result<()> {
    if !is_server_running() {
        if std::path::Path::new(PID_FILE).exists() {
            std::fs::remove_file(PID_FILE).map_err(|e| {
                WasmrunError::Server(ServerError::StopFailed {
                    pid: 0,
                    reason: format!("Failed to remove stale PID file: {e}"),
                })
            })?;
        }
        return Err(WasmrunError::Server(ServerError::NotRunning));
    }

    let pid_str = std::fs::read_to_string(PID_FILE).map_err(|e| {
        WasmrunError::Server(ServerError::StopFailed {
            pid: 0,
            reason: format!("Failed to read PID file: {e}"),
        })
    })?;

    let pid = pid_str.trim().parse::<u32>().map_err(|e| {
        WasmrunError::Server(ServerError::StopFailed {
            pid: 0,
            reason: format!("Failed to parse PID '{}': {}", pid_str.trim(), e),
        })
    })?;

    let kill_command = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| {
            WasmrunError::Server(ServerError::StopFailed {
                pid,
                reason: format!("Failed to kill server process: {e}"),
            })
        })?;

    if kill_command.status.success() {
        std::fs::remove_file(PID_FILE).map_err(|e| {
            WasmrunError::Server(ServerError::StopFailed {
                pid,
                reason: format!("Failed to remove PID file: {e}"),
            })
        })?;
        println!("💀 Existing Wasmrun server terminated successfully.");
        Ok(())
    } else {
        let error_msg = String::from_utf8_lossy(&kill_command.stderr);
        Err(WasmrunError::Server(ServerError::StopFailed {
            pid,
            reason: error_msg.to_string(),
        }))
    }
}

fn handle_js_file(path: &str, port: u16) -> Result<()> {
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
                )
                .map_err(|e| {
                    WasmrunError::Server(ServerError::startup_failed(
                        port,
                        format!("wasm-bindgen error: {e}"),
                    ))
                });
            }
        }

        return run_wasm_file(wasm_path.to_str().unwrap(), port);
    }

    Err(WasmrunError::file_not_found(format!(
        "Corresponding WASM file not found for JS file: {path}"
    )))
}

fn handle_wasm_bindgen_file(path_obj: &std::path::Path, path: &str, port: u16) -> Result<bool> {
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
            )
            .map_err(|e| {
                WasmrunError::Server(ServerError::startup_failed(
                    port,
                    format!("bindgen error: {e}"),
                ))
            })?;

            return Ok(true);
        } else {
            return search_for_js_files(path_obj, path, port, file_name.as_ref());
        }
    }

    Ok(false)
}

fn search_for_js_files(
    path_obj: &std::path::Path,
    path: &str,
    port: u16,
    _wasm_filename: &str,
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
                            config::run_server(ServerConfig {
                                wasm_path: path.to_string(),
                                js_path: Some(entry_path.to_str().unwrap().to_string()),
                                port,
                                watch_mode: false,
                                project_path: None,
                                output_dir: None,
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
