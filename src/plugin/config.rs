//! Plugin configuration management
//!
//! Handles reading and writing the ~/.chakra configuration file in TOML format

use crate::error::{ChakraError, Result};
use crate::plugin::{external::ExternalPluginConfig, PluginSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Main plugin configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Configuration version for future migrations
    pub version: String,
    /// Global plugin settings
    pub settings: GlobalSettings,
    /// External plugin configurations
    pub external_plugins: Vec<ExternalPluginConfig>,
    /// Plugin-specific configurations
    pub plugin_configs: HashMap<String, toml::Value>,
}

/// Global plugin settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// Whether to auto-update plugins
    pub auto_update: bool,
    /// Default plugin registry URL
    pub registry_url: String,
    /// Maximum number of concurrent plugin operations
    pub max_concurrent_ops: usize,
    /// Plugin cache directory
    pub cache_dir: Option<PathBuf>,
    /// Plugin installation directory
    pub install_dir: Option<PathBuf>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            auto_update: false,
            registry_url: "https://crates.io".to_string(),
            max_concurrent_ops: 4,
            cache_dir: None,
            install_dir: None,
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            settings: GlobalSettings::default(),
            external_plugins: Vec::new(),
            plugin_configs: HashMap::new(),
        }
    }
}

impl PluginConfig {
    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        // Check for test override first
        if let Ok(test_path) = std::env::var("CHAKRA_CONFIG_PATH") {
            return Ok(PathBuf::from(test_path));
        }

        let home_dir = dirs::home_dir()
            .ok_or_else(|| ChakraError::from("Could not determine home directory"))?;

