//! Plugin registry for discovering and managing available plugins
//!
//! This module handles plugin discovery, metadata caching, and registry operations

use crate::error::{ChakraError, Result};
use crate::plugin::{PluginInfo, PluginSource, PluginType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Plugin registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Plugin information
    pub info: PluginInfo,
    /// Download count
    pub downloads: u64,
    /// Last updated timestamp
    pub updated_at: u64,
    /// Registry-specific metadata
    pub registry_metadata: RegistryMetadata,
}

/// Registry-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMetadata {
    /// Available versions
    pub versions: Vec<String>,
    /// Documentation URL
    pub documentation: Option<String>,
    /// Homepage URL
    pub homepage: Option<String>,
    /// Repository URL
    pub repository: Option<String>,
    /// License information
    pub license: Option<String>,
    /// Keywords for search
    pub keywords: Vec<String>,
    /// Categories
    pub categories: Vec<String>,
}

/// Plugin registry interface
pub trait PluginRegistry {
    /// Search for plugins matching a query
    fn search(&self, query: &str) -> Result<Vec<RegistryEntry>>;

    /// Get plugin information by name
    fn get_plugin(&self, name: &str) -> Result<Option<RegistryEntry>>;

    /// List all available plugins
    fn list_plugins(&self, limit: Option<usize>) -> Result<Vec<RegistryEntry>>;

    /// Get plugin versions
    #[allow(dead_code)]
    fn get_versions(&self, name: &str) -> Result<Vec<String>>;

    /// Check if registry is available
    fn is_available(&self) -> bool;

    /// Get registry name
    fn name(&self) -> &str;
}

/// Crates.io registry implementation
pub struct CratesIoRegistry {
    /// Base URL for the registry
    #[allow(dead_code)]
    base_url: String,
    /// Cache for registry entries
    cache: HashMap<String, RegistryEntry>,
    /// Cache expiry time (in seconds)
    cache_ttl: u64,
}

impl CratesIoRegistry {
    /// Create a new crates.io registry
    pub fn new() -> Self {
        Self {
            base_url: "https://crates.io/api/v1".to_string(),
            cache: HashMap::new(),
            cache_ttl: 3600, // 1 hour
        }
    }

    /// Create a registry with custom base URL
    #[allow(dead_code)]
    pub fn with_url(url: String) -> Self {
        Self {
            base_url: url,
            cache: HashMap::new(),
            cache_ttl: 3600,
        }
    }

    /// Fetch plugin data from crates.io API
    fn fetch_crate_info(&self, name: &str) -> Result<RegistryEntry> {
        // TODO: Make HTTP requests to crates.io API. For now, return a placeholder
        let info = PluginInfo {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: format!("Plugin from crates.io: {}", name),
            author: "Unknown".to_string(),
            extensions: vec![],
            entry_files: vec![],
            plugin_type: PluginType::External,
            source: Some(PluginSource::CratesIo {
                name: name.to_string(),
                version: "1.0.0".to_string(),
            }),
            dependencies: vec![],
            capabilities: crate::plugin::PluginCapabilities::default(),
        };

        let metadata = RegistryMetadata {
            versions: vec!["1.0.0".to_string()],
            documentation: None,
            homepage: None,
            repository: None,
            license: Some("MIT".to_string()),
            keywords: vec!["wasm".to_string(), "chakra".to_string()],
            categories: vec!["development-tools".to_string()],
        };

        let entry = RegistryEntry {
            info,
            downloads: 100,
            updated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            registry_metadata: metadata,
        };

        Ok(entry)
    }

    /// Search crates.io for Chakra plugins
    fn search_crates(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        // TODO: search the crates.io API
        // Look for crates with "chakra-plugin" or similar keywords

        // Placeholder implementation
        let known_plugins = vec!["chakra-zig-plugin"];

        let mut results = Vec::new();

        for plugin_name in known_plugins {
            if plugin_name.contains(query) || query.is_empty() {
                if let Ok(entry) = self.fetch_crate_info(plugin_name) {
                    results.push(entry);
                }
            }
        }

        Ok(results)
    }
}

impl Default for CratesIoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry for CratesIoRegistry {
    fn search(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        println!("Searching crates.io for plugins matching '{}'...", query);
        self.search_crates(query)
    }

    fn get_plugin(&self, name: &str) -> Result<Option<RegistryEntry>> {
        // Check cache first
        if let Some(entry) = self.cache.get(name) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now - entry.updated_at < self.cache_ttl {
                return Ok(Some(entry.clone()));
            }
        }

