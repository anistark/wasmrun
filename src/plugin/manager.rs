//! Plugin management commands

use crate::error::{Result, WasmrunError};
use crate::plugin::config::WasmrunConfig;
use crate::plugin::external::{detect_plugin_metadata, ExternalPluginInstaller};
use crate::plugin::registry::RegistryManager;
use crate::plugin::{PluginInfo, PluginManager, PluginSource, PluginType};

pub struct PluginCommands {
    manager: PluginManager,
    config: WasmrunConfig,
    registry_manager: RegistryManager,
}

impl PluginCommands {
    pub fn new() -> Result<Self> {
        let manager = PluginManager::new()?;
        let config = WasmrunConfig::load()?;
        let registry_manager = RegistryManager::new();

        Ok(Self {
            manager,
            config,
            registry_manager,
        })
    }

    pub fn list(&self, show_all: bool) -> Result<()> {
        let all_plugins = self.manager.list_plugins();
        let builtin_count = all_plugins
            .iter()
            .filter(|p| p.plugin_type == PluginType::Builtin)
            .count();
        let external_count = all_plugins
            .iter()
            .filter(|p| p.plugin_type == PluginType::External)
            .count();

        println!("📦 \x1b[1;36mInstalled Plugins\x1b[0m");
        println!(
            "  Built-in: {}, External: {}",
            builtin_count, external_count
        );

        if show_all || builtin_count > 0 {
            println!("🔧 \x1b[1;33mBuilt-in Plugins:\x1b[0m");
            for plugin_info in all_plugins
                .iter()
                .filter(|p| p.plugin_type == PluginType::Builtin)
            {
                self.print_plugin_summary(plugin_info);
            }
        }

        if show_all || external_count > 0 {
            println!("🔌 \x1b[1;35mExternal Plugins:\x1b[0m");
            for plugin_info in all_plugins
                .iter()
                .filter(|p| p.plugin_type == PluginType::External)
            {
                self.print_plugin_summary(plugin_info);
                if let Some(entry) = self.config.get_external_plugin(&plugin_info.name) {
                    let status_icon = if entry.enabled { "✅" } else { "❌" };
                    println!(
                        "    Status: {} {}",
                        status_icon,
                        if entry.enabled { "Enabled" } else { "Disabled" }
                    );
                }
            }

            if external_count == 0 {
                println!("  No external plugins installed.");
                println!("  Use 'wasmrun plugin search <query>' to find plugins.");
                println!("  Use 'wasmrun plugin install <plugin>' to install.");
            }
        }

        Ok(())
    }

