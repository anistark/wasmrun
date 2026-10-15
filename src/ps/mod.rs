//! [Process Manager] `wasmrun ps`: keep WASM programs running.
//!
//! A supervisor process runs the programs listed in a `processes.toml`, each
//! as its own `wasmrun exec`, restarts them by policy, and keeps a rotated log
//! per program. `wasmrun ps` commands find it through a state file and talk to
//! it over a loopback control API. It builds on exec mode and touches nothing
//! the other modes depend on.

pub mod config;
pub mod control;
pub mod logs;
pub mod supervisor;

use crate::error::{Result, WasmrunError};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Run a supervisor in this process until it is told to stop (Ctrl+C,
/// SIGTERM, or `wasmrun ps stop`), then stop every program it runs.
pub fn run_supervisor(config_path: &Path) -> Result<()> {
    let home = control::ps_home();
    if let Some(info) = control::running(&home) {
        return Err(WasmrunError::from(format!(
            "a supervisor is already running (pid {}); add to it with `wasmrun ps start --config FILE`",
            info.pid
        )));
    }
    let specs = config::load(config_path)?;

    let exe = std::env::current_exe()
        .map_err(|e| WasmrunError::from(format!("cannot find the wasmrun binary: {e}")))?;
    let launcher = supervisor::Launcher {
        exe,
        nodejs: Box::new(|| {
            crate::runtime::runtime_cache::RuntimeCache::new()
                .and_then(|cache| cache.runtime_path("nodejs"))
                .map_err(|e| e.to_string())
        }),
    };
    let sup = supervisor::Supervisor::new(home.join("logs"), launcher);
    let names = sup.add(specs).map_err(WasmrunError::from)?;

    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| WasmrunError::from(format!("cannot bind the control port: {e}")))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or_else(|| WasmrunError::from("control port has no address".to_string()))?;
    let token = control::new_token();
    let info = control::SupervisorInfo {
        pid: std::process::id(),
        port,
        token: token.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    control::write_state(&home, &info)
        .map_err(|e| WasmrunError::from(format!("cannot write the supervisor state: {e}")))?;

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::SeqCst));
    }
    println!(
        "wasmrun ps: supervising {} ({}), control on 127.0.0.1:{port}",
        names.len(),
        names.join(", ")
    );

    let load = |path: &Path| -> std::result::Result<Vec<String>, String> {
        let specs = config::load(path).map_err(|e| e.to_string())?;
        sup.add(specs)
    };
    control::serve(server, &token, &sup, &load, &stop);

    println!("wasmrun ps: stopping every program");
    sup.shutdown();
    let _ = std::fs::remove_file(control::state_file(&home));
    Ok(())
}
