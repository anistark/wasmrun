//! Plugin registry for managing built-in and external plugins

use crate::error::{Result, WasmrunError};
use crate::plugin::config::{ExternalPluginEntry, WasmrunConfig};
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExternalPluginStats {
    pub total_installed: usize,
    pub enabled_count: usize,
    pub disabled_count: usize,
    pub supported_languages: Vec<String>,
}

pub struct LocalPluginRegistry {
    config: WasmrunConfig,
}

impl LocalPluginRegistry {
    pub fn load() -> Result<Self> {
        let config = WasmrunConfig::load_or_default()?;
        Ok(Self { config })
    }

    #[allow(dead_code)]
    pub fn add_plugin(
        &mut self,
        name: String,
        info: PluginInfo,
        source: PluginSource,
        install_path: String,
    ) -> Result<()> {
        self.config
            .add_external_plugin(name, info, source, install_path)?;
        Ok(())
    }

    pub fn remove_plugin(&mut self, name: &str) -> Result<()> {
        self.config.remove_external_plugin(name)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_installed(&self, name: &str) -> bool {
        self.config.is_external_plugin_installed(name)
    }

    pub fn get_installed_plugin(&self, name: &str) -> Option<&ExternalPluginEntry> {
        self.config.get_external_plugin(name)
    }

    pub fn get_installed_plugins(&self) -> Vec<&PluginInfo> {
        self.config.get_external_plugins()
    }

    pub fn set_plugin_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        self.config.set_external_plugin_enabled(name, enabled)
    }

    #[allow(dead_code)]
    pub fn update_plugin_metadata(&mut self, name: &str, info: PluginInfo) -> Result<()> {
        self.config.update_external_plugin_metadata(name, info)
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> ExternalPluginStats {
        let (total_installed, enabled_count, disabled_count, supported_languages) =
            self.config.get_external_plugin_stats();

        ExternalPluginStats {
            total_installed,
            enabled_count,
            disabled_count,
            supported_languages,
        }
    }
}

pub struct RegistryManager {
    local_registry: LocalPluginRegistry,
    #[allow(dead_code)] // TODO: Use when remote registry caching is implemented
    remote_cache: HashMap<String, RegistryEntry>,
    #[allow(dead_code)] // TODO: Use when remote registry caching is implemented
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

    pub fn local_registry(&self) -> &LocalPluginRegistry {
        &self.local_registry
    }

    pub fn local_registry_mut(&mut self) -> &mut LocalPluginRegistry {
        &mut self.local_registry
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn search_external_registries(&self, _query: &str) -> Result<Vec<RegistryEntry>> {
        // TODO: Implement external registry search
        Ok(Vec::new())
    }

    #[allow(dead_code)]
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
    let config_path = plugin_dir.join("plugin.toml");
    if config_path.exists() {
        return detect_from_plugin_toml(&config_path, plugin_name, source);
    }

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

fn detect_from_plugin_toml(
    config_path: &std::path::Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    let config_content = std::fs::read_to_string(config_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read plugin config: {}", e)))?;

    let config: PluginConfig = toml::from_str(&config_content)
        .map_err(|e| WasmrunError::from(format!("Failed to parse plugin config: {}", e)))?;

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

fn fetch_crates_io_metadata(crate_name: &str) -> Result<PluginInfo> {
    if let Ok(api_info) = fetch_from_crates_io_api(crate_name) {
        return Ok(api_info);
    }

    if let Ok(show_info) = fetch_from_cargo_show(crate_name) {
        return Ok(show_info);
    }

    fetch_from_cargo_search(crate_name)
}

fn fetch_from_cargo_show(crate_name: &str) -> Result<PluginInfo> {
    let output = Command::new("cargo").args(["show", crate_name]).output();

    if let Ok(output) = output {
        if output.status.success() {
            let show_output = String::from_utf8_lossy(&output.stdout);
            return parse_cargo_show_output(crate_name, &show_output);
        }
    }

    Err(WasmrunError::from("cargo show failed"))
}

fn fetch_from_cargo_search(crate_name: &str) -> Result<PluginInfo> {
    let output = Command::new("cargo")
        .args(["search", crate_name, "--limit", "1"])
        .output()
        .map_err(|e| WasmrunError::from(format!("Failed to search crates.io: {}", e)))?;

    if !output.status.success() {
        return Err(WasmrunError::from("Failed to fetch crate metadata"));
    }

    let search_output = String::from_utf8_lossy(&output.stdout);

    for line in search_output.lines() {
        if let Some(parts) = line.split_once(" = ") {
            let name = parts.0.trim();
            if name == crate_name {
                let rest = parts.1;
                if let Some((version_part, description_part)) = rest.split_once("    # ") {
                    let version = version_part.trim_matches('"').trim();
                    let description = description_part.trim();

                    return Ok(PluginInfo {
                        name: crate_name.to_string(),
                        version: version.to_string(),
                        description: description.to_string(),
                        author: "Unknown (from cargo search)".to_string(),
                        extensions: guess_extensions_from_name(crate_name),
                        entry_files: guess_entry_files_from_name(crate_name),
                        plugin_type: PluginType::External,
                        source: Some(PluginSource::CratesIo {
                            name: crate_name.to_string(),
                            version: "*".to_string(),
                        }),
                        dependencies: vec![],
                        capabilities: guess_capabilities_from_name(crate_name),
                    });
                }
            }
        }
    }

    Err(WasmrunError::from("Crate not found in search results"))
}

fn parse_cargo_show_output(crate_name: &str, output: &str) -> Result<PluginInfo> {
    let mut version = "0.1.0".to_string();
    let mut description = "External plugin".to_string();
    let mut author = "Unknown".to_string();

    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("version:") {
            version = line.split(':').nth(1).unwrap_or("0.1.0").trim().to_string();
        } else if line.starts_with("description:") {
            description = line
                .split(':')
                .nth(1)
                .unwrap_or("External plugin")
                .trim()
                .to_string();
        } else if line.starts_with("authors:") {
            author = line
                .split(':')
                .nth(1)
                .unwrap_or("Unknown")
                .trim()
                .to_string();
            author = author.trim_matches(&['[', ']', '"', ' '][..]).to_string();
        }
    }

    Ok(PluginInfo {
        name: crate_name.to_string(),
        version,
        description,
        author,
        extensions: guess_extensions_from_name(crate_name),
        entry_files: guess_entry_files_from_name(crate_name),
        plugin_type: PluginType::External,
        source: Some(PluginSource::CratesIo {
            name: crate_name.to_string(),
            version: "*".to_string(),
        }),
        dependencies: vec![],
        capabilities: guess_capabilities_from_name(crate_name),
    })
}

fn fetch_from_crates_io_api(crate_name: &str) -> Result<PluginInfo> {
    let curl_result = Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "10",
            "--user-agent",
            "wasmrun/0.10.4",
            &format!("https://crates.io/api/v1/crates/{}", crate_name),
        ])
        .output();

    if let Ok(output) = curl_result {
        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            if json_str.len() > 50 && json_str.contains("\"crate\"") {
                return parse_crates_io_json(crate_name, &json_str);
            }
        }
    }