        // Fetch from registry
        match self.fetch_crate_info(name) {
            Ok(entry) => Ok(Some(entry)),
            Err(_) => Ok(None),
        }
    }

    fn list_plugins(&self, limit: Option<usize>) -> Result<Vec<RegistryEntry>> {
        let mut plugins = self.search_crates("")?;

        // Sort by downloads (popular plugins first)
        plugins.sort_by(|a, b| b.downloads.cmp(&a.downloads));

        if let Some(limit) = limit {
            plugins.truncate(limit);
        }

        Ok(plugins)
    }

    fn get_versions(&self, name: &str) -> Result<Vec<String>> {
        if let Some(entry) = self.get_plugin(name)? {
            Ok(entry.registry_metadata.versions)
        } else {
            Err(ChakraError::from(format!(
                "Plugin '{}' not found in registry",
                name
            )))
        }
    }

    fn is_available(&self) -> bool {
        // TODO: check network connectivity to crates.io
        true
    }

    fn name(&self) -> &str {
        "crates.io"
    }
}

/// GitHub registry for Git-based plugins
pub struct GitHubRegistry {
    /// Organization or user to search
    organization: String,
    /// Cache for registry entries
    #[allow(dead_code)]
    cache: HashMap<String, RegistryEntry>,
}

impl GitHubRegistry {
    /// Create a new GitHub registry
    pub fn new(organization: String) -> Self {
        Self {
            organization,
            cache: HashMap::new(),
        }
    }

    /// Search GitHub repositories for Chakra plugins
    fn search_repositories(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        // TODO: use GitHub API
        // For now, return placeholder data

        let example_plugins = vec![
            format!("chakra-{}-plugin", query),
            format!("{}-chakra-plugin", query),
        ];

        let mut results = Vec::new();

        for plugin_name in example_plugins {
            let info = PluginInfo {
                name: plugin_name.clone(),
                version: "main".to_string(),
                description: format!("GitHub plugin: {}", plugin_name),
                author: self.organization.clone(),
                extensions: vec![],
                entry_files: vec![],
                plugin_type: PluginType::External,
                source: Some(PluginSource::Git {
                    url: format!(
                        "https://github.com/{}/{}.git",
                        self.organization, plugin_name
                    ),
                    branch: Some("main".to_string()),
                }),
                dependencies: vec![],
                capabilities: crate::plugin::PluginCapabilities::default(),
            };

            let metadata = RegistryMetadata {
                versions: vec!["main".to_string()],
                documentation: None,
                homepage: Some(format!(
                    "https://github.com/{}/{}",
                    self.organization, plugin_name
                )),
                repository: Some(format!(
                    "https://github.com/{}/{}",
                    self.organization, plugin_name
                )),
                license: Some("MIT".to_string()),
                keywords: vec!["wasm".to_string(), "chakra".to_string()],
                categories: vec!["development-tools".to_string()],
            };

            let entry = RegistryEntry {
                info,
                downloads: 0, // GitHub doesn't have download counts like crates.io
                updated_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                registry_metadata: metadata,
            };

            results.push(entry);
        }

        Ok(results)
    }
}

impl PluginRegistry for GitHubRegistry {
    fn search(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        println!(
            "Searching GitHub ({}) for plugins matching '{}'...",
            self.organization, query
        );
        self.search_repositories(query)
    }

    fn get_plugin(&self, name: &str) -> Result<Option<RegistryEntry>> {
        let results = self.search_repositories(name)?;
        Ok(results.into_iter().find(|entry| entry.info.name == name))
    }

    fn list_plugins(&self, limit: Option<usize>) -> Result<Vec<RegistryEntry>> {
        let mut plugins = self.search_repositories("")?;

        if let Some(limit) = limit {
            plugins.truncate(limit);
        }

        Ok(plugins)
    }

    fn get_versions(&self, _name: &str) -> Result<Vec<String>> {
        // GitHub repositories typically use branches/tags for versions
        Ok(vec!["main".to_string(), "dev".to_string()])
    }

    fn is_available(&self) -> bool {
        // TODO: check GitHub API availability
        true
    }

    fn name(&self) -> &str {
        "github"
    }
}

/// Multi-registry manager
pub struct RegistryManager {
    /// All registered registries
    registries: Vec<Box<dyn PluginRegistry>>,
    /// Cache for search results
    #[allow(dead_code)]
    search_cache: HashMap<String, Vec<RegistryEntry>>,
}

impl RegistryManager {
    /// Create a new registry manager
    pub fn new() -> Self {
        let mut manager = Self {
            registries: Vec::new(),
            search_cache: HashMap::new(),
        };

        // Add default registries
        manager.add_registry(Box::new(CratesIoRegistry::new()));
        manager.add_registry(Box::new(GitHubRegistry::new("chakra-plugins".to_string())));

        manager
    }

