//! Plugin system for Chakra
//!
//! This module provides a plugin architecture that supports both built-in and external plugins.
//! Built-in plugins are compiled into the binary, while external plugins are loaded at runtime.

use crate::compiler::builder::WasmBuilder;
use crate::error::{ChakraError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub mod builtin;
pub mod config;
pub mod external;
pub mod manager;
pub mod registry;

/// Plugin metadata and information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name (e.g., "rust", "go", "python")
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// Supported file extensions
    pub extensions: Vec<String>,
    /// Entry file candidates for project detection
    pub entry_files: Vec<String>,
    /// Plugin type (builtin or external)
    pub plugin_type: PluginType,
    /// Installation source (for external plugins)
    pub source: Option<PluginSource>,
    /// Plugin dependencies (other plugins or system tools)
    pub dependencies: Vec<String>,
    /// Plugin capabilities
    pub capabilities: PluginCapabilities,
}

/// Plugin type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    /// Built-in plugin (shipped with Chakra)
    Builtin,
    /// External plugin (installed at runtime)
    External,
}

/// Plugin installation source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    /// Crates.io package
    CratesIo { name: String, version: String },
    /// Git repository
    Git { url: String, branch: Option<String> },
    /// Local path
    Local { path: PathBuf },
}

/// Plugin capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapabilities {
    /// Can compile to WebAssembly
    pub compile_wasm: bool,
    /// Can compile to web applications
    pub compile_webapp: bool,
    /// Supports live reload
    pub live_reload: bool,
    /// Supports optimization levels
    pub optimization: bool,
    /// Custom build targets
    pub custom_targets: Vec<String>,
}

impl Default for PluginCapabilities {
    fn default() -> Self {
        Self {
            compile_wasm: true,
            compile_webapp: false,
            live_reload: true,
            optimization: true,
            custom_targets: Vec::new(),
        }
    }
}

/// Plugin trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Get plugin information
    fn info(&self) -> &PluginInfo;

    /// Check if this plugin can handle the given project
    #[allow(dead_code)]
    fn can_handle_project(&self, project_path: &str) -> bool;

    /// Get the underlying WASM builder
    #[allow(dead_code)]
    fn get_builder(&self) -> Box<dyn WasmBuilder>;

    /// Initialize the plugin (called once when loaded)
    #[allow(dead_code)]
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Cleanup plugin resources (called when unloading)
    fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }

    /// Get plugin-specific commands (optional)
    #[allow(dead_code)]
    fn get_commands(&self) -> Vec<PluginCommand> {
        Vec::new()
    }
}

/// Plugin command definition
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Command name
    #[allow(dead_code)]
    pub name: String,
    /// Command description
    #[allow(dead_code)]
    pub description: String,
    /// Command handler
    #[allow(dead_code)]
    pub handler: fn(&[String]) -> Result<()>,
}

