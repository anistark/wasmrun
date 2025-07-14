//! Plugin management commands and operations

use crate::error::{Result, WasmrunError};
use crate::plugin::registry::{detect_plugin_metadata, RegistryManager};
use crate::plugin::{PluginInfo, PluginManager, PluginSource, PluginType};

pub struct PluginCommands {
    manager: PluginManager,
    registry_manager: RegistryManager,
}

impl PluginCommands {
    pub fn new() -> Result<Self> {
        let manager = PluginManager::new()?;
        let registry_manager = RegistryManager::new();
        Ok(Self {
            manager,
            registry_manager,
        })
    }

    pub fn list(&self, show_all: bool) -> Result<()> {
        let builtin_plugins = self.manager.list_plugins();
        let external_plugins = self
            .registry_manager
            .local_registry()
            .get_installed_plugins();

        if builtin_plugins.is_empty() && external_plugins.is_empty() {
            println!("No plugins installed.");
            return Ok(());
        }

        println!("Available plugins:\n");

        if !builtin_plugins.is_empty() {
            println!("🔧 \x1b[1;36mBuilt-in Plugins:\x1b[0m");
            for plugin in &builtin_plugins {
                let status = if plugin.capabilities.compile_wasm {
                    "\x1b[1;32m✓ Ready\x1b[0m"
                } else {
                    "\x1b[1;33m⚠ Limited\x1b[0m"
                };

                println!(
                    "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - {} [{}]",
                    plugin.name, plugin.version, plugin.description, status
                );

                if show_all {
                    println!("    Extensions: {}", plugin.extensions.join(", "));
                    println!("    Entry files: {}", plugin.entry_files.join(", "));
                    if !plugin.capabilities.custom_targets.is_empty() {
                        println!(
                            "    Targets: {}",
                            plugin.capabilities.custom_targets.join(", ")
                        );
                    }
                    println!();
                }
            }
            println!();
        }

        if !external_plugins.is_empty() {
            println!("🔌 \x1b[1;36mExternal Plugins:\x1b[0m");
            for plugin_info in &external_plugins {
                let status = if let Some(plugin_entry) = self
                    .registry_manager
                    .local_registry()
                    .get_installed_plugin(&plugin_info.name)
                {
                    let install_dir = match crate::plugin::config::WasmrunConfig::plugin_dir() {
                        Ok(dir) => dir,
                        Err(_) => {
                            println!(
                                "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - {} [\x1b[1;31m✗ Error\x1b[0m]",
                                plugin_info.name, plugin_info.version, plugin_info.description
                            );
                            continue;
                        }
                    };
                    let plugin_dir = install_dir.join(&plugin_entry.install_path);

                    if !plugin_dir.exists() {
                        "\x1b[1;31m✗ Missing\x1b[0m"
                    } else if !plugin_entry.enabled {
                        "\x1b[1;33m⚠ Disabled\x1b[0m"
                    } else {
                        match check_plugin_status(&plugin_dir) {
                            PluginStatus::Ready => "\x1b[1;32m✓ Ready\x1b[0m",
                            PluginStatus::NotInstalled => "\x1b[1;31m✗ Not Installed\x1b[0m",
                            PluginStatus::AccessError => "\x1b[1;31m✗ Access Error\x1b[0m",
                        }
                    }
                } else {
                    "\x1b[1;31m✗ Registry Error\x1b[0m"
                };

                println!(
                    "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - {} [{}]",
                    plugin_info.name, plugin_info.version, plugin_info.description, status
                );

                if show_all {
                    println!("    Extensions: {}", plugin_info.extensions.join(", "));
                    println!("    Entry files: {}", plugin_info.entry_files.join(", "));
                    if !plugin_info.capabilities.custom_targets.is_empty() {
                        println!(
                            "    Targets: {}",
                            plugin_info.capabilities.custom_targets.join(", ")
                        );
                    }

                    if let Some(plugin_entry) = self
                        .registry_manager
                        .local_registry()
                        .get_installed_plugin(&plugin_info.name)
                    {
                        println!("    Install path: {}", plugin_entry.install_path);
                        if let Ok(install_dir) = crate::plugin::config::WasmrunConfig::plugin_dir()
                        {
                            let full_path = install_dir.join(&plugin_entry.install_path);
                            println!("    Full path: {}", full_path.display());
                        }
                    }
                    println!();
                }
            }
            println!();
        } else {
            println!("🔌 \x1b[1;36mExternal Plugins:\x1b[0m");
            println!("  No external plugins installed.");
            println!("  Use 'wasmrun plugin install <plugin-name>' to install external plugins.");
            println!();
        }

        if !show_all {
            println!("Use 'wasmrun plugin list --all' to see detailed information.");
        }

        Ok(())
    }

