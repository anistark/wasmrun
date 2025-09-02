//! Configuration management for Wasmrun

use crate::error::{Result, WasmrunError};
use crate::plugin::{PluginInfo, PluginSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmrunConfig {
    pub version: String,
    pub settings: GlobalSettings,
    pub plugin_configs: HashMap<String, toml::Value>,
    pub external_plugins: HashMap<String, ExternalPluginEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub auto_update: bool,
    pub registry_url: String,
    pub max_concurrent_ops: usize,
    pub cache_dir: Option<PathBuf>,
    pub install_dir: Option<PathBuf>,
    pub verbose: bool,
    pub default_optimization: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPluginEntry {
    pub info: PluginInfo,
    pub source: PluginSource,
    pub installed_at: String,
    pub enabled: bool,
    pub install_path: String,
    pub executable_path: Option<String>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            auto_update: false,
            registry_url: "https://crates.io".to_string(),
            max_concurrent_ops: 4,
            cache_dir: None,
            install_dir: None,
            verbose: false,
            default_optimization: "size".to_string(),
        }
    }
}

impl Default for WasmrunConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            settings: GlobalSettings::default(),
            plugin_configs: HashMap::new(),
            external_plugins: HashMap::new(),
        }
    }
}

impl WasmrunConfig {
    pub fn config_path() -> Result<PathBuf> {
        if let Ok(test_path) = std::env::var("WASMRUN_CONFIG_PATH") {
            return Ok(PathBuf::from(test_path).join("config.toml"));
        }

        let home_dir = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not determine home directory"))?;

        Ok(home_dir.join(".wasmrun").join("config.toml"))
    }

    pub fn config_dir() -> Result<PathBuf> {
        if let Ok(test_path) = std::env::var("WASMRUN_CONFIG_PATH") {
            return Ok(PathBuf::from(test_path));
        }

        let home_dir = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not determine home directory"))?;

        Ok(home_dir.join(".wasmrun"))
    }

    pub fn plugin_dir() -> Result<PathBuf> {
        let config = Self::load_or_default()?;

        if let Some(install_dir) = &config.settings.install_dir {
            Ok(install_dir.clone())
        } else {
            Ok(Self::config_dir()?.join("plugins"))
        }
    }

    #[allow(dead_code)] // TODO: Future plugin cache directory management
    pub fn cache_dir() -> Result<PathBuf> {
        let config = Self::load_or_default()?;

        if let Some(cache_dir) = &config.settings.cache_dir {
            Ok(cache_dir.clone())
        } else {
            Ok(Self::config_dir()?.join("cache"))
        }
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Err(WasmrunError::Config(
                crate::error::ConfigError::FileNotFound {
                    path: format!(
                        "{}. Run 'wasmrun init' to create it.",
                        config_path.display()
                    ),
                },
            ));
        }

        if config_path.is_dir() {
            return Err(WasmrunError::Config(
                crate::error::ConfigError::InvalidValue {
                    message: format!(
                        "Config path is a directory, not a file: {}",
                        config_path.display()
                    ),
                },
            ));
        }

        let config_content = fs::read_to_string(&config_path).map_err(|e| {
            WasmrunError::Config(crate::error::ConfigError::ParseError {
                message: format!("Failed to read config file: {e}"),
            })
        })?;

        let config: Self = toml::from_str(&config_content).map_err(|e| {
            WasmrunError::Config(crate::error::ConfigError::ParseError {
                message: format!("Failed to parse TOML config file: {e}"),
            })
        })?;

        if config.version.is_empty() {
            return Err(WasmrunError::Config(
                crate::error::ConfigError::MissingRequired {
                    key: "version".to_string(),
                },
            ));
        }

