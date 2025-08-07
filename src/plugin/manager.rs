//! Plugin management and registry

use crate::compiler::builder::WasmBuilder;
use crate::error::{Result, WasmrunError};
use crate::plugin::builtin::load_all_builtin_plugins;
use crate::plugin::config::{ExternalPluginEntry, WasmrunConfig};
use crate::plugin::external::ExternalPluginLoader;
use crate::plugin::{Plugin, PluginInfo, PluginSource, PluginCapabilities};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PluginStats {
    pub builtin_count: usize,
    pub external_count: usize,
    pub enabled_count: usize,
    pub available_count: usize,
}

pub struct PluginManager {
    builtin_plugins: Vec<Box<dyn Plugin>>,
    external_plugins: HashMap<String, Box<dyn Plugin>>,
    config: WasmrunConfig,
    plugin_stats: PluginStats,
}

impl PluginManager {
    pub fn new() -> Result<Self> {
        let config = WasmrunConfig::load().unwrap_or_default();
        let mut manager = Self {
            builtin_plugins: vec![],
            external_plugins: HashMap::new(),
            config,
            plugin_stats: PluginStats {
                builtin_count: 0,
                external_count: 0,
                enabled_count: 0,
                available_count: 0,
            },
        };

        manager.load_all_plugins()?;
        manager.update_stats();
        Ok(manager)
    }

