//! Plugin registry for managing built-in and external plugins

use crate::error::{Result, WasmrunError};
use crate::plugin::config::WasmrunConfig;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub info: PluginInfo,
    pub downloads: u64,
    pub updated_at: u64,
    pub registry_metadata: RegistryMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMetadata {
    pub versions: Vec<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

pub struct LocalPluginRegistry {
    #[allow(dead_code)]
    config: WasmrunConfig,
}

impl LocalPluginRegistry {
    pub fn load() -> Result<Self> {
        let config = WasmrunConfig::load_or_default()?;
        Ok(Self { config })
    }
}

#[allow(dead_code)]
pub struct RegistryManager {
    local_registry: LocalPluginRegistry,
    // TODO: Use when remote registry caching is implemented
    remote_cache: HashMap<String, RegistryEntry>,
    // TODO: Use when remote registry caching is implemented
    cache_updated_at: Option<std::time::SystemTime>,
}

impl RegistryManager {
    pub fn new() -> Self {
        let local_registry = LocalPluginRegistry::load().unwrap_or_else(|_| LocalPluginRegistry {
            config: WasmrunConfig::default(),
        });

        Self {
            local_registry,
            remote_cache: HashMap::new(),
            cache_updated_at: None,
        }
    }

    pub fn search_all(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        let builtin_plugins = self.get_builtin_plugins();
        for plugin in builtin_plugins {
            if plugin.name.to_lowercase().contains(&query_lower)
                || plugin.description.to_lowercase().contains(&query_lower)
                || plugin
                    .extensions
                    .iter()
                    .any(|ext| ext.to_lowercase().contains(&query_lower))
            {
                let entry = RegistryEntry {
                    info: plugin.clone(),
                    downloads: 0,
                    updated_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    registry_metadata: RegistryMetadata {
                        versions: vec![plugin.version.clone()],
                        documentation: None,
                        homepage: Some("https://github.com/wasmrun-core/wasmrun".to_string()),
                        repository: Some("https://github.com/wasmrun-core/wasmrun".to_string()),
                        license: Some("MIT".to_string()),
                        keywords: plugin.extensions.clone(),
                        categories: vec!["builtin".to_string(), "compiler".to_string()],
                    },
                };
                results.push(entry);
            }
        }

        // TODO: Search external registries when implemented
        let external_results = self.search_external_registries(&query_lower)?;
        results.extend(external_results);

        results.sort_by(|a, b| {
            let a_exact = a.info.name.to_lowercase() == query_lower;
            let b_exact = b.info.name.to_lowercase() == query_lower;

            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.downloads.cmp(&a.downloads),
            }
        });

        Ok(results)
    }

    fn search_external_registries(&self, _query: &str) -> Result<Vec<RegistryEntry>> {
        // TODO: Implement external registry search
        Ok(Vec::new())
    }

    pub fn get_builtin_plugins(&self) -> Vec<PluginInfo> {
        use crate::plugin::languages::{
            asc_plugin::AscPlugin, c_plugin::CPlugin, python_plugin::PythonPlugin,
        };

        vec![
            CPlugin::new().info().clone(),
            AscPlugin::new().info().clone(),
            PythonPlugin::new().info().clone(),
        ]
    }
}

impl Default for RegistryManager {
    fn default() -> Self {
        Self::new()
    }
}

// Plugin metadata detection
#[allow(dead_code)]
pub fn detect_plugin_metadata(
    plugin_dir: &std::path::Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    // Try plugin.toml first (for Git/local installs)
    let config_path = plugin_dir.join("plugin.toml");
    if config_path.exists() {
        return detect_from_plugin_toml(&config_path, plugin_name, source);
    }

    // For crates.io plugins, fetch metadata from API
    if let PluginSource::CratesIo { name, version: _ } = source {
        if let Ok(metadata) = fetch_crates_io_metadata(name) {
            return Ok(metadata);
        }
    }

    Ok(PluginInfo {
        name: plugin_name.to_string(),
        version: "0.1.0".to_string(),
        description: "External plugin".to_string(),
        author: "Unknown".to_string(),
        extensions: vec![],
        entry_files: vec![],
        plugin_type: PluginType::External,
        source: Some(source.clone()),
        dependencies: vec![],
        capabilities: PluginCapabilities::default(),
    })
}

#[allow(dead_code)]
fn detect_from_plugin_toml(
    config_path: &std::path::Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    use toml;

    let content = std::fs::read_to_string(config_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read plugin.toml: {}", e)))?;

    #[derive(Deserialize)]
    struct PluginConfig {
        name: Option<String>,
        version: Option<String>,
        description: Option<String>,
        author: Option<String>,
        extensions: Option<Vec<String>>,
        entry_files: Option<Vec<String>>,
        dependencies: Option<Vec<String>>,
        capabilities: Option<PluginCapabilities>,
    }

    let config: PluginConfig = toml::from_str(&content)
        .map_err(|e| WasmrunError::from(format!("Failed to parse plugin.toml: {}", e)))?;

    Ok(PluginInfo {
        name: config.name.unwrap_or_else(|| plugin_name.to_string()),
        version: config.version.unwrap_or_else(|| "0.1.0".to_string()),
        description: config
            .description
            .unwrap_or_else(|| "External plugin".to_string()),
        author: config.author.unwrap_or_else(|| "Unknown".to_string()),
        extensions: config.extensions.unwrap_or_default(),
        entry_files: config.entry_files.unwrap_or_default(),
        plugin_type: PluginType::External,
        source: Some(source.clone()),
        dependencies: config.dependencies.unwrap_or_default(),
        capabilities: config.capabilities.unwrap_or_default(),
    })
}

#[allow(dead_code)]
fn fetch_crates_io_metadata(crate_name: &str) -> Result<PluginInfo> {
    let url = format!("https://crates.io/api/v1/crates/{}", crate_name);
    let output = Command::new("curl")
        .args(["-s", &url])
        .output()
        .map_err(|e| WasmrunError::from(format!("Failed to fetch crates.io metadata: {}", e)))?;

    if !output.status.success() {
        return Err(WasmrunError::from(
            "Failed to fetch crates.io metadata".to_string(),
        ));
    }

    #[derive(Deserialize)]
    struct CratesIoResponse {
        #[serde(rename = "crate")]
        crate_info: CrateInfo,
    }

    #[derive(Deserialize)]
    struct CrateInfo {
        name: String,
        description: Option<String>,
        max_version: String,
        #[serde(default)]
        keywords: Vec<String>,
    }

    let response_text = String::from_utf8_lossy(&output.stdout);
    let response: CratesIoResponse = serde_json::from_str(&response_text)
        .map_err(|e| WasmrunError::from(format!("Failed to parse crates.io response: {}", e)))?;

    Ok(PluginInfo {
        name: response.crate_info.name,
        version: response.crate_info.max_version,
        description: response
            .crate_info
            .description
            .unwrap_or_else(|| "External plugin from crates.io".to_string()),
        author: "Unknown".to_string(),
        extensions: response.crate_info.keywords,
        entry_files: vec![],
        plugin_type: PluginType::External,
        source: Some(PluginSource::CratesIo {
            name: crate_name.to_string(),
            version: "latest".to_string(),
        }),
        dependencies: vec![],
        capabilities: PluginCapabilities::default(),
    })
}