        config.validate_and_setup()
    }

    pub fn load_or_default() -> Result<Self> {
        match Self::load() {
            Ok(config) => Ok(config),
            Err(_) => {
                let config = Self::default();
                config.save()?;
                Ok(config)
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                WasmrunError::from(format!("Failed to create config directory: {e}"))
            })?;
        }

        let config_content = toml::to_string_pretty(self).map_err(|e| {
            WasmrunError::Config(crate::error::ConfigError::ParseError {
                message: format!("Failed to serialize config to TOML: {e}"),
            })
        })?;

        fs::write(&config_path, config_content).map_err(|e| {
            WasmrunError::Config(crate::error::ConfigError::ParseError {
                message: format!("Failed to write config file: {e}"),
            })
        })?;

        Ok(())
    }

    #[allow(dead_code)] // TODO: Future initial configuration setup
    pub fn create_initial_config() -> Result<()> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            println!(
                "Configuration file already exists at: {}",
                config_path.display()
            );
            return Ok(());
        }

        let config = Self::default();
        config.save()?;

        println!(
            "✅ Created configuration file at: {}",
            config_path.display()
        );
        Ok(())
    }

    fn validate_and_setup(mut self) -> Result<Self> {
        let plugin_dir = if let Some(install_dir) = &self.settings.install_dir {
            install_dir.clone()
        } else {
            Self::config_dir()?.join("plugins")
        };

        let cache_dir = if let Some(cache_dir) = &self.settings.cache_dir {
            cache_dir.clone()
        } else {
            Self::config_dir()?.join("cache")
        };

        fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        fs::create_dir_all(&cache_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create cache directory: {e}")))?;

        self.settings.install_dir = Some(plugin_dir);
        self.settings.cache_dir = Some(cache_dir);

        match self.version.as_str() {
            env!("CARGO_PKG_VERSION") => {}
            _ => {
                return Err(WasmrunError::Config(
                    crate::error::ConfigError::InvalidValue {
                        message: format!(
                            "Unsupported config version: {}. Please update Wasmrun.",
                            self.version
                        ),
                    },
                ));
            }
        }

        Ok(self)
    }

    // External plugin management
    #[allow(dead_code)] // TODO: Future external plugin installation
    pub fn add_external_plugin(
        &mut self,
        name: String,
        info: PluginInfo,
        source: PluginSource,
        install_path: String,
    ) -> Result<()> {
        let entry = ExternalPluginEntry {
            info,
            source,
            installed_at: chrono::Utc::now().to_rfc3339(),
            enabled: true,
            install_path,
            executable_path: None,
        };

        self.external_plugins.insert(name, entry);
        self.save()?;
        Ok(())
    }

    #[allow(dead_code)] // TODO: Future plugin management
    pub fn remove_external_plugin(&mut self, name: &str) -> Result<()> {
        self.external_plugins.remove(name);
        self.save()?;
        Ok(())
    }

    #[allow(dead_code)] // TODO: Future plugin installation checking
    pub fn is_external_plugin_installed(&self, name: &str) -> bool {
        self.external_plugins.contains_key(name)
    }

    #[allow(dead_code)] // TODO: Get external plugin by name
    pub fn get_external_plugin(&self, name: &str) -> Option<&ExternalPluginEntry> {
        self.external_plugins.get(name)
    }

    #[allow(dead_code)] // TODO: Future plugin listing functionality
    pub fn get_external_plugins(&self) -> Vec<&PluginInfo> {
        self.external_plugins
            .values()
            .map(|entry| &entry.info)
            .collect()
    }

    #[allow(dead_code)] // TODO: Future plugin enable/disable functionality
    pub fn set_external_plugin_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        if let Some(entry) = self.external_plugins.get_mut(name) {
            entry.enabled = enabled;
            self.save()?;
            Ok(())
        } else {
            Err(WasmrunError::from(format!(
                "External plugin '{name}' not found"
            )))
        }
    }

    #[allow(dead_code)] // TODO: Future plugin metadata update functionality
    pub fn update_external_plugin_metadata(&mut self, name: &str, info: PluginInfo) -> Result<()> {
        if let Some(entry) = self.external_plugins.get_mut(name) {
            entry.info = info;
            self.save()?;
            Ok(())
        } else {
            Err(WasmrunError::from(format!(
                "External plugin '{name}' not found"
            )))
        }
    }

    #[allow(dead_code)] // TODO: Future plugin validation system
    pub fn validate_external_plugins(&mut self) -> Result<Vec<String>> {
        let mut missing_plugins = Vec::new();
        let plugin_dir = Self::plugin_dir()?;

        let mut plugins_to_remove = Vec::new();

        for (name, entry) in &self.external_plugins {
            let install_path = plugin_dir.join(&entry.install_path);
            if !install_path.exists() {
                missing_plugins.push(name.clone());
                plugins_to_remove.push(name.clone());
            }
        }

        for name in plugins_to_remove {
            self.external_plugins.remove(&name);
        }

        if !missing_plugins.is_empty() {
            self.save()?;
        }

        Ok(missing_plugins)
    }

    #[allow(dead_code)] // TODO: Future plugin statistics system
    pub fn get_external_plugin_stats(&self) -> (usize, usize, usize, Vec<String>) {
        let total = self.external_plugins.len();
        let enabled = self.external_plugins.values().filter(|e| e.enabled).count();
        let disabled = total - enabled;

        let mut languages = Vec::new();
        for entry in self.external_plugins.values() {
            for ext in &entry.info.extensions {
                if !languages.contains(ext) {
                    languages.push(ext.clone());
                }
            }
        }
        languages.sort();

        (total, enabled, disabled, languages)
    }

    // Plugin-specific configuration
    #[allow(dead_code)] // TODO: Future plugin-specific configuration
    pub fn get_plugin_config(&self, plugin_name: &str) -> Option<&toml::Value> {
        self.plugin_configs.get(plugin_name)
    }

    #[allow(dead_code)] // TODO: Future plugin-specific configuration
    pub fn set_plugin_config(&mut self, plugin_name: String, config: toml::Value) -> Result<()> {
        self.plugin_configs.insert(plugin_name, config);
        self.save()?;
        Ok(())
    }

    #[allow(dead_code)] // TODO: Future plugin-specific configuration
    pub fn remove_plugin_config(&mut self, plugin_name: &str) -> Result<()> {
        self.plugin_configs.remove(plugin_name);
        self.save()?;
        Ok(())
    }

    #[allow(dead_code)] // TODO: Future configuration display functionality
    pub fn print_config(&self) -> Result<()> {
        let config_toml = toml::to_string_pretty(self)
            .map_err(|e| WasmrunError::from(format!("Failed to serialize config: {e}")))?;

        println!("Current Wasmrun Configuration:");
        println!("============================");
        println!("{config_toml}");

        Ok(())
    }

    #[allow(dead_code)] // TODO: Future configuration reset functionality
    pub fn reset(&mut self) -> Result<()> {
        *self = Self::default();
        self.save()?;
        Ok(())
    }
}
