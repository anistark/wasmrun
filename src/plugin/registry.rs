//! Plugin registry for managing built-in and external plugins

use crate::error::{ChakraError, Result};
use crate::plugin::{Plugin, PluginInfo, PluginSource, PluginType, PluginCapabilities};
use crate::plugin::config::{ChakraConfig, InstalledPluginEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

// TODO: Future comprehensive plugin statistics
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PluginStats {
    pub total_builtin: usize,
    pub total_external: usize,
    pub enabled_external: usize,
    pub disabled_external: usize,
    pub supported_languages: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ExternalPluginStats {
    pub total_installed: usize,
    pub enabled_count: usize,
    pub disabled_count: usize,
    pub supported_languages: Vec<String>,
}

pub struct LocalPluginRegistry {
    config: ChakraConfig,
}

impl LocalPluginRegistry {
    pub fn load() -> Result<Self> {
        let config = ChakraConfig::load_or_default()?;
        Ok(Self { config })
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        self.config.save()
    }

    pub fn add_plugin(&mut self, name: String, info: PluginInfo, source: PluginSource, install_path: String) -> Result<()> {
        self.config.add_installed_plugin(name, info, source, install_path)?;
        Ok(())
    }

    pub fn remove_plugin(&mut self, name: &str) -> Result<()> {
        self.config.remove_installed_plugin(name)?;
        Ok(())
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.config.is_plugin_installed(name)
    }

    pub fn get_installed_plugin(&self, name: &str) -> Option<&InstalledPluginEntry> {
        self.config.get_installed_plugin(name)
    }

    pub fn get_installed_plugins(&self) -> Vec<&PluginInfo> {
        self.config.get_installed_plugins()
    }

    pub fn set_plugin_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        self.config.set_plugin_enabled(name, enabled)
    }

    pub fn update_plugin_metadata(&mut self, name: &str, info: PluginInfo) -> Result<()> {
        self.config.update_plugin_metadata(name, info)
    }

    #[allow(dead_code)]
    pub fn validate_installations(&mut self) -> Result<Vec<String>> {
        self.config.validate_plugin_installations()
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> ExternalPluginStats {
        let (total_installed, enabled_count, disabled_count, supported_languages) = self.config.get_plugin_stats();
        
        ExternalPluginStats {
            total_installed,
            enabled_count,
            disabled_count,
            supported_languages,
        }
    }

    #[allow(dead_code)]
    pub fn get_external_stats(&self) -> (usize, usize, usize, Vec<String>) {
        self.config.get_plugin_stats()
    }

    #[allow(dead_code)]
    pub fn config_mut(&mut self) -> &mut ChakraConfig {
        &mut self.config
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &ChakraConfig {
        &self.config
    }
}

pub struct RegistryManager {
    local_registry: LocalPluginRegistry,
    #[allow(dead_code)]
    remote_cache: HashMap<String, RegistryEntry>,
    #[allow(dead_code)]
    cache_updated_at: Option<std::time::SystemTime>,
}

impl RegistryManager {
    pub fn new() -> Self {
        let local_registry = LocalPluginRegistry::load().unwrap_or_else(|_| {
            LocalPluginRegistry { config: ChakraConfig::default() }
        });
        
        Self {
            local_registry,
            remote_cache: HashMap::new(),
            cache_updated_at: None,
        }
    }

    pub fn local_registry(&self) -> &LocalPluginRegistry {
        &self.local_registry
    }

    pub fn local_registry_mut(&mut self) -> &mut LocalPluginRegistry {
        &mut self.local_registry
    }

    pub fn search_all(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        let builtin_plugins = self.get_builtin_plugins();
        for plugin in builtin_plugins {
            if plugin.name.to_lowercase().contains(&query_lower)
                || plugin.description.to_lowercase().contains(&query_lower)
                || plugin.extensions.iter().any(|ext| ext.to_lowercase().contains(&query_lower))
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
                        homepage: Some("https://github.com/chakra-core/chakra".to_string()),
                        repository: Some("https://github.com/chakra-core/chakra".to_string()),
                        license: Some("MIT".to_string()),
                        keywords: plugin.extensions.clone(),
                        categories: vec!["builtin".to_string(), "compiler".to_string()],
                    },
                };
                results.push(entry);
            }
        }

        // TODO: Search external registries
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
            rust_plugin::RustPlugin,
            c_plugin::CPlugin,
            asc_plugin::AscPlugin,
            python_plugin::PythonPlugin,
        };
        
        vec![
            RustPlugin::new().info().clone(),
            CPlugin::new().info().clone(),
            AscPlugin::new().info().clone(),
            PythonPlugin::new().info().clone(),
        ]
    }

    // TODO: Future comprehensive plugin discovery
    #[allow(dead_code)]
    pub fn find_plugin(&self, name: &str) -> Option<PluginInfo> {
        for plugin in self.get_builtin_plugins() {
            if plugin.name == name {
                return Some(plugin);
            }
        }

        if let Some(entry) = self.local_registry.get_installed_plugin(name) {
            return Some(entry.info.clone());
        }

        None
    }

    #[allow(dead_code)]
    pub fn plugin_exists(&self, name: &str) -> bool {
        self.find_plugin(name).is_some()
    }

    #[allow(dead_code)]
    pub fn get_comprehensive_stats(&self) -> PluginStats {
        let builtin_plugins = self.get_builtin_plugins();
        let (total_external, enabled_external, disabled_external, external_langs) = 
            self.local_registry.get_external_stats();

        let mut all_languages = Vec::new();
        
        for plugin in &builtin_plugins {
            for ext in &plugin.extensions {
                if !all_languages.contains(ext) {
                    all_languages.push(ext.clone());
                }
            }
        }
        
        for lang in external_langs {
            if !all_languages.contains(&lang) {
                all_languages.push(lang);
            }
        }
        
        all_languages.sort();

        PluginStats {
            total_builtin: builtin_plugins.len(),
            total_external,
            enabled_external,
            disabled_external,
            supported_languages: all_languages,
        }
    }

    #[allow(dead_code)]
    pub fn list_all_plugins(&self) -> (Vec<PluginInfo>, Vec<&PluginInfo>) {
        let builtin_plugins = self.get_builtin_plugins();
        let external_plugins = self.local_registry.get_installed_plugins();
        
        (builtin_plugins, external_plugins)
    }

    #[allow(dead_code)]
    pub fn is_builtin_plugin(&self, name: &str) -> bool {
        self.get_builtin_plugins().iter().any(|p| p.name == name)
    }

    #[allow(dead_code)]
    pub fn get_plugin_type(&self, name: &str) -> Option<PluginType> {
        if self.is_builtin_plugin(name) {
            Some(PluginType::Builtin)
        } else if self.local_registry.is_installed(name) {
            Some(PluginType::External)
        } else {
            None
        }
    }
}

