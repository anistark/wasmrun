use crate::error::Result;
use crate::plugin::config::ExternalPluginEntry;
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
        // Return empty - plugins are discovered dynamically through installation
        Ok(HashMap::new())
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
                "Plugin '{name}' not found in registry"
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

    /// Gets plugin metadata from crates.io
    pub fn get_plugin_metadata(
        plugin_name: &str,
    ) -> Result<crate::plugin::metadata::PluginMetadata> {
        fetch_plugin_metadata_from_crates_io(plugin_name)
    }

    /// Creates a plugin entry
    pub fn create_plugin_entry(plugin_name: &str) -> Result<ExternalPluginEntry> {
        use crate::plugin::external::ExternalPluginLoader;
        ExternalPluginLoader::create_generic_entry(plugin_name)
    }

    /// Checks plugin dependencies from crates.io
    #[allow(dead_code)]
    pub fn check_plugin_dependencies(plugin_name: &str) -> Vec<String> {
        fetch_plugin_dependencies_from_crates_io(plugin_name).unwrap_or_default()
    }
}

fn fetch_plugin_metadata_from_crates_io(
    plugin_name: &str,
) -> Result<crate::plugin::metadata::PluginMetadata> {
    use crate::error::WasmrunError;

    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg(format!("https://crates.io/api/v1/crates/{plugin_name}"))
        .output()
        .map_err(|e| WasmrunError::from(format!("Failed to fetch metadata from crates.io: {e}")))?;

    if !output.status.success() {
        return Err(WasmrunError::from(format!(
            "Failed to query crates.io for {plugin_name}"
        )));
    }

    let response = String::from_utf8_lossy(&output.stdout);
    parse_crate_metadata(&response, plugin_name)
}

fn parse_crate_metadata(
    response: &str,
    plugin_name: &str,
) -> Result<crate::plugin::metadata::PluginMetadata> {
    use crate::error::WasmrunError;
    use crate::plugin::metadata::PluginMetadata;
    use serde_json::Value;

    let json: Value = serde_json::from_str(response)
        .map_err(|e| WasmrunError::from(format!("Failed to parse crates.io response: {e}")))?;

    let crate_info = json["crate"]
        .as_object()
        .ok_or_else(|| WasmrunError::from("Invalid crates.io response format".to_string()))?;

    let version = crate_info["max_version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let description = crate_info["description"]
        .as_str()
        .unwrap_or("No description")
        .to_string();

    Ok(PluginMetadata {
        name: plugin_name.to_string(),
        version,
        description,
        author: crate_info["id"].as_str().unwrap_or("unknown").to_string(),
        extensions: vec![],
        entry_files: vec![],
        capabilities: crate::plugin::metadata::MetadataCapabilities {
            compile_wasm: true,
            compile_webapp: false,
            live_reload: false,
            optimization: true,
            custom_targets: vec![],
            supported_languages: None,
        },
        dependencies: crate::plugin::metadata::MetadataDependencies {
            tools: fetch_plugin_dependencies_from_crates_io(plugin_name).unwrap_or_default(),
            optional_tools: None,
        },
        exports: None,
        frameworks: None,
    })
}

fn fetch_plugin_dependencies_from_crates_io(plugin_name: &str) -> Result<Vec<String>> {
    use crate::error::WasmrunError;

    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg(format!(
            "https://crates.io/api/v1/crates/{plugin_name}/dependencies"
        ))
        .output()
        .map_err(|e| {
            WasmrunError::from(format!("Failed to fetch dependencies from crates.io: {e}"))
        })?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let response = String::from_utf8_lossy(&output.stdout);
    parse_crate_dependencies(&response)
}

fn parse_crate_dependencies(response: &str) -> Result<Vec<String>> {
    use crate::error::WasmrunError;
    use serde_json::Value;

    let json: Value = serde_json::from_str(response)
        .map_err(|e| WasmrunError::from(format!("Failed to parse dependencies response: {e}")))?;

    let mut dependencies = Vec::new();

    if let Some(deps) = json["dependencies"].as_array() {
        for dep in deps.iter() {
            if let Some(name) = dep["crate_id"].as_str() {
                dependencies.push(name.to_string());
            }
        }
    }

    Ok(dependencies)
}