    pub fn install(&mut self, plugin_spec: &str, version: Option<String>) -> Result<()> {
        let plugin_source = self.parse_plugin_source(plugin_spec, version)?;
        let plugin_name = self.extract_plugin_name(plugin_spec, &plugin_source);

        if self.config.is_external_plugin_installed(&plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is already installed. Use 'wasmrun plugin update {}' to update it.",
                plugin_name, plugin_name
            )));
        }

        println!("📦 Installing plugin: {}", plugin_name);

        // Install plugin
        ExternalPluginInstaller::install(&plugin_source, &plugin_name, false)?;

        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(&plugin_name);
        let plugin_info = detect_plugin_metadata(&plugin_dir, &plugin_name, &plugin_source)?;

        // Add to config
        self.config.add_external_plugin(
            plugin_name.clone(),
            plugin_info.clone(),
            plugin_source,
            plugin_name.clone(),
        )?;

        println!(
            "✅ Plugin '{}' installed and enabled successfully",
            plugin_name
        );
        self.print_plugin_info(&plugin_info)?;

        Ok(())
    }

    pub fn uninstall(&mut self, plugin_name: &str) -> Result<()> {
        if !self.config.is_external_plugin_installed(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not installed",
                plugin_name
            )));
        }

        println!("🗑️  Uninstalling plugin: {}", plugin_name);

        // Remove from filesystem
        ExternalPluginInstaller::uninstall(plugin_name)?;

        // Remove from config
        self.config.remove_external_plugin(plugin_name)?;

        println!("✅ Plugin '{}' uninstalled successfully", plugin_name);
        Ok(())
    }

    pub fn update(&mut self, plugin_name: &str) -> Result<()> {
        let entry = self
            .config
            .get_external_plugin(plugin_name)
            .ok_or_else(|| {
                WasmrunError::from(format!("Plugin '{}' is not installed", plugin_name))
            })?
            .clone();

        println!("🔄 Updating plugin: {}", plugin_name);

        // Reinstall with force flag
        ExternalPluginInstaller::install(&entry.source, plugin_name, true)?;

        // Update metadata
        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(&entry.install_path);
        let updated_info = detect_plugin_metadata(&plugin_dir, plugin_name, &entry.source)?;

        self.config
            .update_external_plugin_metadata(plugin_name, updated_info.clone())?;

        println!("✅ Plugin '{}' updated successfully", plugin_name);
        self.print_plugin_info(&updated_info)?;

        Ok(())
    }

    pub fn update_all(&mut self) -> Result<()> {
        let plugin_names: Vec<String> = self
            .config
            .get_external_plugins()
            .iter()
            .map(|info| info.name.clone())
            .collect();

        if plugin_names.is_empty() {
            println!("No external plugins to update.");
            return Ok(());
        }

        println!("🔄 Updating {} external plugins...", plugin_names.len());

        let mut updated_count = 0;
        let mut failed_count = 0;

        for plugin_name in plugin_names {
            match self.update(&plugin_name) {
                Ok(()) => {
                    println!("✅ Updated: {}", plugin_name);
                    updated_count += 1;
                }
                Err(e) => {
                    eprintln!("❌ Failed to update {}: {}", plugin_name, e);
                    failed_count += 1;
                }
            }
        }

        println!("🎉 Update process completed!");
        println!("  ✅ Successfully updated: {}", updated_count);
        if failed_count > 0 {
            println!("  ❌ Failed to update: {}", failed_count);
        }

        Ok(())
    }

    pub fn set_enabled(&mut self, plugin_name: &str, enabled: bool) -> Result<()> {
        if !self.config.is_external_plugin_installed(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not installed",
                plugin_name
            )));
        }

        self.config
            .set_external_plugin_enabled(plugin_name, enabled)?;

        let action = if enabled { "enabled" } else { "disabled" };
        println!("✅ Plugin '{}' {} successfully", plugin_name, action);

        if enabled {
            println!("💡 Restart wasmrun or reload your project to use the enabled plugin.");
        }

        Ok(())
    }

    pub fn info(&self, plugin_name: &str) -> Result<()> {
        // Check built-in plugins first
        if let Some(plugin) = self.manager.get_plugin_by_name(plugin_name) {
            return self.print_plugin_info(plugin.info());
        }

        // Check external plugins in config
        if let Some(entry) = self.config.get_external_plugin(plugin_name) {
            return self.print_plugin_info(&entry.info);
        }

        Err(WasmrunError::from(format!(
            "Plugin '{}' not found. Use 'wasmrun plugin list' to see installed plugins.",
            plugin_name
        )))
    }

    pub fn search(&self, query: &str) -> Result<()> {
        println!("🔍 Searching for plugins matching: '{}'", query);

        let results = self.registry_manager.search_all(query)?;

        if results.is_empty() {
            println!("No plugins found matching '{}'", query);
            println!("💡 Tips:");
            println!("  • Try broader search terms");
            println!("  • Check plugin names or descriptions");
            println!("  • Browse available plugins with 'wasmrun plugin search \"\"'");
            return Ok(());
        }

        println!("Found {} plugins:", results.len());

        for (i, entry) in results.iter().take(10).enumerate() {
            println!(
                "{}. 📦 \x1b[1;36m{}\x1b[0m v{}",
                i + 1,
                entry.info.name,
                entry.info.version
            );
            println!("   {}", entry.info.description);
            println!(
                "   Downloads: {} | Author: {}",
                entry.downloads, entry.info.author
            );

            if let Some(ref source) = entry.info.source {
                match source {
                    PluginSource::CratesIo { name, version: _ } => {
                        println!("   Install: \x1b[32mwasmrun plugin install {}\x1b[0m", name);
                    }
                    PluginSource::Git { url, branch: _ } => {
                        println!("   Install: \x1b[32mwasmrun plugin install {}\x1b[0m", url);
                    }
                    PluginSource::Local { path } => {
                        println!(
                            "   Install: \x1b[32mwasmrun plugin install {}\x1b[0m",
                            path.display()
                        );
                    }
                }
            }

            if !entry.info.extensions.is_empty() {
                println!("   Languages: {}", entry.info.extensions.join(", "));
            }
        }

        if results.len() > 10 {
            println!("... and {} more results", results.len() - 10);
            println!("Use a more specific search term to narrow results.");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> Result<()> {
        let all_plugins = self.manager.list_plugins();
        let builtin_count = all_plugins
            .iter()
            .filter(|p| p.plugin_type == PluginType::Builtin)
            .count();

        let external_plugins = self.config.get_external_plugins();
        let external_installed = external_plugins.len();
        let external_enabled = external_plugins
            .iter()
            .filter(|info| {
                if let Some(entry) = self.config.get_external_plugin(&info.name) {
                    entry.enabled
                } else {
                    false
                }
            })
            .count();
        let external_disabled = external_installed - external_enabled;

        let supported_languages: Vec<String> =
            all_plugins.iter().map(|info| info.name.clone()).collect();

        println!("📊 \x1b[1;36mPlugin Statistics:\x1b[0m");
        println!("  Built-in plugins: {}", builtin_count);
        println!("  External plugins installed: {}", external_installed);
        println!("  External plugins enabled: {}", external_enabled);
        println!("  External plugins disabled: {}", external_disabled);
        println!(
            "  Total active plugins: {}",
            builtin_count + external_enabled
        );

        if !supported_languages.is_empty() {
            println!("🌐 \x1b[1;36mSupported Languages:\x1b[0m");
            for (i, lang) in supported_languages.iter().enumerate() {
                if i > 0 && i % 4 == 0 {}
                print!("  {:<12}", lang);
            }
        }

        // Show capabilities summary
        let mut capabilities_count = std::collections::HashMap::new();
        for plugin in &all_plugins {
            let caps = &plugin.capabilities;
            if caps.compile_wasm {
                *capabilities_count.entry("WASM Compilation").or_insert(0) += 1;
            }
            if caps.compile_webapp {
                *capabilities_count.entry("Web Apps").or_insert(0) += 1;
            }
            if caps.live_reload {
                *capabilities_count.entry("Live Reload").or_insert(0) += 1;
            }
            if caps.optimization {
                *capabilities_count.entry("Optimization").or_insert(0) += 1;
            }
        }

        if !capabilities_count.is_empty() {
            println!("🚀 \x1b[1;36mPlugin Capabilities:\x1b[0m");
            for (capability, count) in capabilities_count {
                println!("  {}: {} plugins", capability, count);
            }
        }

        Ok(())
    }

    fn parse_plugin_source(
        &self,
        plugin_spec: &str,
        version: Option<String>,
    ) -> Result<PluginSource> {
        if plugin_spec.starts_with("http://") || plugin_spec.starts_with("https://") {
            Ok(PluginSource::Git {
                url: plugin_spec.to_string(),
                branch: None,
            })
        } else if plugin_spec.starts_with("git+") {
            let url = plugin_spec.strip_prefix("git+").unwrap().to_string();
            Ok(PluginSource::Git { url, branch: None })
        } else if std::path::Path::new(plugin_spec).exists() {
            Ok(PluginSource::Local {
                path: std::path::PathBuf::from(plugin_spec),
            })
        } else {
            Ok(PluginSource::CratesIo {
                name: plugin_spec.to_string(),
                version: version.unwrap_or_else(|| "*".to_string()),
            })
        }
    }

    fn extract_plugin_name(&self, plugin_spec: &str, source: &PluginSource) -> String {
        match source {
            PluginSource::CratesIo { name, version: _ } => name.clone(),
            PluginSource::Git { url, branch: _ } => {
                // Extract name from git URL
                if let Some(name) = url.split('/').last() {
                    name.strip_suffix(".git").unwrap_or(name).to_string()
                } else {
                    plugin_spec.to_string()
                }
            }
            PluginSource::Local { path } => {
                // Extract name from path
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(plugin_spec)
                    .to_string()
            }
        }
    }

    fn print_plugin_summary(&self, plugin_info: &PluginInfo) {
        let type_icon = match plugin_info.plugin_type {
            PluginType::Builtin => "🔧",
            PluginType::External => "🔌",
        };

        println!(
            "  {} \x1b[1;37m{}\x1b[0m v{} - {}",
            type_icon, plugin_info.name, plugin_info.version, plugin_info.description
        );

        if !plugin_info.extensions.is_empty() {
            println!("    Extensions: {}", plugin_info.extensions.join(", "));
        }

        let caps = &plugin_info.capabilities;
        let mut indicators = Vec::new();
        if caps.compile_wasm {
            indicators.push("WASM");
        }
        if caps.compile_webapp {
            indicators.push("WebApp");
        }
        if caps.live_reload {
            indicators.push("LiveReload");
        }
        if caps.optimization {
            indicators.push("Optimization");
        }

        if !indicators.is_empty() {
            println!("    Features: {}", indicators.join(", "));
        }
    }

    fn print_plugin_info(&self, plugin_info: &PluginInfo) -> Result<()> {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!(
            "\x1b[1;34m│\x1b[0m  📦 \x1b[1;36mPlugin Information: {:<42}\x1b[0m \x1b[1;34m│\x1b[0m",
            plugin_info.name
        );
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );

        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mName:\x1b[0m {:<55} \x1b[1;34m│\x1b[0m",
            plugin_info.name
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mVersion:\x1b[0m {:<52} \x1b[1;34m│\x1b[0m",
            plugin_info.version
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mAuthor:\x1b[0m {:<53} \x1b[1;34m│\x1b[0m",
            plugin_info.author
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mType:\x1b[0m {:<55} \x1b[1;34m│\x1b[0m",
            match plugin_info.plugin_type {
                PluginType::Builtin => "Built-in",
                PluginType::External => "External",
            }
        );

        if !plugin_info.description.is_empty() {
            let desc = if plugin_info.description.len() > 53 {
                format!("{}...", &plugin_info.description[..50])
            } else {
                plugin_info.description.clone()
            };
            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mDescription:\x1b[0m {:<44} \x1b[1;34m│\x1b[0m",
                desc
            );
        }

        if !plugin_info.extensions.is_empty() {
            let extensions = plugin_info.extensions.join(", ");
            let ext_display = if extensions.len() > 48 {
                format!("{}...", &extensions[..45])
            } else {
                extensions
            };
            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mExtensions:\x1b[0m {:<48} \x1b[1;34m│\x1b[0m",
                ext_display
            );
        }

        if !plugin_info.entry_files.is_empty() {
            let entry_files = plugin_info.entry_files.join(", ");
            let files_display = if entry_files.len() > 47 {
                format!("{}...", &entry_files[..44])
            } else {
                entry_files
            };
            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mEntry Files:\x1b[0m {:<47} \x1b[1;34m│\x1b[0m",
                files_display
            );
        }

        let caps = &plugin_info.capabilities;
        let mut cap_list = Vec::new();
        if caps.compile_wasm {
            cap_list.push("WASM");
        }
        if caps.compile_webapp {
            cap_list.push("WebApp");
        }
        if caps.live_reload {
            cap_list.push("LiveReload");
        }
        if caps.optimization {
            cap_list.push("Optimization");
        }

        if !cap_list.is_empty() {
            let capabilities = cap_list.join(", ");
            let cap_display = if capabilities.len() > 46 {
                format!("{}...", &capabilities[..43])
            } else {
                capabilities
            };
            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mCapabilities:\x1b[0m {:<46} \x1b[1;34m│\x1b[0m",
                cap_display
            );
        }

        if !plugin_info.dependencies.is_empty() {
            let deps = plugin_info.dependencies.join(", ");
            let deps_display = if deps.len() > 46 {
                format!("{}...", &deps[..43])
            } else {
                deps
            };
            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mDependencies:\x1b[0m {:<46} \x1b[1;34m│\x1b[0m",
                deps_display
            );
        }

        if let Some(source) = &plugin_info.source {
            let source_str = match source {
                PluginSource::CratesIo { name, version } => {
                    format!("crates.io: {}@{}", name, version)
                }
                PluginSource::Git { url, branch } => {
                    if let Some(branch) = branch {
                        format!("git: {}#{}", url, branch)
                    } else {
                        format!("git: {}", url)
                    }
                }
                PluginSource::Local { path } => {
                    format!("local: {}", path.display())
                }
            };

            let source_display = if source_str.len() > 53 {
                format!("{}...", &source_str[..50])
            } else {
                source_str
            };

            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mSource:\x1b[0m {:<53} \x1b[1;34m│\x1b[0m",
                source_display
            );
        }

        // External plugin installation status
        if plugin_info.plugin_type == PluginType::External {
            if let Some(entry) = self.config.get_external_plugin(&plugin_info.name) {
                let status = if entry.enabled {
                    "✅ Enabled"
                } else {
                    "❌ Disabled"
                };
                println!(
                    "\x1b[1;34m│\x1b[0m  \x1b[1;37mStatus:\x1b[0m {:<53} \x1b[1;34m│\x1b[0m",
                    status
                );
            }
        }

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
        Ok(())
    }
}
