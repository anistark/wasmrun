use crate::compiler::builder::WasmBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod config;
pub mod languages;
pub mod manager;
pub mod registry;
// TODO: Dynamic plugin loading - will be implemented when we add support
// for loading plugins from external .so/.dll/.dylib files at runtime
// Currently plugins are statically compiled, but this infrastructure
// is kept for future dynamic loading capability
pub mod external;

/// Plugin source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    /// from crates.io
    CratesIo { name: String, version: String },
    /// from Git repository
    Git { url: String, branch: Option<String> },
    /// from local directory
    Local { path: PathBuf },
}

/// Core trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Get plugin information
    fn info(&self) -> &PluginInfo;

    /// Check if this plugin can handle the given project
    #[allow(dead_code)]
    fn can_handle_project(&self, project_path: &str) -> bool;

    /// Get a builder instance for compilation. TODO: Remove this in future.
    #[allow(dead_code)]
    fn get_builder(&self) -> Box<dyn WasmBuilder>;
}

/// Plugin metadata and capabilities
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

/// Type of plugin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    Builtin,
    External,
    Registry,
}

/// Plugin capabilities
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

/// Plugin manager for handling all available plugins
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    /// Create a new plugin manager with default builtin plugins
    pub fn new() -> crate::error::Result<Self> {
        // Register all builtin plugins
        let plugins: Vec<Box<dyn Plugin>> = vec![
            Box::new(languages::rust_plugin::RustPlugin::new()),
            Box::new(languages::go_plugin::GoPlugin::new()),
            Box::new(languages::c_plugin::CPlugin::new()),
            Box::new(languages::asc_plugin::AscPlugin::new()),
            Box::new(languages::python_plugin::PythonPlugin::new()),
        ];

        Ok(Self { plugins })
    }

    /// Get all available plugins
    #[allow(dead_code)]
    pub fn get_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    // TODO: Remove this in future
    #[allow(dead_code)]
    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.can_handle_project(project_path))
            .map(|boxed| boxed.as_ref())
    }

    /// Get a plugin by name
    pub fn get_plugin_by_name(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.info().name == name)
            .map(|boxed| boxed.as_ref())
    }

    /// Add an external plugin
    #[allow(dead_code)]
    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// List all plugin names and descriptions
    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.iter().map(|plugin| plugin.info()).collect()
    }

    /// Get detailed information about a specific plugin
    pub fn get_plugin_info(&self, name: &str) -> Option<&PluginInfo> {
        self.get_plugin_by_name(name).map(|plugin| plugin.info())
    }

    /// Check dependencies for all plugins
    #[allow(dead_code)]
    pub fn check_all_dependencies(&self) -> Vec<(String, Vec<String>)> {
        self.plugins
            .iter()
            .map(|plugin| {
                let info = plugin.info();
                let builder = plugin.get_builder();
                let missing = builder.check_dependencies();
                (info.name.clone(), missing)
            })
            .filter(|(_, missing)| !missing.is_empty())
            .collect()
    }

    /// Check dependencies for a specific plugin
    #[allow(dead_code)]
    pub fn check_plugin_dependencies(&self, plugin_name: &str) -> Option<Vec<String>> {
        self.get_plugin_by_name(plugin_name)
            .map(|plugin| plugin.get_builder().check_dependencies())
    }

    /// Get all plugins that support web applications
    #[allow(dead_code)]
    pub fn get_webapp_plugins(&self) -> Vec<&dyn Plugin> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.info().capabilities.compile_webapp)
            .map(|boxed| boxed.as_ref())
            .collect()
    }

    /// Get all plugins that support optimization
    #[allow(dead_code)]
    pub fn get_optimization_plugins(&self) -> Vec<&dyn Plugin> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.info().capabilities.optimization)
            .map(|boxed| boxed.as_ref())
            .collect()
    }

    /// Get plugins that can handle a specific file extension
    #[allow(dead_code)]
    pub fn get_plugins_for_extension(&self, extension: &str) -> Vec<&dyn Plugin> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.info().extensions.iter().any(|ext| ext == extension))
            .map(|boxed| boxed.as_ref())
            .collect()
    }

    /// Get the number of available plugins
    #[allow(dead_code)]
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get plugin stats
    #[allow(dead_code)]
    pub fn get_plugin_stats(&self) -> PluginStats {
        let total = self.plugins.len();
        let webapp_count = self.get_webapp_plugins().len();
        let optimization_count = self.get_optimization_plugins().len();
        let live_reload_count = self
            .plugins
            .iter()
            .filter(|plugin| plugin.info().capabilities.live_reload)
            .count();

        let mut extension_count = std::collections::HashMap::new();
        for plugin in &self.plugins {
            for ext in &plugin.info().extensions {
                *extension_count.entry(ext.clone()).or_insert(0) += 1;
            }
        }

        PluginStats {
            total_plugins: total,
            webapp_plugins: webapp_count,
            optimization_plugins: optimization_count,
            live_reload_plugins: live_reload_count,
            supported_extensions: extension_count.keys().cloned().collect(),
        }
    }

    /// Validate that all required plugins are available
    #[allow(dead_code)]
    pub fn validate_plugin_availability(&self) -> crate::error::Result<()> {
        let required_plugins = ["rust", "go", "c", "asc", "python"];
        let missing: Vec<_> = required_plugins
            .iter()
            .filter(|name| self.get_plugin_by_name(name).is_none())
            .collect();

        if !missing.is_empty() {
            let missing_names: Vec<&str> = missing.into_iter().cloned().collect();
            return Err(crate::error::ChakraError::from(format!(
                "Missing required plugins: {}",
                missing_names.join(", ")
            )));
        }

        Ok(())
    }

    /// Get all plugins that support live reload
    #[allow(dead_code)]
    pub fn get_live_reload_plugins(&self) -> Vec<&dyn Plugin> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.info().capabilities.live_reload)
            .map(|boxed| boxed.as_ref())
            .collect()
    }

    /// Find the best plugin for a project based on scoring
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

    /// Calculate a score for how well a plugin matches a project
    fn calculate_plugin_score(&self, plugin: &dyn Plugin, project_path: &str) -> f32 {
        let info = plugin.info();
        let mut score = 1.0; // Base score for can_handle_project returning true

        // Check for entry files
        for entry_file in &info.entry_files {
            let entry_path = std::path::Path::new(project_path).join(entry_file);
            if entry_path.exists() {
                score += 2.0;
            }
        }

        // Check for file extensions in the project
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

    /// Install a plugin
    pub fn install_plugin(&mut self, source: PluginSource) -> crate::error::Result<()> {
        // TODO: Implement external plugin installation
        let _plugin = external::install_plugin(source)?;
        // self.add_plugin(plugin);
        Ok(())
    }

    /// Uninstall a plugin
    pub fn uninstall_plugin(&mut self, name: &str) -> crate::error::Result<()> {
        // TODO: Implement external plugin uninstallation
        external::uninstall_plugin(name)?;
        self.plugins.retain(|p| p.info().name != name);
        Ok(())
    }

    /// Update a plugin
    pub fn update_plugin(&mut self, _name: &str) -> crate::error::Result<()> {
        // TODO: Implement external plugin updates
        Ok(())
    }

    /// Enable or disable a plugin
    pub fn set_plugin_enabled(&mut self, _name: &str, _enabled: bool) -> crate::error::Result<()> {
        // TODO: Implement plugin enable/disable
        Ok(())
    }

    /// Get the plugin registry
    pub fn registry(&self) -> PluginRegistry {
        PluginRegistry::new()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            plugins: Vec::new(),
        })
    }
}

/// Available plugins stats
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PluginStats {
    pub total_plugins: usize,
    pub webapp_plugins: usize,
    pub optimization_plugins: usize,
    pub live_reload_plugins: usize,
    pub supported_extensions: Vec<String>,
}

/// Simplified plugin registry for manager compatibility
pub struct PluginRegistry;

impl PluginRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn get_plugin(&self, name: &str) -> Option<&PluginInfo> {
        // TODO: Implement registry lookup
        // For now, return None as external plugins aren't fully implemented
        let _ = name;
        None
    }
}
