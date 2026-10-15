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

    // Plugin-specific configuration
}
