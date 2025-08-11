use crate::error::Result;
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::external::ExternalPluginLoader;
use std::collections::HashMap;

#[allow(dead_code)]
pub struct PluginRegistry {
    entries: HashMap<String, ExternalPluginEntry>,
}

impl PluginRegistry {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn get_default_external_plugins() -> Result<HashMap<String, ExternalPluginEntry>> {
        let mut plugins = HashMap::new();

        // Use generic entry creation for all external plugins
        for plugin_name in &["wasmrust", "wasmgo", "wasmzig", "wasmjs"] {
            if let Ok(entry) = ExternalPluginLoader::create_generic_entry(plugin_name) {
                plugins.insert(plugin_name.to_string(), entry);
            }
        }

        Ok(plugins)
    }

    #[allow(dead_code)]
    pub fn add_entry(&mut self, name: String, entry: ExternalPluginEntry) {
        self.entries.insert(name, entry);
    }

    #[allow(dead_code)]
    pub fn get_entry(&self, name: &str) -> Option<&ExternalPluginEntry> {
        self.entries.get(name)
    }

    #[allow(dead_code)]
    pub fn remove_entry(&mut self, name: &str) -> Option<ExternalPluginEntry> {
        self.entries.remove(name)
    }

    #[allow(dead_code)]
    pub fn list_entries(&self) -> &HashMap<String, ExternalPluginEntry> {
        &self.entries
    }

    #[allow(dead_code)]
    pub fn search_plugins(&self, query: &str) -> Vec<&ExternalPluginEntry> {
        self.entries
            .values()
            .filter(|entry| {
                entry
                    .info
                    .name
                    .to_lowercase()
                    .contains(&query.to_lowercase())
                    || entry
                        .info
                        .description
                        .to_lowercase()
                        .contains(&query.to_lowercase())
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn get_plugin_count(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_plugin_registered(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    #[allow(dead_code)]
    pub fn update_entry(&mut self, name: &str, entry: ExternalPluginEntry) -> Result<()> {
        if self.entries.contains_key(name) {
            self.entries.insert(name.to_string(), entry);
            Ok(())
        } else {
            Err(crate::error::WasmrunError::from(format!(
                "Plugin '{}' not found in registry",
                name
            )))
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Validates if a plugin exists and is installable
    pub fn validate_plugin(plugin_name: &str) -> Result<bool> {
        if plugin_name.is_empty() {
            return Ok(false);
        }

        // Check crates.io for the plugin
        let output = std::process::Command::new("cargo")
            .args(["search", plugin_name, "--limit", "1"])
            .output()
            .map_err(|e| {
                crate::error::WasmrunError::from(format!("Failed to search crates.io: {e}"))
            })?;

        if !output.status.success() {
            return Ok(false);
        }

        let search_output = String::from_utf8_lossy(&output.stdout);
        Ok(!search_output.trim().is_empty())
    }

    /// Gets plugin metadata (stub for now - implement when metadata system is ready)
    pub fn get_plugin_metadata(
        _plugin_name: &str,
    ) -> Result<crate::plugin::metadata::PluginMetadata> {
        Err(crate::error::WasmrunError::from(
            "Plugin metadata not yet implemented".to_string(),
        ))
    }

    /// Creates a plugin entry
    pub fn create_plugin_entry(plugin_name: &str) -> Result<ExternalPluginEntry> {
        use crate::plugin::external::ExternalPluginLoader;
        ExternalPluginLoader::create_generic_entry(plugin_name)
    }

    /// Checks if a plugin is supported
    pub fn is_supported_external_plugin(plugin_name: &str) -> bool {
        Self::validate_plugin(plugin_name).unwrap_or(false)
    }

    /// Checks plugin dependencies (stub for now)
    #[allow(dead_code)]
    pub fn check_plugin_dependencies(_plugin_name: &str) -> Vec<String> {
        vec![] // Return empty for now - implement when dependency system is ready
    }
}