    Err(WasmrunError::from("API call failed"))
}

fn parse_crates_io_json(crate_name: &str, json_str: &str) -> Result<PluginInfo> {
    let mut version = "0.1.0".to_string();
    let mut description = "External plugin".to_string();
    let mut author = "Unknown".to_string();

    // Extract version
    if let Some(start) = json_str.find("\"max_version\":\"") {
        let start = start + 15;
        if let Some(end) = json_str[start..].find('"') {
            version = json_str[start..start + end].to_string();
        }
    }

    // Extract description
    if let Some(start) = json_str.find("\"description\":\"") {
        let start = start + 15;
        if let Some(end) = json_str[start..].find('"') {
            description = json_str[start..start + end].to_string();
        }
    }

    // Extract author
    if let Some(published_start) = json_str.find("\"published_by\":{") {
        let published_section = &json_str[published_start..];

        if let Some(name_start) = published_section.find("\"name\":\"") {
            let name_start = published_start + name_start + 8;
            if let Some(end) = json_str[name_start..].find('"') {
                author = json_str[name_start..name_start + end].to_string();
            }
        } else if let Some(login_start) = published_section.find("\"login\":\"") {
            let login_start = published_start + login_start + 9;
            if let Some(end) = json_str[login_start..].find('"') {
                author = json_str[login_start..login_start + end].to_string();
            }
        }
    }

    if author == "Unknown" {
        if let Some(start) = json_str.find("\"authors\":[\"") {
            let start = start + 12;
            if let Some(end) = json_str[start..].find('"') {
                author = json_str[start..start + end].to_string();
            }
        }
    }

    Ok(PluginInfo {
        name: crate_name.to_string(),
        version,
        description,
        author,
        extensions: guess_extensions_from_name(crate_name),
        entry_files: guess_entry_files_from_name(crate_name),
        plugin_type: PluginType::External,
        source: Some(PluginSource::CratesIo {
            name: crate_name.to_string(),
            version: "*".to_string(),
        }),
        dependencies: vec![],
        capabilities: guess_capabilities_from_name(crate_name),
    })
}

fn guess_extensions_from_name(crate_name: &str) -> Vec<String> {
    match crate_name {
        name if name.contains("rust") || name.contains("rs") => vec!["rs".to_string()],
        name if name.contains("go") => vec!["go".to_string()],
        name if name.contains("c") || name.contains("cpp") => {
            vec!["c".to_string(), "cpp".to_string()]
        }
        name if name.contains("python") || name.contains("py") => vec!["py".to_string()],
        name if name.contains("js") || name.contains("typescript") => {
            vec!["js".to_string(), "ts".to_string()]
        }
        _ => vec![],
    }
}

fn guess_entry_files_from_name(crate_name: &str) -> Vec<String> {
    match crate_name {
        name if name.contains("rust") || name.contains("rs") => {
            vec!["main.rs".to_string(), "lib.rs".to_string()]
        }
        name if name.contains("go") => vec!["main.go".to_string()],
        name if name.contains("c") || name.contains("cpp") => {
            vec!["main.c".to_string(), "main.cpp".to_string()]
        }
        name if name.contains("python") || name.contains("py") => vec!["main.py".to_string()],
        _ => vec![],
    }
}

fn guess_capabilities_from_name(_crate_name: &str) -> PluginCapabilities {
    PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["wasm".to_string()],
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
