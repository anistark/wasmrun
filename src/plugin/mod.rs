//! Plugin system for Wasmrun - Built-in and External plugins

use crate::compiler::builder::WasmBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod builtin;
pub mod config;
pub mod external;
pub mod languages;
pub mod manager;
pub mod protocol;
pub mod registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    CratesIo { name: String, version: String },
    Git { url: String, branch: Option<String> },
    Local { path: PathBuf },
}

pub trait Plugin: Send + Sync {
    fn info(&self) -> &PluginInfo;
    #[allow(dead_code)]
    fn can_handle_project(&self, project_path: &str) -> bool;
    #[allow(dead_code)]
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

/// Plugin manager for managing all plugins
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> crate::error::Result<Self> {
        use crate::plugin::builtin;
        use crate::plugin::config::WasmrunConfig;
        use crate::plugin::external::ExternalPluginLoader;

        let mut plugins: Vec<Box<dyn Plugin>> = vec![];

        // Load built-in plugins
        let builtin_plugins = builtin::get_builtin_plugins();
        for plugin in builtin_plugins {
            plugins.push(plugin);
        }

        // Load external plugins
        let config = WasmrunConfig::load()?;
        for entry in config.external_plugins.values() {
            if entry.enabled {
                match ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        plugins.push(plugin);
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️  Failed to load external plugin '{}': {}",
                            entry.info.name, e
                        );
                    }
                }
            }
        }

        Ok(Self { plugins })
    }

    #[allow(dead_code)]
    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        for plugin in &self.plugins {
            if plugin.can_handle_project(project_path) {
                return Some(plugin.as_ref());
            }
        }
        None
    }

    pub fn get_plugin_by_name(&self, name: &str) -> Option<&dyn Plugin> {
        for plugin in &self.plugins {
            if plugin.info().name == name {
                return Some(plugin.as_ref());
            }
        }
        None
    }

    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.iter().map(|p| p.info()).collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            plugins: Vec::new(),
        })
    }
}
