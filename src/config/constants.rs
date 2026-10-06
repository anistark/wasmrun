//! Constants used throughout Wasmrun

use std::sync::atomic::AtomicBool;

/// WASM file validation constants
/// The `\0asm` magic, and nothing else. The four bytes after it are a version
/// and a layer, and a binary that has the magic but a different layer is a
/// component, not a file with bad magic.
pub const WASM_MAGIC_BYTES: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// Agent mode's default port. Server mode's UI steps over it when its own port is taken, so a
/// second dev session never takes the port an agent server expects to start on
pub const AGENT_DEFAULT_PORT: u16 = 8430;

/// Debug flag for global debug state
pub static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
