//! Plugin system for Chakra - Built-in and External plugins

use crate::compiler::builder::WasmBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod config;
pub mod languages;
pub mod manager;
pub mod registry;
pub mod external;

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

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> crate::error::Result<Self> {
        let plugins: Vec<Box<dyn Plugin>> = vec![
            Box::new(languages::rust_plugin::RustPlugin::new()),
            Box::new(languages::c_plugin::CPlugin::new()),
            Box::new(languages::asc_plugin::AscPlugin::new()),
            Box::new(languages::python_plugin::PythonPlugin::new()),
        ];

        Ok(Self { plugins })
    }

    #[allow(dead_code)]
    pub fn get_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    #[allow(dead_code)]
    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.can_handle_project(project_path))
            .map(|boxed| boxed.as_ref())
    }

    pub fn get_plugin_by_name(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.info().name == name)
            .map(|boxed| boxed.as_ref())
    }

    #[allow(dead_code)]
    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.iter().map(|plugin| plugin.info()).collect()
    }

    pub fn get_plugin_info(&self, name: &str) -> Option<&PluginInfo> {
        self.get_plugin_by_name(name).map(|plugin| plugin.info())
    }

    #[allow(dead_code)]
    pub fn check_all_dependencies(&self) -> Vec<(String, Vec<String>)> {
        self.plugins
            .iter()
            .map(|plugin| {
                let builder = plugin.get_builder();
                (
                    plugin.info().name.clone(),
                    builder.check_dependencies(),
                )
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn verify_dependencies(&self, required_plugins: &[String]) -> crate::error::Result<()> {
        let available_plugins: Vec<String> = self
            .plugins
            .iter()
            .map(|p| p.info().name.clone())
            .collect();

        let missing_plugins: Vec<String> = required_plugins
            .iter()
            .filter(|&name| !available_plugins.contains(name))
            .cloned()
            .collect();

        if !missing_plugins.is_empty() {
            let missing_names = missing_plugins.join(", ");
            return Err(crate::error::ChakraError::from(format!(
                "Missing required plugins: {}",
                missing_names
            )));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_live_reload_plugins(&self) -> Vec<&dyn Plugin> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.info().capabilities.live_reload)
            .map(|boxed| boxed.as_ref())
            .collect()
    }

    #[allow(dead_code)]
    pub fn find_best_plugin_for_project(&self, project_path: &str) -> Option<(&dyn Plugin, f32)> {
        let mut best_plugin = None;
        let mut best_score = 0.0;

        for plugin in &self.plugins {
            if plugin.can_handle_project(project_path) {
                let score = self.calculate_plugin_score(plugin.as_ref(), project_path);
                if score > best_score {
                    best_score = score;
                    best_plugin = Some(plugin.as_ref());
                }
            }
        }

        best_plugin.map(|plugin| (plugin, best_score))
    }

    #[allow(dead_code)]
    fn calculate_plugin_score(&self, plugin: &dyn Plugin, project_path: &str) -> f32 {
        let info = plugin.info();
        let mut score = 1.0;

        for entry_file in &info.entry_files {
            let entry_path = std::path::Path::new(project_path).join(entry_file);
            if entry_path.exists() {
                score += 2.0;
            }
        }

        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if info.extensions.contains(&ext_str) {
                        score += 0.5;
                    }
                }
            }
        }

        score
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            plugins: Vec::new(),
        })
    }
}