    pub fn install(&mut self, plugin_spec: &str, version: Option<String>) -> Result<()> {
        println!("Installing plugin: {}", plugin_spec);

        let source = self.parse_plugin_source(plugin_spec, version)?;

        if self
            .registry_manager
            .local_registry()
            .is_installed(plugin_spec)
        {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is already installed. Use 'wasmrun plugin update {}' to update it.",
                plugin_spec, plugin_spec
            )));
        }

        if self.manager.get_plugin_by_name(plugin_spec).is_some() {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is a built-in plugin and cannot be installed",
                plugin_spec
            )));
        }

        let plugin_dir = crate::plugin::external::PluginInstaller::install(source.clone())?;
        let plugin_info = detect_plugin_metadata(&plugin_dir, plugin_spec, &source)?;

        self.registry_manager.local_registry_mut().add_plugin(
            plugin_spec.to_string(),
            plugin_info,
            source,
            plugin_dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        )?;

        println!("✅ Plugin '{}' installed successfully!", plugin_spec);
        println!("   Installed to: {}", plugin_dir.display());
        Ok(())
    }

    pub fn uninstall(&mut self, plugin_name: &str) -> Result<()> {
        println!("Uninstalling plugin: {}", plugin_name);

        if let Some(plugin) = self.manager.get_plugin_by_name(plugin_name) {
            if plugin.info().plugin_type == PluginType::Builtin {
                return Err(WasmrunError::from(format!(
                    "Cannot uninstall built-in plugin: {}",
                    plugin_name
                )));
            }
        }

        if !self
            .registry_manager
            .local_registry()
            .is_installed(plugin_name)
        {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not installed",
                plugin_name
            )));
        }

        crate::plugin::external::PluginInstaller::uninstall(plugin_name)?;
        self.registry_manager
            .local_registry_mut()
            .remove_plugin(plugin_name)?;

        println!("✅ Plugin '{}' uninstalled successfully!", plugin_name);
        Ok(())
    }

    pub fn info(&self, plugin_name: &str) -> Result<()> {
        if let Some(plugin_info) = self.manager.get_plugin_info(plugin_name) {
            self.print_plugin_info(plugin_info)?;
            return Ok(());
        }

        if let Some(entry) = self
            .registry_manager
            .local_registry()
            .get_installed_plugin(plugin_name)
        {
            self.print_plugin_info(&entry.info)?;
            return Ok(());
        }

        Err(WasmrunError::from(format!(
            "Plugin '{}' not found",
            plugin_name
        )))
    }

    pub fn set_enabled(&mut self, plugin_name: &str, enabled: bool) -> Result<()> {
        let action = if enabled { "Enabling" } else { "Disabling" };
        println!("{} plugin: {}", action, plugin_name);

        if let Some(plugin) = self.manager.get_plugin_by_name(plugin_name) {
            if plugin.info().plugin_type == PluginType::Builtin {
                return Err(WasmrunError::from(format!(
                    "Cannot disable built-in plugin: {}",
                    plugin_name
                )));
            }
        }

        self.registry_manager
            .local_registry_mut()
            .set_plugin_enabled(plugin_name, enabled)?;

        let status = if enabled { "enabled" } else { "disabled" };
        println!("✅ Plugin '{}' {} successfully!", plugin_name, status);
        Ok(())
    }

    pub fn update(&mut self, plugin_name: &str) -> Result<()> {
        println!("Updating plugin: {}", plugin_name);

        if let Some(plugin) = self.manager.get_plugin_by_name(plugin_name) {
            if plugin.info().plugin_type == PluginType::Builtin {
                return Err(WasmrunError::from(format!(
                    "Cannot update built-in plugin: {}. Built-in plugins are updated with Wasmrun itself.",
                    plugin_name
                )));
            }
        }

        if !self
            .registry_manager
            .local_registry()
            .is_installed(plugin_name)
        {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not installed",
                plugin_name
            )));
        }

        let entry = self
            .registry_manager
            .local_registry()
            .get_installed_plugin(plugin_name)
            .unwrap();
        let source = entry.source.clone();

        crate::plugin::external::PluginInstaller::uninstall(plugin_name)?;

        let plugin_dir = crate::plugin::external::PluginInstaller::install(source.clone())?;
        let plugin_info = detect_plugin_metadata(&plugin_dir, plugin_name, &source)?;

        self.registry_manager
            .local_registry_mut()
            .update_plugin_metadata(plugin_name, plugin_info)?;

        println!("✅ Plugin '{}' updated successfully!", plugin_name);
        Ok(())
    }

    pub fn update_all(&mut self) -> Result<()> {
        println!("Updating all external plugins...");

        let external_plugins: Vec<String> = self
            .registry_manager
            .local_registry()
            .get_installed_plugins()
            .iter()
            .map(|p| p.name.clone())
            .collect();

        if external_plugins.is_empty() {
            println!("No external plugins to update.");
            return Ok(());
        }

        let mut updated_count = 0;
        let mut failed_count = 0;

        for plugin_name in external_plugins {
            match self.update(&plugin_name) {
                Ok(_) => {
                    println!("✅ Updated: {}", plugin_name);
                    updated_count += 1;
                }
                Err(e) => {
                    println!("❌ Failed to update {}: {}", plugin_name, e);
                    failed_count += 1;
                }
            }
        }

        println!("\nUpdate summary:");
        println!("  ✅ Updated: {}", updated_count);
        if failed_count > 0 {
            println!("  ❌ Failed: {}", failed_count);
        }

        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<()> {
        println!("🔍 \x1b[1;36mSearching for plugins: {}\x1b[0m\n", query);

        let results = self.registry_manager.search_all(query)?;

        if results.is_empty() {
            println!("No plugins found matching '{}'.", query);
            return Ok(());
        }

        println!("Found {} plugin(s):\n", results.len());

        for (index, result) in results.iter().enumerate() {
            let status = if self.manager.get_plugin_by_name(&result.info.name).is_some() {
                "\x1b[1;32m[Built-in]\x1b[0m"
            } else if self
                .registry_manager
                .local_registry()
                .is_installed(&result.info.name)
            {
                "\x1b[1;33m[Installed]\x1b[0m"
            } else {
                "\x1b[1;37m[Available]\x1b[0m"
            };

            println!(
                "{}. \x1b[1;37m{}\x1b[0m v{} {}",
                index + 1,
                result.info.name,
                result.info.version,
                status
            );
            println!("   {}", result.info.description);

            if !result.info.extensions.is_empty() {
                println!("   Extensions: {}", result.info.extensions.join(", "));
            }

            println!("   Downloads: {}", result.downloads);
            println!();
        }

        let builtin_count = self.manager.list_plugins().len();
        let stats = self.registry_manager.local_registry().get_stats();

        println!("📊 \x1b[1;36mPlugin Statistics:\x1b[0m");
        println!("  Built-in plugins: {}", builtin_count);
        println!("  External plugins installed: {}", stats.total_installed);
        println!("  External plugins enabled: {}", stats.enabled_count);
        println!("  External plugins disabled: {}", stats.disabled_count);

        if !stats.supported_languages.is_empty() {
            println!(
                "  Supported languages: {}",
                stats.supported_languages.join(", ")
            );
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

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
        Ok(())
    }
}

// Plugin status

#[derive(Debug)]
enum PluginStatus {
    Ready,        // Plugin is properly installed and ready
    NotInstalled, // Plugin directory missing or installation failed
    AccessError,  // Cannot access plugin directory
}

fn check_plugin_status(plugin_dir: &std::path::Path) -> PluginStatus {
    // For external plugins, verify they're properly installed
    // Assumption: cargo install creates .crates.toml/.crates2.json on success
    let crates_toml = plugin_dir.join(".crates.toml");
    let crates2_json = plugin_dir.join(".crates2.json");

    if crates_toml.exists() || crates2_json.exists() {
        return PluginStatus::Ready;
    }

    // Check for any plugin artifacts as fallback
    let search_paths = vec![
        plugin_dir.join("bin"),
        plugin_dir.to_path_buf(),
        plugin_dir.join("target").join("release"),
        plugin_dir.join("src"),
    ];

    for search_path in search_paths {
        if !search_path.exists() {
            continue;
        }

        match std::fs::read_dir(&search_path) {
            Ok(entries) => {
                let has_content = entries.filter_map(|entry| entry.ok()).any(|entry| {
                    let path = entry.path();
                    if path.is_file() {
                        is_executable_file(&path)
                            || is_library_file(&path)
                            || path.extension().map_or(false, |ext| {
                                let ext_str = ext.to_string_lossy().to_lowercase();
                                ext_str == "rs" || ext_str == "toml" || ext_str == "md"
                            })
                    } else {
                        false
                    }
                });

                if has_content {
                    return PluginStatus::Ready;
                }
            }
            Err(_) => return PluginStatus::AccessError,
        }
    }

    PluginStatus::NotInstalled
}

fn is_library_file(path: &std::path::Path) -> bool {
    if let Some(extension) = path.extension() {
        let ext_str = extension.to_string_lossy().to_lowercase();
        ext_str == "so" || ext_str == "dll" || ext_str == "dylib"
    } else {
        false
    }
}

#[cfg(unix)]
fn is_executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

#[cfg(windows)]
fn is_executable_file(path: &std::path::Path) -> bool {
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        ext == "exe" || ext == "bat" || ext == "cmd"
    } else {
        false
    }
}

#[cfg(not(any(unix, windows)))]
fn is_executable_file(_path: &std::path::Path) -> bool {
    false
}