    fn load_all_plugins(&mut self) -> Result<()> {
        load_all_builtin_plugins(&mut self.builtin_plugins)?;

        for (name, entry) in &self.config.external_plugins {
            if entry.enabled {
                match ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        println!("✅ Loaded external plugin: {}", name);
                        self.external_plugins.insert(name.clone(), plugin);
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to load external plugin '{}': {}", name, e);
                        eprintln!("   This plugin will be unavailable for compilation.");
                    }
                }
            }
        }

        Ok(())
    }

    fn update_stats(&mut self) {
        let builtin_count = self.builtin_plugins.len();
        let external_count = self.external_plugins.len();
        let enabled_count = self.config.external_plugins.values()
            .filter(|entry| entry.enabled)
            .count() + builtin_count;
        let available_count = builtin_count + self.external_plugins.len();

        self.plugin_stats = PluginStats {
            builtin_count,
            external_count,
            enabled_count,
            available_count,
        };
    }

    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        for plugin in self.external_plugins.values() {
            if plugin.can_handle_project(project_path) {
                return Some(plugin.as_ref());
            }
        }

        for plugin in &self.builtin_plugins {
            if plugin.can_handle_project(project_path) {
                return Some(plugin.as_ref());
            }
        }

        None
    }

    pub fn get_builder_for_project(&self, project_path: &str) -> Option<Box<dyn WasmBuilder>> {
        if let Some(plugin) = self.find_plugin_for_project(project_path) {
            Some(plugin.get_builder())
        } else {
            None
        }
    }

    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        let mut plugins = Vec::new();

        for plugin in &self.builtin_plugins {
            plugins.push(plugin.info());
        }

        for plugin in self.external_plugins.values() {
            plugins.push(plugin.info());
        }

        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        plugins
    }

    pub fn get_plugin_info(&self, name: &str) -> Option<&PluginInfo> {
        if let Some(plugin) = self.external_plugins.get(name) {
            return Some(plugin.info());
        }

        for plugin in &self.builtin_plugins {
            if plugin.info().name == name {
                return Some(plugin.info());
            }
        }

        None
    }

    pub fn find_plugin_by_name(&self, name: &str) -> Option<&dyn Plugin> {
        if let Some(plugin) = self.external_plugins.get(name) {
            return Some(plugin.as_ref());
        }

        for plugin in &self.builtin_plugins {
            if plugin.info().name == name {
                return Some(plugin.as_ref());
            }
        }

        None
    }

    pub fn get_plugin_by_language(&self, language: &str) -> Option<&dyn Plugin> {
        let normalized = language.to_lowercase();

        let plugin_name = match normalized.as_str() {
            "rust" | "rs" => "wasmrust",
            "go" => "wasmgo",
            "c" | "cpp" | "c++" | "cc" | "cxx" => "c",
            "assemblyscript" | "asc" | "as" => "assemblyscript",
            "python" | "py" => "python",
            "javascript" | "js" | "typescript" | "ts" => "javascript",
            _ => &normalized,
        };

        self.find_plugin_by_name(plugin_name)
    }

    pub fn get_available_languages(&self) -> Vec<String> {
        let mut languages = Vec::new();

        for plugin in &self.builtin_plugins {
            languages.push(plugin.info().name.clone());
        }

        for plugin in self.external_plugins.values() {
            languages.push(plugin.info().name.clone());
        }

        languages.sort();
        languages.dedup();
        languages
    }

    pub fn get_auto_detected_plugins(&self) -> Vec<String> {
        let mut detected = Vec::new();
        let known_plugins = ["wasmrust", "wasmgo"];

        for plugin_name in &known_plugins {
            if Self::is_external_binary_available(plugin_name)
                && !self.config.external_plugins.contains_key(*plugin_name)
            {
                detected.push(plugin_name.to_string());
            }
        }

        detected
    }

    fn is_external_binary_available(plugin_name: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        Command::new(which_cmd)
            .arg(plugin_name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn update_plugin(&mut self, plugin_name: &str) -> Result<()> {
        if plugin_name == "all" {
            self.reload_external_plugins()
        } else {
            self.reload_single_plugin(plugin_name)
        }
    }

    pub fn enable_plugin(&mut self, plugin_name: &str) -> Result<()> {
        if let Some(entry) = self.config.external_plugins.get_mut(plugin_name) {
            entry.enabled = true;
            self.config.save()?;
            self.reload_single_plugin(plugin_name)?;
        } else {
            return Err(WasmrunError::from(format!("Plugin '{}' not found", plugin_name)));
        }
        Ok(())
    }

    pub fn disable_plugin(&mut self, plugin_name: &str) -> Result<()> {
        if let Some(entry) = self.config.external_plugins.get_mut(plugin_name) {
            entry.enabled = false;
            self.config.save()?;
            self.external_plugins.remove(plugin_name);
            self.update_stats();
        } else {
            return Err(WasmrunError::from(format!("Plugin '{}' not found", plugin_name)));
        }
        Ok(())
    }

    pub fn uninstall_plugin(&mut self, plugin_name: &str) -> Result<()> {
        if self.external_plugins.contains_key(plugin_name) {
            self.external_plugins.remove(plugin_name);
        }
        
        if self.config.external_plugins.contains_key(plugin_name) {
            self.config.external_plugins.remove(plugin_name);
            self.config.save()?;
        }
        
        self.update_stats();
        Ok(())
    }

    pub fn get_plugin_capabilities(&self, plugin_name: &str) -> Option<&PluginCapabilities> {
        if let Some(info) = self.get_plugin_info(plugin_name) {
            Some(&info.capabilities)
        } else {
            None
        }
    }

    pub fn is_plugin_enabled(&self, plugin_name: &str) -> bool {
        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            entry.enabled
        } else {
            self.builtin_plugins.iter()
                .any(|p| p.info().name == plugin_name)
        }
    }

    pub fn get_plugin_source_info(&self, plugin_name: &str) -> Option<String> {
        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            match &entry.source {
                PluginSource::CratesIo { name, version } => {
                    Some(format!("crates.io: {} v{}", name, version))
                }
                PluginSource::Git { url, branch } => {
                    if let Some(branch) = branch {
                        Some(format!("Git: {} ({})", url, branch))
                    } else {
                        Some(format!("Git: {}", url))
                    }
                }
                PluginSource::Local { path } => {
                    Some(format!("Local: {}", path.display()))
                }
            }
        } else {
            None
        }
    }

    pub fn get_plugins_by_capability(&self, capability: PluginCapabilityFilter) -> Vec<&PluginInfo> {
        let mut matching_plugins = Vec::new();

        for plugin in &self.builtin_plugins {
            if self.matches_capability_filter(plugin.info(), &capability) {
                matching_plugins.push(plugin.info());
            }
        }

        for plugin in self.external_plugins.values() {
            if self.matches_capability_filter(plugin.info(), &capability) {
                matching_plugins.push(plugin.info());
            }
        }

        matching_plugins
    }

    fn matches_capability_filter(&self, info: &PluginInfo, filter: &PluginCapabilityFilter) -> bool {
        match filter {
            PluginCapabilityFilter::CompileWasm => info.capabilities.compile_wasm,
            PluginCapabilityFilter::CompileWebapp => info.capabilities.compile_webapp,
            PluginCapabilityFilter::LiveReload => info.capabilities.live_reload,
            PluginCapabilityFilter::Optimization => info.capabilities.optimization,
            PluginCapabilityFilter::Extension(ext) => info.extensions.contains(ext),
        }
    }

    pub fn update_plugin_config(&mut self, plugin_name: &str, entry: ExternalPluginEntry) -> Result<()> {
        self.config.external_plugins.insert(plugin_name.to_string(), entry);
        self.config.save()?;
        
        if self.external_plugins.contains_key(plugin_name) {
            self.reload_single_plugin(plugin_name)?;
        }
        
        Ok(())
    }

    fn reload_single_plugin(&mut self, plugin_name: &str) -> Result<()> {
        self.external_plugins.remove(plugin_name);
        
        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            if entry.enabled {
                match ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        self.external_plugins.insert(plugin_name.to_string(), plugin);
                        self.update_stats();
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to reload plugin '{}': {}", plugin_name, e);
                    }
                }
            }
        }
        
        Ok(())
    }

    pub fn reload_external_plugins(&mut self) -> Result<()> {
        self.external_plugins.clear();
        
        for (name, entry) in &self.config.external_plugins {
            if entry.enabled {
                match ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        self.external_plugins.insert(name.clone(), plugin);
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to reload external plugin '{}': {}", name, e);
                    }
                }
            }
        }
        
        self.update_stats();
        Ok(())
    }

    pub fn export_plugin_config(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.config)
            .map_err(|e| WasmrunError::from(format!("Failed to export config: {}", e)))
    }

    pub fn import_plugin_config(&mut self, config_json: &str) -> Result<()> {
        let new_config: WasmrunConfig = serde_json::from_str(config_json)
            .map_err(|e| WasmrunError::from(format!("Failed to parse config: {}", e)))?;
        
        self.config = new_config;
        self.config.save()?;
        self.reload_external_plugins()?;
        
        Ok(())
    }

    pub fn plugin_counts(&self) -> (usize, usize, usize) {
        (
            self.plugin_stats.builtin_count,
            self.plugin_stats.external_count,
            self.plugin_stats.enabled_count
        )
    }

    pub fn get_stats(&self) -> &PluginStats {
        &self.plugin_stats
    }

    pub fn get_config(&self) -> &WasmrunConfig {
        &self.config
    }

    pub fn get_external_plugins(&self) -> &HashMap<String, Box<dyn Plugin>> {
        &self.external_plugins
    }

    pub fn get_builtin_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.builtin_plugins
    }

    pub fn detect_project_plugin(&self, project_path: &str) -> Option<String> {
        if let Some(plugin) = self.find_plugin_for_project(project_path) {
            Some(plugin.info().name.clone())
        } else {
            None
        }
    }

    pub fn validate_plugin_dependencies(&self, plugin_name: &str) -> Vec<String> {
        if let Some(plugin) = self.find_plugin_by_name(plugin_name) {
            let builder = plugin.get_builder();
            builder.check_dependencies()
        } else {
            vec![format!("Plugin '{}' not found", plugin_name)]
        }
    }

    // Detect the actual version of an installed external plugin
    fn detect_plugin_version(&self, plugin_name: &str) -> String {
        if let Ok(output) = std::process::Command::new(plugin_name)
            .arg("--version")
            .output() 
        {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                if let Some(version_line) = version_output.lines().next() {
                    let parts: Vec<&str> = version_line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        return parts[1].to_string();
                    }
                    if let Ok(re) = regex::Regex::new(r"(\d+\.\d+\.\d+)") {
                        if let Some(cap) = re.captures(&version_output) {
                            if let Some(version) = cap.get(1) {
                                return version.as_str().to_string();
                            }
                        }
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("cargo")
            .args(&["search", plugin_name, "--limit", "1"])
            .output()
        {
            if output.status.success() {
                let search_output = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = search_output.lines().next() {
                    if let Ok(re) = regex::Regex::new(r#"=\s*"([^"]+)""#) {
                        if let Some(cap) = re.captures(line) {
                            if let Some(version) = cap.get(1) {
                                return version.as_str().to_string();
                            }
                        }
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("cargo")
            .args(&["install", "--list"])
            .output()
        {
            if output.status.success() {
                let install_output = String::from_utf8_lossy(&output.stdout);
                for line in install_output.lines() {
                    if line.starts_with(plugin_name) {
                        if let Ok(re) = regex::Regex::new(r"v(\d+\.\d+\.\d+)") {
                            if let Some(cap) = re.captures(line) {
                                if let Some(version) = cap.get(1) {
                                    return version.as_str().to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        "unknown".to_string()
    }

    pub fn is_plugin_installed(&self, plugin_name: &str) -> bool {
        // Check if it's a builtin plugin
        if self.builtin_plugins.iter().any(|p| p.info().name == plugin_name) {
            return true;
        }

        // Check if it's an external plugin in config
        if self.config.external_plugins.contains_key(plugin_name) {
            return true;
        }

        false
    }

    pub fn register_installed_plugin(&mut self, plugin_name: &str) -> Result<()> {
        // Check if plugin binary is actually available
        if !self.is_tool_available_in_path(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin binary '{}' not found in PATH after installation",
                plugin_name
            )));
        }

        // Detect plugin version
        let detected_version = self.detect_plugin_version(plugin_name);

        // Create plugin entry
        let entry = match plugin_name {
            "wasmrust" => ExternalPluginLoader::create_wasmrust_entry(),
            "wasmgo" => ExternalPluginLoader::create_wasmgo_entry(),
            _ => return Err(WasmrunError::from(format!(
                "Plugin '{}' registration not supported", plugin_name
            ))),
        };

        // Update version with detected version
        let mut entry = entry;
        entry.info.version = detected_version.clone();
        if let PluginSource::CratesIo { ref mut version, .. } = entry.source {
            *version = detected_version;
        }

        // Update install path to actual plugin directory
        let plugin_dir = self.get_plugin_directory(plugin_name)?;
        entry.install_path = plugin_dir.to_string_lossy().to_string();

        // Add to config and save
        self.config.external_plugins.insert(plugin_name.to_string(), entry);
        self.config.save()?;

        // Try to load the plugin
        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            match ExternalPluginLoader::load(entry) {
                Ok(plugin) => {
                    self.external_plugins.insert(plugin_name.to_string(), plugin);
                    self.update_stats();
                    println!("Plugin '{}' registered and loaded successfully", plugin_name);
                }
                Err(e) => {
                    eprintln!("Plugin '{}' registered but failed to load: {}", plugin_name, e);
                    eprintln!("The plugin will be available for basic operations");
                }
            }
        }

        Ok(())
    }

    pub fn check_plugin_health(&self, plugin_name: &str) -> Result<PluginHealthStatus> {
        // Check if plugin exists in builtin or external
        if let Some(plugin) = self.find_plugin_by_name(plugin_name) {
            let builder = plugin.get_builder();
            let missing_deps = builder.check_dependencies();
            
            if missing_deps.is_empty() {
                Ok(PluginHealthStatus::Healthy)
            } else {
                Ok(PluginHealthStatus::MissingDependencies(missing_deps))
            }
        } else if self.config.external_plugins.contains_key(plugin_name) {
            // Plugin is in config but not loaded
            let entry = self.config.external_plugins.get(plugin_name).unwrap();
            if !entry.enabled {
                Ok(PluginHealthStatus::LoadError("Plugin is disabled".to_string()))
            } else {
                // Try to check if binary is available
                if self.is_tool_available_in_path(plugin_name) {
                    Ok(PluginHealthStatus::LoadError("Plugin binary exists but failed to load".to_string()))
                } else {
                    Ok(PluginHealthStatus::NotFound)
                }
            }
        } else {
            Ok(PluginHealthStatus::NotFound)
        }
    }

    fn get_plugin_directory(&self, plugin_name: &str) -> Result<std::path::PathBuf> {
        let config_dir = crate::plugin::config::WasmrunConfig::config_dir()?;
        Ok(config_dir.join("plugins").join(plugin_name))
    }

    fn is_tool_available_in_path(&self, tool: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        std::process::Command::new(which_cmd)
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn install_plugin(&mut self, plugin_name: &str) -> Result<()> {
        // Check if already installed
        if self.is_plugin_installed(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is already installed", plugin_name
            )));
        }

        // Check if it's a supported plugin
        if !is_supported_external_plugin(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not supported. Supported plugins: wasmrust, wasmgo", 
                plugin_name
            )));
        }

        // The actual installation (cargo install) should already be done
        // by the command handler, so we just register it here
        if !self.is_tool_available_in_path(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' binary not found. Please ensure it was installed correctly via cargo install",
                plugin_name
            )));
        }

        self.register_installed_plugin(plugin_name)?;
        Ok(())
    }

}

#[derive(Debug, Clone)]
pub enum PluginHealthStatus {
    Healthy,
    MissingDependencies(Vec<String>),
    NotFound,
    LoadError(String),
}

#[derive(Debug, Clone)]
pub enum PluginCapabilityFilter {
    CompileWasm,
    CompileWebapp,
    LiveReload,
    Optimization,
    Extension(String),
}

fn is_supported_external_plugin(plugin: &str) -> bool {
    matches!(plugin, "wasmrust" | "wasmgo")
}
