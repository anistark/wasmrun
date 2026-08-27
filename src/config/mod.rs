//! Configuration module for Wasmrun

pub mod constants;
pub mod plugin;
pub mod project;
pub mod server;

pub use constants::*;
pub use plugin::{ExternalPluginEntry, WasmrunConfig};
pub use project::ProjectConfig;
pub use server::{
    compile_project, run_server, setup_project_compilation, FileInfo, PortStatus, ServerConfig,
    ServerInfo,
};