impl Default for RegistryManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn detect_plugin_metadata(
    plugin_dir: &std::path::Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    let config_path = plugin_dir.join("chakra-plugin.toml");
    
    if config_path.exists() {
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| ChakraError::from(format!("Failed to read plugin config: {}", e)))?;
        
        let config: PluginConfig = toml::from_str(&config_content)
            .map_err(|e| ChakraError::from(format!("Failed to parse plugin config: {}", e)))?;
        
        Ok(PluginInfo {
            name: config.name.unwrap_or_else(|| plugin_name.to_string()),
            version: config.version.unwrap_or_else(|| "0.1.0".to_string()),
            description: config.description.unwrap_or_else(|| "External plugin".to_string()),
            author: config.author.unwrap_or_else(|| "Unknown".to_string()),
            extensions: config.extensions.unwrap_or_default(),
            entry_files: config.entry_files.unwrap_or_default(),
            plugin_type: PluginType::External,
            source: Some(source.clone()),
            dependencies: config.dependencies.unwrap_or_default(),
            capabilities: config.capabilities.unwrap_or_default(),
        })
    } else {
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
}

// TODO: Future legacy compatibility layer
#[allow(dead_code)]
pub struct PluginRegistry {
    #[allow(dead_code)]
    manager: RegistryManager,
}

#[allow(dead_code)]
impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            manager: RegistryManager::new(),
        }
    }

    pub fn get_plugin(&self, name: &str) -> Option<PluginInfo> {
        self.manager.find_plugin(name)
    }

    pub fn list_all(&self) -> Vec<PluginInfo> {
        let (builtin, external) = self.manager.list_all_plugins();
        let mut all_plugins = builtin;
        all_plugins.extend(external.into_iter().cloned());
        all_plugins
    }

    pub fn exists(&self, name: &str) -> bool {
        self.manager.plugin_exists(name)
    }

    pub fn get_stats(&self) -> PluginStats {
        self.manager.get_comprehensive_stats()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
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
