//! Plugin management and registry

use crate::compiler::builder::WasmBuilder;
use crate::error::{Result, WasmrunError};
use crate::plugin::builtin::load_all_builtin_plugins;
use crate::plugin::config::{ExternalPluginEntry, WasmrunConfig};
use crate::plugin::external::ExternalPluginLoader;
use crate::plugin::installer::PluginInstaller;
use crate::plugin::registry::PluginRegistry;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource};
use crate::utils::PluginUtils;
use crate::{debug_enter, debug_exit, debug_println};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PluginStats {
    pub builtin_count: usize,
    pub external_count: usize,
    pub enabled_count: usize,
    #[allow(dead_code)]
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
        debug_enter!("PluginManager::new");

        debug_println!("Loading wasmrun configuration");
        let config = WasmrunConfig::load().unwrap_or_default();
        debug_println!(
            "Configuration loaded with {} external plugins configured",
            config.external_plugins.len()
        );

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

        debug_println!("Loading all plugins");
        manager.load_all_plugins()?;
        manager.update_stats();
        debug_println!(
            "Plugin manager initialized with {} builtin and {} external plugins",
            manager.plugin_stats.builtin_count,
            manager.plugin_stats.external_count
        );

        debug_exit!("PluginManager::new");
        Ok(manager)
    }

    fn load_all_plugins(&mut self) -> Result<()> {
        load_all_builtin_plugins(&mut self.builtin_plugins)?;

        for (name, entry) in &self.config.external_plugins {
            if entry.enabled {
                match ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        println!("✅ Loaded external plugin: {name}");
                        self.external_plugins.insert(name.clone(), plugin);
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to load external plugin '{name}': {e}");
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
        let enabled_count = self
            .config
            .external_plugins
            .values()
            .filter(|entry| entry.enabled)
            .count()
            + builtin_count;
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

    pub fn find_plugin_for_language(&self, language: &str) -> Option<&dyn Plugin> {
        // Check external plugins first using the common utility
        for plugin in self.external_plugins.values() {
            if PluginUtils::supports_language(plugin.as_ref(), language) {
                return Some(plugin.as_ref());
            }
        }

        // Check built-in plugins
        for plugin in &self.builtin_plugins {
            if PluginUtils::supports_language(plugin.as_ref(), language) {
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

    #[allow(dead_code)]
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
        // Use the common utility function to find plugins that support the language
        self.find_plugin_for_language(language)
    }

    #[allow(dead_code)]
    pub fn get_available_languages(&self) -> Vec<String> {
        let mut languages = Vec::new();

        // Get languages from built-in plugins
        for plugin in &self.builtin_plugins {
            let plugin_languages = PluginUtils::get_supported_languages(plugin.as_ref());
            languages.extend(plugin_languages);
        }

        // Get languages from external plugins
        for plugin in self.external_plugins.values() {
            let plugin_languages = PluginUtils::get_supported_languages(plugin.as_ref());
            languages.extend(plugin_languages);
        }

        languages.sort();
        languages.dedup();
        languages
    }

    #[allow(dead_code)]
    pub fn get_auto_detected_plugins(&self) -> Vec<String> {
        let mut detected = Vec::new();

        // Check if any installed external plugins are available
        for (name, entry) in &self.config.external_plugins {
            if entry.enabled && Self::is_external_binary_available(name) {
                detected.push(name.clone());
            }
        }

        detected
    }

    #[allow(dead_code)]
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
            self.update_all_external_plugins()
        } else {
            self.update_single_plugin(plugin_name)
        }
    }

    fn update_single_plugin(&mut self, plugin_name: &str) -> Result<()> {
        println!("🔄 Updating plugin: {plugin_name}");

        // Check if plugin exists
        if !self.config.external_plugins.contains_key(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' is not installed"
            )));
        }

        // Get current version
        let current_version = self.get_current_plugin_version(plugin_name);
        println!("📦 Current version: {current_version}");

        // Check for latest version
        let latest_version = self.get_latest_plugin_version(plugin_name)?;
        println!("🆕 Latest version: {latest_version}");

        // Compare versions
        if current_version == latest_version {
            println!("✅ Plugin '{plugin_name}' is already up to date (v{current_version})");
            return Ok(());
        }

        // Perform the update
        println!("⬆️  Updating from v{current_version} to v{latest_version}");

        // For external plugins, we need to reinstall
        self.reinstall_external_plugin(plugin_name, &latest_version)?;

        println!("✅ Plugin '{plugin_name}' updated successfully to v{latest_version}");
        Ok(())
    }

    fn update_all_external_plugins(&mut self) -> Result<()> {
        println!("🔄 Updating all external plugins...");

        let plugin_names: Vec<String> = self.config.external_plugins.keys().cloned().collect();
        let mut updated_count = 0;
        let mut failed_count = 0;

        for plugin_name in plugin_names {
            match self.update_single_plugin(&plugin_name) {
                Ok(()) => {
                    updated_count += 1;
                }
                Err(e) => {
                    eprintln!("❌ Failed to update {plugin_name}: {e}");
                    failed_count += 1;
                }
            }
        }

        println!("📊 Update summary: {updated_count} updated, {failed_count} failed");
        Ok(())
    }

    fn get_current_plugin_version(&self, plugin_name: &str) -> String {
        // First try to get version from config
        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            if let PluginSource::CratesIo { version, .. } = &entry.source {
                if version != "unknown" && !version.is_empty() {
                    return version.clone();
                }
            }

            // Also check the info version
            if !entry.info.version.is_empty() && entry.info.version != "unknown" {
                return entry.info.version.clone();
            }
        }

        // Try to detect from plugin directory files
        self.detect_plugin_version_from_directory(plugin_name)
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn get_latest_plugin_version(&self, plugin_name: &str) -> Result<String> {
        // Try to get version from crates.io for any plugin
        self.get_latest_crates_io_version(plugin_name)
    }

    fn get_latest_crates_io_version(&self, crate_name: &str) -> Result<String> {
        // Use cargo search to find the latest version
        let output = std::process::Command::new("cargo")
            .args(["search", crate_name, "--limit", "1"])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo search: {e}")))?;

        if !output.status.success() {
            return Err(WasmrunError::from(format!(
                "cargo search failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let search_output = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = search_output.lines().next() {
            // Parse output like: wasmrust = "0.3.0"    # Rust to WebAssembly compiler
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    let version = &line[start + 1..start + 1 + end];
                    return Ok(version.to_string());
                }
            }
        }

        Err(WasmrunError::from(format!(
            "Could not parse version from cargo search output for {crate_name}"
        )))
    }

    fn detect_plugin_version_from_directory(&self, plugin_name: &str) -> Option<String> {
        // Try to get plugin directory
        let plugin_dir = match PluginUtils::get_plugin_directory(plugin_name) {
            Ok(dir) => dir,
            Err(_) => return None,
        };

        // Try to read version from Cargo.toml
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                // Simple TOML parsing for version
                for line in content.lines() {
                    if line.trim().starts_with("version") && line.contains('=') {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                let version = &line[start + 1..start + 1 + end];
                                return Some(version.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Try to read from metadata file
        let metadata_path = plugin_dir.join(".wasmrun_metadata");
        if metadata_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                for line in content.lines() {
                    if line.trim().starts_with("version") && line.contains('=') {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                let version = &line[start + 1..start + 1 + end];
                                return Some(version.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    fn reinstall_external_plugin(&mut self, plugin_name: &str, new_version: &str) -> Result<()> {
        // Remove current plugin from memory
        self.external_plugins.remove(plugin_name);

        // Remove plugin directory
        PluginInstaller::remove_plugin_directory(plugin_name)?;

        // Install the plugin again (this will get the latest version)
        let _result = PluginInstaller::install_external_plugin(plugin_name)?;

        // 🔧 FIX: Update the actual plugin metadata files with the new version
        PluginInstaller::update_plugin_metadata(plugin_name, new_version)?;

        // Update the config with the new version
        if let Some(entry) = self.config.external_plugins.get_mut(plugin_name) {
            entry.info.version = new_version.to_string();
            if let PluginSource::CratesIo { version, .. } = &mut entry.source {
                *version = new_version.to_string();
            }
        }

        // Save config
        self.config.save()?;

        // Reload the plugin
        self.reload_single_plugin(plugin_name)?;

        Ok(())
    }

    pub fn enable_plugin(&mut self, plugin_name: &str) -> Result<()> {
        if let Some(entry) = self.config.external_plugins.get_mut(plugin_name) {
            entry.enabled = true;
            self.config.save()?;
            self.reload_single_plugin(plugin_name)?;
        } else {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' not found"
            )));
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
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' not found"
            )));
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

    #[allow(dead_code)]
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
            self.builtin_plugins
                .iter()
                .any(|p| p.info().name == plugin_name)
        }
    }

    #[allow(dead_code)]
    pub fn get_plugin_source_info(&self, plugin_name: &str) -> Option<String> {
        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            match &entry.source {
                PluginSource::CratesIo { name, version } => {
                    Some(format!("crates.io: {name} v{version}"))
                }
                PluginSource::Git { url, branch } => {
                    if let Some(branch) = branch {
                        Some(format!("Git: {url} ({branch})"))
                    } else {
                        Some(format!("Git: {url}"))
                    }
                }
                PluginSource::Local { path } => Some(format!("Local: {}", path.display())),
            }
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn get_plugins_by_capability(
        &self,
        capability: PluginCapabilityFilter,
    ) -> Vec<&PluginInfo> {
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

    #[allow(dead_code)]
    fn matches_capability_filter(
        &self,
        info: &PluginInfo,
        filter: &PluginCapabilityFilter,
    ) -> bool {
        match filter {
            PluginCapabilityFilter::CompileWasm => info.capabilities.compile_wasm,
            PluginCapabilityFilter::CompileWebapp => info.capabilities.compile_webapp,
            PluginCapabilityFilter::LiveReload => info.capabilities.live_reload,
            PluginCapabilityFilter::Optimization => info.capabilities.optimization,
            PluginCapabilityFilter::Extension(ext) => info.extensions.contains(ext),
        }
    }

    #[allow(dead_code)]
    pub fn update_plugin_config(
        &mut self,
        plugin_name: &str,
        entry: ExternalPluginEntry,
    ) -> Result<()> {
        self.config
            .external_plugins
            .insert(plugin_name.to_string(), entry);
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
                match crate::plugin::external::ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        println!(
                            "✅ Successfully loaded plugin: {} v{}",
                            plugin.info().name,
                            plugin.info().version
                        );
                        self.external_plugins
                            .insert(plugin_name.to_string(), plugin);
                        self.update_stats();
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to reload plugin '{plugin_name}': {e}");
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn reload_external_plugins(&mut self) -> Result<()> {
        self.external_plugins.clear();

        for (name, entry) in &self.config.external_plugins {
            if entry.enabled {
                match ExternalPluginLoader::load(entry) {
                    Ok(plugin) => {
                        self.external_plugins.insert(name.clone(), plugin);
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to reload external plugin '{name}': {e}");
                    }
                }
            }
        }

        self.update_stats();
        Ok(())
    }

    #[allow(dead_code)]
    pub fn export_plugin_config(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.config)
            .map_err(|e| WasmrunError::from(format!("Failed to export config: {e}")))
    }

    #[allow(dead_code)]
    pub fn import_plugin_config(&mut self, config_json: &str) -> Result<()> {
        let new_config: WasmrunConfig = serde_json::from_str(config_json)
            .map_err(|e| WasmrunError::from(format!("Failed to parse config: {e}")))?;

        self.config = new_config;
        self.config.save()?;
        self.reload_external_plugins()?;

        Ok(())
    }

    pub fn plugin_counts(&self) -> (usize, usize, usize) {
        (
            self.plugin_stats.builtin_count,
            self.plugin_stats.external_count,
            self.plugin_stats.enabled_count,
        )
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> &PluginStats {
        &self.plugin_stats
    }

    #[allow(dead_code)]
    pub fn get_config(&self) -> &WasmrunConfig {
        &self.config
    }

    pub fn get_external_plugins(&self) -> &HashMap<String, Box<dyn Plugin>> {
        &self.external_plugins
    }

    pub fn get_builtin_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.builtin_plugins
    }

    #[allow(dead_code)]
    pub fn detect_project_plugin(&self, project_path: &str) -> Option<String> {
        self.find_plugin_for_project(project_path)
            .map(|plugin| plugin.info().name.clone())
    }

    #[allow(dead_code)]
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
            .args(["search", plugin_name, "--limit", "1"])
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
            .args(["install", "--list"])
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

        "unknown".to_string()
    }

    pub fn is_plugin_installed(&self, plugin_name: &str) -> bool {
        // Check if it's a builtin plugin
        if self
            .builtin_plugins
            .iter()
            .any(|p| p.info().name == plugin_name)
        {
            return true;
        }

        // Check if it's an external plugin that's actually loaded and functional
        if self.external_plugins.contains_key(plugin_name) {
            return true;
        }

        false
    }

    pub fn register_installed_plugin(&mut self, plugin_name: &str) -> Result<()> {
        let plugin_dir = self.get_plugin_directory(plugin_name)?;

        // Load metadata from the installed plugin directory
        let metadata_result =
            crate::plugin::metadata::PluginMetadata::from_installed_plugin(&plugin_dir);

        let (plugin_info, detected_version) = match metadata_result {
            Ok(metadata) => {
                println!("📋 Found plugin metadata with capabilities");
                let plugin_info = metadata.to_plugin_info();
                (plugin_info, metadata.version)
            }
            Err(_) => {
                println!("📋 Using basic plugin registration");
                let detected_version = self.detect_plugin_version(plugin_name);
                let mut entry = PluginRegistry::create_plugin_entry(plugin_name)?;
                entry.info.version = detected_version.clone();
                (entry.info, detected_version)
            }
        };

        // Check if binary exists in ~/.wasmrun/bin/
        let wasmrun_root = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not find home directory"))?
            .join(".wasmrun");
        let binary_path = wasmrun_root.join("bin").join(plugin_name);

        let executable_path = if binary_path.exists() {
            Some(binary_path.to_string_lossy().to_string())
        } else {
            None
        };

        // Create the external plugin entry with enhanced metadata
        let entry = ExternalPluginEntry {
            info: plugin_info,
            source: PluginSource::CratesIo {
                name: plugin_name.to_string(),
                version: detected_version,
            },
            installed_at: chrono::Utc::now().to_rfc3339(),
            enabled: true,
            install_path: plugin_dir.to_string_lossy().to_string(),
            executable_path,
        };

        self.config
            .external_plugins
            .insert(plugin_name.to_string(), entry);
        self.config.save()?;

        // Load the plugin
        let load_result = if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            let entry_clone = entry.clone(); // Clone to avoid borrowing issues
            match ExternalPluginLoader::load(&entry_clone) {
                Ok(plugin) => {
                    self.external_plugins
                        .insert(plugin_name.to_string(), plugin);
                    Some((entry_clone, None))
                }
                Err(e) => Some((entry_clone, Some(e))),
            }
        } else {
            None
        };

        if let Some((entry, error)) = load_result {
            self.update_stats();

            if let Some(e) = error {
                eprintln!("Plugin '{plugin_name}' registered but failed to load: {e}");
                eprintln!("   This might be due to missing dependencies or compilation issues");
                eprintln!("   Run 'wasmrun plugin info {plugin_name}' for more details");
            } else {
                let info = &entry.info;
                println!(
                    "🔌 Plugin '{plugin_name}' registered successfully (v{})",
                    info.version
                );
                println!("   📁 Extensions: {}", info.extensions.join(", "));
                println!("   📄 Entry files: {}", info.entry_files.join(", "));
                println!("   🔧 Dependencies: {}", info.dependencies.join(", "));

                if info.capabilities.compile_webapp {
                    println!("   🌐 Supports web applications");
                }
                if info.capabilities.live_reload {
                    println!("   ⚡ Supports live reload");
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn check_plugin_health(&self, plugin_name: &str) -> Result<PluginHealthStatus> {
        if !self.is_plugin_installed(plugin_name) {
            return Ok(PluginHealthStatus::NotFound);
        }

        // Use registry for dependency checking
        let missing_deps = PluginRegistry::check_plugin_dependencies(plugin_name);

        if !missing_deps.is_empty() {
            return Ok(PluginHealthStatus::MissingDependencies(missing_deps));
        }

        if let Some(entry) = self.config.external_plugins.get(plugin_name) {
            match ExternalPluginLoader::load(entry) {
                Ok(_) => Ok(PluginHealthStatus::Healthy),
                Err(e) => Ok(PluginHealthStatus::LoadError(e.to_string())),
            }
        } else {
            Ok(PluginHealthStatus::Healthy)
        }
    }

    fn get_plugin_directory(&self, plugin_name: &str) -> Result<std::path::PathBuf> {
        let config_dir = crate::plugin::config::WasmrunConfig::config_dir()?;
        Ok(config_dir.join("plugins").join(plugin_name))
    }

    #[allow(dead_code)]
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

    /// Install plugin
    pub fn install_plugin(&mut self, plugin_name: &str) -> Result<()> {
        // Check if already installed
        if self.is_plugin_installed(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' is already installed"
            )));
        }

        // Use the improved plugin installer
        let install_result = PluginInstaller::install_external_plugin(plugin_name)?;

        println!(
            "🔌 Plugin '{}' installation completed (v{})",
            plugin_name, install_result.version
        );

        // Register the newly installed plugin
        self.register_installed_plugin(plugin_name)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum PluginHealthStatus {
    Healthy,
    #[allow(dead_code)]
    MissingDependencies(Vec<String>),
    NotFound,
    #[allow(dead_code)]
    LoadError(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PluginCapabilityFilter {
    CompileWasm,
    CompileWebapp,
    LiveReload,
    Optimization,
    Extension(String),
}