        Ok(home_dir.join(".chakra"))
    }

    /// Get the plugin directory path
    pub fn plugin_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| ChakraError::from("Could not determine home directory"))?;

        Ok(home_dir.join(".chakra").join("plugins"))
    }

    /// Get the cache directory path
    pub fn cache_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| ChakraError::from("Could not determine home directory"))?;

        Ok(home_dir.join(".chakra").join("cache"))
    }

    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            // Create default configuration
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        // Make sure we're reading a file, not a directory
        if config_path.is_dir() {
            return Err(ChakraError::from(format!(
                "Config path is a directory, not a file: {}",
                config_path.display()
            )));
        }

        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| ChakraError::from(format!("Failed to read config file: {}", e)))?;

        let config: Self = toml::from_str(&config_content)
            .map_err(|e| ChakraError::from(format!("Failed to parse TOML config file: {}", e)))?;

        // Validate and migrate if necessary
        config.validate_and_migrate()
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ChakraError::from(format!("Failed to create config directory: {}", e))
            })?;
        }

        let config_content = toml::to_string_pretty(self)
            .map_err(|e| ChakraError::from(format!("Failed to serialize config to TOML: {}", e)))?;

        fs::write(&config_path, config_content)
            .map_err(|e| ChakraError::from(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Create initial configuration file during installation
    #[allow(dead_code)]
    pub fn create_initial_config() -> Result<()> {
        let config_path = Self::config_path()?;

        // Don't overwrite existing config
        if config_path.exists() {
            println!(
                "Configuration file already exists at: {}",
                config_path.display()
            );
            return Ok(());
        }

        // Create the default configuration
        let config = Self::default();
        config.save()?;

        println!(
            "✅ Created configuration file at: {}",
            config_path.display()
        );
        println!("🔧 You can customize your settings by editing this file");

        Ok(())
    }

    /// Validate configuration and migrate if necessary
    fn validate_and_migrate(mut self) -> Result<Self> {
        // Ensure directories exist
        let plugin_dir = self
            .settings
            .install_dir
            .clone()
            .unwrap_or_else(|| Self::plugin_dir().unwrap());

        let cache_dir = self
            .settings
            .cache_dir
            .clone()
            .unwrap_or_else(|| Self::cache_dir().unwrap());

        // Create directories if they don't exist
        fs::create_dir_all(&plugin_dir)
            .map_err(|e| ChakraError::from(format!("Failed to create plugin directory: {}", e)))?;

        fs::create_dir_all(&cache_dir)
            .map_err(|e| ChakraError::from(format!("Failed to create cache directory: {}", e)))?;

        // Update paths in config
        self.settings.install_dir = Some(plugin_dir);
        self.settings.cache_dir = Some(cache_dir);

        // Validate version and migrate if necessary
        match self.version.as_str() {
            "1.0.0" => {
                // Current version, no migration needed
            }
            _ => {
                // Future versions can implement migration logic here
                return Err(ChakraError::from(format!(
                    "Unsupported config version: {}",
                    self.version
                )));
            }
        }

        Ok(self)
    }

    /// Add a new external plugin configuration
    #[allow(dead_code)]
    pub fn add_external_plugin(&mut self, config: ExternalPluginConfig) -> Result<()> {
        // Check for duplicates
        if self.external_plugins.iter().any(|p| p.name == config.name) {
            return Err(ChakraError::from(format!(
                "Plugin '{}' is already configured",
                config.name
            )));
        }

        self.external_plugins.push(config);
        Ok(())
    }

    /// Remove an external plugin configuration
    #[allow(dead_code)]
    pub fn remove_external_plugin(&mut self, name: &str) -> Result<()> {
        let initial_len = self.external_plugins.len();
        self.external_plugins.retain(|p| p.name != name);

        if self.external_plugins.len() == initial_len {
            return Err(ChakraError::from(format!(
                "Plugin '{}' not found in configuration",
                name
            )));
        }

        Ok(())
    }

    /// Get configuration for a specific plugin
    #[allow(dead_code)]
    pub fn get_plugin_config(&self, plugin_name: &str) -> Option<&toml::Value> {
        self.plugin_configs.get(plugin_name)
    }

    /// Set configuration for a specific plugin
    #[allow(dead_code)]
    pub fn set_plugin_config(&mut self, plugin_name: String, config: toml::Value) {
        self.plugin_configs.insert(plugin_name, config);
    }

    /// Remove configuration for a specific plugin
    #[allow(dead_code)]
    pub fn remove_plugin_config(&mut self, plugin_name: &str) {
        self.plugin_configs.remove(plugin_name);
    }

    /// Get all enabled external plugins
    #[allow(dead_code)]
    pub fn get_enabled_plugins(&self) -> Vec<&ExternalPluginConfig> {
        self.external_plugins.iter().filter(|p| p.enabled).collect()
    }

    /// Enable or disable a plugin
    #[allow(dead_code)]
    pub fn set_plugin_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        for plugin in &mut self.external_plugins {
            if plugin.name == name {
                plugin.enabled = enabled;
                return Ok(());
            }
        }

        Err(ChakraError::from(format!("Plugin '{}' not found", name)))
    }

    /// Reset configuration to defaults
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Export configuration to a file
    #[allow(dead_code)]
    pub fn export_to_file(&self, path: &PathBuf) -> Result<()> {
        let config_content = toml::to_string_pretty(self)
            .map_err(|e| ChakraError::from(format!("Failed to serialize config to TOML: {}", e)))?;

        fs::write(path, config_content)
            .map_err(|e| ChakraError::from(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Import configuration from a file
    #[allow(dead_code)]
    pub fn import_from_file(path: &PathBuf) -> Result<Self> {
        let config_content = fs::read_to_string(path)
            .map_err(|e| ChakraError::from(format!("Failed to read config file: {}", e)))?;

        let config: Self = toml::from_str(&config_content)
            .map_err(|e| ChakraError::from(format!("Failed to parse TOML config file: {}", e)))?;

        config.validate_and_migrate()
    }

    /// Print the configuration in a human-readable format
    #[allow(dead_code)]
    pub fn print_config(&self) -> Result<()> {
        let config_toml = toml::to_string_pretty(self)
            .map_err(|e| ChakraError::from(format!("Failed to serialize config: {}", e)))?;

        println!("Current Chakra Configuration:");
        println!("============================");
        println!("{}", config_toml);

        Ok(())
    }
}

/// Configuration validation utilities
#[allow(dead_code)]
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate plugin source
    #[allow(dead_code)]
    pub fn validate_plugin_source(source: &PluginSource) -> Result<()> {
        match source {
            PluginSource::CratesIo { name, version } => {
                if name.is_empty() {
                    return Err(ChakraError::from("Plugin name cannot be empty"));
                }
                if version.is_empty() {
                    return Err(ChakraError::from("Plugin version cannot be empty"));
                }
            }
            PluginSource::Git { url, .. } => {
                if url.is_empty() {
                    return Err(ChakraError::from("Git URL cannot be empty"));
                }
                // Basic URL validation
                if !url.starts_with("http://")
                    && !url.starts_with("https://")
                    && !url.starts_with("git://")
                {
                    return Err(ChakraError::from("Invalid Git URL format"));
                }
            }
            PluginSource::Local { path } => {
                if !path.exists() {
                    return Err(ChakraError::from(format!(
                        "Local plugin path does not exist: {}",
                        path.display()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate global settings
    #[allow(dead_code)]
    pub fn validate_settings(settings: &GlobalSettings) -> Result<()> {
        if settings.max_concurrent_ops == 0 {
            return Err(ChakraError::from(
                "max_concurrent_ops must be greater than 0",
            ));
        }

        if settings.max_concurrent_ops > 20 {
            return Err(ChakraError::from("max_concurrent_ops should not exceed 20"));
        }

        if settings.registry_url.is_empty() {
            return Err(ChakraError::from("registry_url cannot be empty"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = PluginConfig::default();
        assert_eq!(config.version, "1.0.0");
        assert!(!config.settings.auto_update);
        assert_eq!(config.settings.registry_url, "https://crates.io");
        assert_eq!(config.settings.max_concurrent_ops, 4);
        assert!(config.external_plugins.is_empty());
        assert!(config.plugin_configs.is_empty());
    }

    #[test]
    fn test_config_toml_serialization() {
        let config = PluginConfig::default();
        let toml_string = toml::to_string_pretty(&config).unwrap();
        let deserialized: PluginConfig = toml::from_str(&toml_string).unwrap();

        assert_eq!(config.version, deserialized.version);
        assert_eq!(
            config.settings.auto_update,
            deserialized.settings.auto_update
        );
    }

    #[test]
    fn test_plugin_source_validation() {
        // Valid crates.io source
        let source = PluginSource::CratesIo {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
        };
        assert!(ConfigValidator::validate_plugin_source(&source).is_ok());

        // Invalid empty name
        let source = PluginSource::CratesIo {
            name: "".to_string(),
            version: "1.0.0".to_string(),
        };
        assert!(ConfigValidator::validate_plugin_source(&source).is_err());

        // Valid git source
        let source = PluginSource::Git {
            url: "https://github.com/user/plugin.git".to_string(),
            branch: None,
        };
        assert!(ConfigValidator::validate_plugin_source(&source).is_ok());

        // Invalid git URL
        let source = PluginSource::Git {
            url: "invalid-url".to_string(),
            branch: None,
        };
        assert!(ConfigValidator::validate_plugin_source(&source).is_err());
    }

    #[test]
    fn test_settings_validation() {
        let mut settings = GlobalSettings::default();
        assert!(ConfigValidator::validate_settings(&settings).is_ok());

        // Invalid max_concurrent_ops
        settings.max_concurrent_ops = 0;
        assert!(ConfigValidator::validate_settings(&settings).is_err());

        settings.max_concurrent_ops = 25;
        assert!(ConfigValidator::validate_settings(&settings).is_err());

        // Invalid empty registry URL
        settings.max_concurrent_ops = 4;
        settings.registry_url = "".to_string();
        assert!(ConfigValidator::validate_settings(&settings).is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test.toml");

        let mut config = PluginConfig::default();
        config.settings.auto_update = true;
        config.settings.max_concurrent_ops = 8;

        // Save config
        let config_toml = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&config_path, &config_toml).unwrap();

        // Load config
        let loaded_config = PluginConfig::import_from_file(&config_path).unwrap();

        assert_eq!(
            config.settings.auto_update,
            loaded_config.settings.auto_update
        );
        assert_eq!(
            config.settings.max_concurrent_ops,
            loaded_config.settings.max_concurrent_ops
        );
    }
}
