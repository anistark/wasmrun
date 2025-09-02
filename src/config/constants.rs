//! Constants used throughout Wasmrun

use std::sync::atomic::AtomicBool;

/// Server constants
pub const PID_FILE: &str = "/tmp/wasmrun_server.pid";

/// WASM file validation constants
pub const WASM_MAGIC_BYTES: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

/// Debug flag for global debug state
pub static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