/// Plugin registry for managing all plugins
pub struct PluginRegistry {
    /// All registered plugins
    plugins: HashMap<String, Box<dyn Plugin>>,
    /// Plugin load order for dependency resolution
    load_order: Vec<String>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            load_order: Vec::new(),
        }
    }

    /// Register a plugin
    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        let name = plugin.info().name.clone();

        // Check for conflicts
        if self.plugins.contains_key(&name) {
            return Err(ChakraError::from(format!(
                "Plugin '{}' is already registered",
                name
            )));
        }

        // Add to load order
        self.load_order.push(name.clone());

        // Register the plugin
        self.plugins.insert(name, plugin);

        Ok(())
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// Get all plugins
    #[allow(dead_code)]
    pub fn get_all_plugins(&self) -> &HashMap<String, Box<dyn Plugin>> {
        &self.plugins
    }

    /// Find plugin that can handle a project
    #[allow(dead_code)]
    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        // Check plugins in load order for deterministic behavior
        for plugin_name in &self.load_order {
            if let Some(plugin) = self.plugins.get(plugin_name) {
                if plugin.can_handle_project(project_path) {
                    return Some(plugin.as_ref());
                }
            }
        }
        None
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.values().map(|p| p.info()).collect()
    }

    /// Remove a plugin
    pub fn unregister_plugin(&mut self, name: &str) -> Result<()> {
        if let Some(mut plugin) = self.plugins.remove(name) {
            plugin.cleanup()?;
            self.load_order.retain(|n| n != name);
            Ok(())
        } else {
            Err(ChakraError::from(format!("Plugin '{}' not found", name)))
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin manager for high-level plugin operations
pub struct PluginManager {
    /// Plugin registry
    registry: PluginRegistry,
    /// Configuration
    config: config::PluginConfig,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Result<Self> {
        let config = config::PluginConfig::load()?;
        let mut manager = Self {
            registry: PluginRegistry::new(),
            config,
        };

        // Load built-in plugins
        manager.load_builtin_plugins()?;

        // Load external plugins
        manager.load_external_plugins()?;

        Ok(manager)
    }

    /// Get the plugin registry
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Get mutable access to the plugin registry
    #[allow(dead_code)]
    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }

    /// Get the configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &config::PluginConfig {
        &self.config
    }

    /// Load all built-in plugins
    fn load_builtin_plugins(&mut self) -> Result<()> {
        // Load built-in plugins (will be implemented in Stage 2)
        builtin::load_all_builtin_plugins(&mut self.registry)?;
        Ok(())
    }

    /// Load all external plugins
    fn load_external_plugins(&mut self) -> Result<()> {
        // Collect plugin configs to avoid borrowing issues
        let plugin_configs = self.config.external_plugins.to_vec();

        for plugin_config in plugin_configs {
            if plugin_config.enabled {
                match external::load_external_plugin(&plugin_config) {
                    Ok(plugin) => {
                        println!("Loaded external plugin: {}", plugin.info().name);
                        if let Err(e) = self.registry.register_plugin(plugin) {
                            eprintln!("Failed to register external plugin: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to load external plugin {}: {}",
                            plugin_config.name, e
                        );
                    }
                }
            }
        }

        // Save config after loading all plugins
        self.config.save()?;
        Ok(())
    }

    /// Install an external plugin
    pub fn install_plugin(&mut self, source: PluginSource) -> Result<()> {
        let plugin = external::install_plugin(source)?;
        let plugin_name = plugin.info().name.clone();

        // Update configuration
        let external_config = external::ExternalPluginConfig {
            name: plugin_name.clone(),
            source: plugin.info().source.clone().unwrap(),
            enabled: true,
            config: HashMap::new(),
        };

        self.config.external_plugins.push(external_config);
        self.config.save()?;

        // Register the plugin
        self.registry.register_plugin(plugin)?;

        println!("Successfully installed plugin: {}", plugin_name);
        Ok(())
    }

    /// Uninstall an external plugin
    pub fn uninstall_plugin(&mut self, name: &str) -> Result<()> {
        // Check if it's a built-in plugin
        if let Some(plugin) = self.registry.get_plugin(name) {
            if plugin.info().plugin_type == PluginType::Builtin {
                return Err(ChakraError::from(format!(
                    "Cannot uninstall built-in plugin: {}",
                    name
                )));
            }
        }

        // Remove from registry
        self.registry.unregister_plugin(name)?;

        // Remove from configuration
        self.config.external_plugins.retain(|p| p.name != name);
        self.config.save()?;

        // Clean up plugin files
        external::uninstall_plugin(name)?;

        println!("Successfully uninstalled plugin: {}", name);
        Ok(())
    }

    /// List all plugins
    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.registry.list_plugins()
    }

    /// Find the best plugin for a project
    #[allow(dead_code)]
    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        self.registry.find_plugin_for_project(project_path)
    }

    /// Enable/disable a plugin
    pub fn set_plugin_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        // Clone the plugin configs to avoid borrowing issues
        let mut plugin_configs = self.config.external_plugins.clone();
        let mut found = false;

        // Update the specific plugin config
        for plugin_config in &mut plugin_configs {
            if plugin_config.name == name {
                plugin_config.enabled = enabled;
                found = true;

                if !enabled {
                    self.registry.unregister_plugin(name)?;
                } else {
                    // Reload the plugin
                    let plugin = external::load_external_plugin(plugin_config)?;
                    self.registry.register_plugin(plugin)?;
                }
                break;
            }
        }

        if found {
            // Update the config with the modified versions
            self.config.external_plugins = plugin_configs;
            self.config.save()?;
            Ok(())
        } else {
            Err(ChakraError::from(format!("Plugin '{}' not found", name)))
        }
    }

    /// Update a plugin
    pub fn update_plugin(&mut self, name: &str) -> Result<()> {
        // Find the plugin config
        let plugin_config = self
            .config
            .external_plugins
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| ChakraError::from(format!("Plugin '{}' not found", name)))?
            .clone();

        // Uninstall current version
        self.registry.unregister_plugin(name)?;

        // Reinstall latest version
        let plugin = external::install_plugin(plugin_config.source)?;
        self.registry.register_plugin(plugin)?;

        println!("Successfully updated plugin: {}", name);
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new().expect("Failed to create plugin manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry_creation() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.plugins.len(), 0);
        assert_eq!(registry.load_order.len(), 0);
    }

    #[test]
    fn test_plugin_capabilities_default() {
        let caps = PluginCapabilities::default();
        assert!(caps.compile_wasm);
        assert!(!caps.compile_webapp);
        assert!(caps.live_reload);
        assert!(caps.optimization);
        assert!(caps.custom_targets.is_empty());
    }
}