    /// Add a registry to the manager
    pub fn add_registry(&mut self, registry: Box<dyn PluginRegistry>) {
        self.registries.push(registry);
    }

    /// Search all registries for plugins
    #[allow(dead_code)]
    pub fn search_all(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        let mut all_results = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for registry in &self.registries {
            if !registry.is_available() {
                continue;
            }

            match registry.search(query) {
                Ok(results) => {
                    for entry in results {
                        // Avoid duplicates
                        if seen_names.insert(entry.info.name.clone()) {
                            all_results.push(entry);
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to search registry '{}': {}",
                        registry.name(),
                        e
                    );
                }
            }
        }

        // Sort by downloads (popular plugins first)
        all_results.sort_by(|a, b| b.downloads.cmp(&a.downloads));

        Ok(all_results)
    }

    /// Get plugin from any registry
    #[allow(dead_code)]
    pub fn get_plugin(&self, name: &str) -> Result<Option<RegistryEntry>> {
        for registry in &self.registries {
            if !registry.is_available() {
                continue;
            }

            match registry.get_plugin(name) {
                Ok(Some(entry)) => return Ok(Some(entry)),
                Ok(None) => continue,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to get plugin from registry '{}': {}",
                        registry.name(),
                        e
                    );
                }
            }
        }

        Ok(None)
    }

    /// List plugins from all registries
    #[allow(dead_code)]
    pub fn list_all(&self, limit: Option<usize>) -> Result<Vec<RegistryEntry>> {
        let mut all_plugins = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for registry in &self.registries {
            if !registry.is_available() {
                continue;
            }

            match registry.list_plugins(None) {
                Ok(plugins) => {
                    for plugin in plugins {
                        if seen_names.insert(plugin.info.name.clone()) {
                            all_plugins.push(plugin);
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to list plugins from registry '{}': {}",
                        registry.name(),
                        e
                    );
                }
            }
        }

        // Sort by downloads
        all_plugins.sort_by(|a, b| b.downloads.cmp(&a.downloads));

        if let Some(limit) = limit {
            all_plugins.truncate(limit);
        }

        Ok(all_plugins)
    }

    /// Get available registries
    #[allow(dead_code)]
    pub fn get_registries(&self) -> Vec<&str> {
        self.registries.iter().map(|r| r.name()).collect()
    }

    /// Clear search cache
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.search_cache.clear();
    }
}

impl Default for RegistryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry search filters
#[derive(Debug, Clone)]
pub struct SearchFilters {
    /// Category filter
    #[allow(dead_code)]
    pub category: Option<String>,
    /// Minimum download count
    #[allow(dead_code)]
    pub min_downloads: Option<u64>,
    /// Language/extension filter
    #[allow(dead_code)]
    pub language: Option<String>,
    /// Sort order
    #[allow(dead_code)]
    pub sort_by: SortBy,
}

/// Sort options for search results
#[derive(Debug, Clone)]
pub enum SortBy {
    /// Sort by popularity (download count)
    Popularity,
    /// Sort by recent updates
    #[allow(dead_code)]
    RecentlyUpdated,
    /// Sort alphabetically
    #[allow(dead_code)]
    Name,
    /// Sort by relevance to search query
    #[allow(dead_code)]
    Relevance,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            category: None,
            min_downloads: None,
            language: None,
            sort_by: SortBy::Popularity,
        }
    }
}

/// Apply filters to search results
#[allow(dead_code)]
pub fn apply_filters(entries: Vec<RegistryEntry>, filters: &SearchFilters) -> Vec<RegistryEntry> {
    let mut filtered: Vec<RegistryEntry> = entries
        .into_iter()
        .filter(|entry| {
            // Category filter
            if let Some(category) = &filters.category {
                if !entry.registry_metadata.categories.contains(category) {
                    return false;
                }
            }

            // Download count filter
            if let Some(min_downloads) = filters.min_downloads {
                if entry.downloads < min_downloads {
                    return false;
                }
            }

            // Language/extension filter
            if let Some(language) = &filters.language {
                if !entry.info.extensions.contains(language) {
                    return false;
                }
            }

            true
        })
        .collect();

    // Apply sorting
    match filters.sort_by {
        SortBy::Popularity => {
            filtered.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        }
        SortBy::RecentlyUpdated => {
            filtered.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
        SortBy::Name => {
            filtered.sort_by(|a, b| a.info.name.cmp(&b.info.name));
        }
        SortBy::Relevance => {
            // TODO: Implement relevance. For now, using popularity
            filtered.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        }
    }

    filtered
}
