//! Configuration module for Wasmrun

pub mod constants;
pub mod plugin;
pub mod project;

pub use constants::*;
pub use plugin::{ExternalPluginEntry, WasmrunConfig};
pub use project::ProjectConfig;
