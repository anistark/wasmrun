//! Plugin system for Wasmrun

use crate::compiler::builder::WasmBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod builtin;
pub mod bridge;
pub mod config;
pub mod external;
pub mod languages;
pub mod manager;
pub mod registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    CratesIo { name: String, version: String },
    Git { url: String, branch: Option<String> },
    Local { path: PathBuf },
}

pub trait Plugin: Send + Sync {
    fn info(&self) -> &PluginInfo;
    fn can_handle_project(&self, project_path: &str) -> bool;
    fn get_builder(&self) -> Box<dyn WasmBuilder>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub extensions: Vec<String>,
    pub entry_files: Vec<String>,
    pub plugin_type: PluginType,
    pub source: Option<PluginSource>,
    pub dependencies: Vec<String>,
    pub capabilities: PluginCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    Builtin,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapabilities {
    pub compile_wasm: bool,
    pub compile_webapp: bool,
    pub live_reload: bool,
    pub optimization: bool,
    pub custom_targets: Vec<String>,
}

impl Default for PluginCapabilities {
    fn default() -> Self {
        Self {
            compile_wasm: true,
            compile_webapp: false,
            live_reload: false,
            optimization: false,
            custom_targets: vec![],
        }
    }
}
