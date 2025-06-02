//! Plugin management commands and operations

use crate::error::{ChakraError, Result};
use crate::plugin::{PluginManager, PluginSource, PluginType};

/// Plugin command handler
pub struct PluginCommands {
    manager: PluginManager,
}

impl PluginCommands {
    /// Create a new plugin command handler
    pub fn new() -> Result<Self> {
        let manager = PluginManager::new()?;
        Ok(Self { manager })
    }

    /// List all available plugins
    pub fn list(&self, show_all: bool) -> Result<()> {
        let plugins = self.manager.list_plugins();

        if plugins.is_empty() {
            println!("No plugins installed.");
            return Ok(());
        }

        println!("Available plugins:\n");

        // Group plugins by type
        let mut builtin_plugins = Vec::new();
        let mut external_plugins = Vec::new();

        for plugin in plugins {
            match plugin.plugin_type {
                PluginType::Builtin => builtin_plugins.push(plugin),
                PluginType::External => external_plugins.push(plugin),
            }
        }

        // Display built-in plugins
        if !builtin_plugins.is_empty() {
            println!("🔧 \x1b[1;36mBuilt-in Plugins:\x1b[0m");
            for plugin in builtin_plugins {
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

        // Display external plugins
        if !external_plugins.is_empty() {
            println!("🔌 \x1b[1;36mExternal Plugins:\x1b[0m");
            for plugin in external_plugins {
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
                    if let Some(source) = &plugin.source {
                        match source {
                            PluginSource::CratesIo { name, version } => {
                                println!("    Source: crates.io ({} v{})", name, version);
                            }
                            PluginSource::Git { url, branch } => {
                                let branch_info = if let Some(b) = branch {
                                    format!(" (branch: {})", b)
                                } else {
                                    String::new()
                                };
                                println!("    Source: Git ({}{})", url, branch_info);
                            }
                            PluginSource::Local { path } => {
                                println!("    Source: Local ({})", path.display());
                            }
                        }
                    }
                    println!("    Extensions: {}", plugin.extensions.join(", "));
                    println!("    Entry files: {}", plugin.entry_files.join(", "));
                    println!();
                }
            }
        } else if show_all {
            println!("🔌 \x1b[1;36mExternal Plugins:\x1b[0m");
            println!("  No external plugins installed.");
            println!("  Use 'chakra plugin install <plugin-name>' to install external plugins.");
            println!();
        }

        if !show_all {
            println!("Use 'chakra plugin list --all' to see detailed information.");
        }

        Ok(())
    }

    /// Install a plugin
    pub fn install(&mut self, plugin_spec: &str, version: Option<String>) -> Result<()> {
        println!("Installing plugin: {}", plugin_spec);

        let source = self.parse_plugin_source(plugin_spec, version)?;

        // Check if plugin is already installed
        if let Some(existing) = self.manager.registry().get_plugin(plugin_spec) {
            if existing.info().plugin_type == PluginType::Builtin {
                return Err(ChakraError::from(format!(
                    "Plugin '{}' is a built-in plugin and cannot be installed",
                    plugin_spec
                )));
            }

            return Err(ChakraError::from(format!(
                "Plugin '{}' is already installed. Use 'chakra plugin update {}' to update it.",
                plugin_spec, plugin_spec
            )));
        }

        // Install the plugin
        self.manager.install_plugin(source)?;

        println!("✅ Plugin '{}' installed successfully!", plugin_spec);
        Ok(())
    }

    /// Uninstall a plugin
    pub fn uninstall(&mut self, plugin_name: &str) -> Result<()> {
        println!("Uninstalling plugin: {}", plugin_name);

        // Check if plugin exists
        let plugin = self
            .manager
            .registry()
            .get_plugin(plugin_name)
            .ok_or_else(|| ChakraError::from(format!("Plugin '{}' not found", plugin_name)))?;

        if plugin.info().plugin_type == PluginType::Builtin {
            return Err(ChakraError::from(format!(
                "Cannot uninstall built-in plugin: {}",
                plugin_name
            )));
        }

        // Uninstall the plugin
        self.manager.uninstall_plugin(plugin_name)?;

        println!("✅ Plugin '{}' uninstalled successfully!", plugin_name);
        Ok(())
    }

    /// Update a plugin
    pub fn update(&mut self, plugin_name: &str) -> Result<()> {
        println!("Updating plugin: {}", plugin_name);

        // Check if plugin exists
        let plugin = self
            .manager
            .registry()
            .get_plugin(plugin_name)
            .ok_or_else(|| ChakraError::from(format!("Plugin '{}' not found", plugin_name)))?;

        if plugin.info().plugin_type == PluginType::Builtin {
            return Err(ChakraError::from(format!(
                "Cannot update built-in plugin: {}. Built-in plugins are updated with Chakra itself.",
                plugin_name
            )));
        }

        // Update the plugin
        self.manager.update_plugin(plugin_name)?;

        println!("✅ Plugin '{}' updated successfully!", plugin_name);
        Ok(())
    }

    /// Enable or disable a plugin
    pub fn set_enabled(&mut self, plugin_name: &str, enabled: bool) -> Result<()> {
        let action = if enabled { "Enabling" } else { "Disabling" };
        println!("{} plugin: {}", action, plugin_name);

        // Check if plugin exists
        let plugin = self
            .manager
            .registry()
            .get_plugin(plugin_name)
            .ok_or_else(|| ChakraError::from(format!("Plugin '{}' not found", plugin_name)))?;

        if plugin.info().plugin_type == PluginType::Builtin {
            return Err(ChakraError::from(format!(
                "Cannot disable built-in plugin: {}",
                plugin_name
            )));
        }

        // Enable/disable the plugin
        self.manager.set_plugin_enabled(plugin_name, enabled)?;

        let status = if enabled { "enabled" } else { "disabled" };
        println!("✅ Plugin '{}' {} successfully!", plugin_name, status);
        Ok(())
    }

    /// Show detailed information about a plugin
    pub fn info(&self, plugin_name: &str) -> Result<()> {
        let plugin = self
            .manager
            .registry()
            .get_plugin(plugin_name)
            .ok_or_else(|| ChakraError::from(format!("Plugin '{}' not found", plugin_name)))?;

        let info = plugin.info();

        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!(
            "\x1b[1;34m│\x1b[0m  📦 \x1b[1;36mPlugin Information: {:<42}\x1b[0m \x1b[1;34m│\x1b[0m",
            info.name
        );
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );

        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mName:\x1b[0m {:<55} \x1b[1;34m│\x1b[0m",
            info.name
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mVersion:\x1b[0m {:<52} \x1b[1;34m│\x1b[0m",
            info.version
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mAuthor:\x1b[0m {:<53} \x1b[1;34m│\x1b[0m",
            info.author
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mType:\x1b[0m {:<55} \x1b[1;34m│\x1b[0m",
            match info.plugin_type {
                PluginType::Builtin => "Built-in",
                PluginType::External => "External",
            }
        );

        // Description (might be long, so wrap it)
        let desc_lines = self.wrap_text(&info.description, 57);
        for (i, line) in desc_lines.iter().enumerate() {
            if i == 0 {
                println!(
                    "\x1b[1;34m│\x1b[0m  \x1b[1;37mDescription:\x1b[0m {:<46} \x1b[1;34m│\x1b[0m",
                    line
                );
            } else {
                println!(
                    "\x1b[1;34m│\x1b[0m               {:<46} \x1b[1;34m│\x1b[0m",
                    line
                );
            }
        }

        // Extensions
        if !info.extensions.is_empty() {
            println!(
                "\x1b[1;34m│\x1b[0m  \x1b[1;37mExtensions:\x1b[0m {:<48} \x1b[1;34m│\x1b[0m",
                info.extensions.join(", ")
            );
        }

        // Entry files
        if !info.entry_files.is_empty() {
            let entry_text = info.entry_files.join(", ");
            let entry_lines = self.wrap_text(&entry_text, 48);
            for (i, line) in entry_lines.iter().enumerate() {
                if i == 0 {
                    println!("\x1b[1;34m│\x1b[0m  \x1b[1;37mEntry Files:\x1b[0m {:<47} \x1b[1;34m│\x1b[0m", line);
                } else {
                    println!(
                        "\x1b[1;34m│\x1b[0m               {:<47} \x1b[1;34m│\x1b[0m",
                        line
                    );
                }
            }
        }

        // Capabilities
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🔧 \x1b[1;36mCapabilities:\x1b[0m                                           \x1b[1;34m│\x1b[0m");

        let check_mark = |enabled: bool| if enabled { "✓" } else { "✗" };
        let color = |enabled: bool| if enabled { "\x1b[1;32m" } else { "\x1b[1;31m" };

        println!("\x1b[1;34m│\x1b[0m    {}{} WASM Compilation\x1b[0m                                    \x1b[1;34m│\x1b[0m", 
                 color(info.capabilities.compile_wasm), check_mark(info.capabilities.compile_wasm));
        println!("\x1b[1;34m│\x1b[0m    {}{} Web Application Support\x1b[0m                            \x1b[1;34m│\x1b[0m", 
                 color(info.capabilities.compile_webapp), check_mark(info.capabilities.compile_webapp));
        println!("\x1b[1;34m│\x1b[0m    {}{} Live Reload\x1b[0m                                         \x1b[1;34m│\x1b[0m", 
                 color(info.capabilities.live_reload), check_mark(info.capabilities.live_reload));
        println!("\x1b[1;34m│\x1b[0m    {}{} Optimization Support\x1b[0m                                \x1b[1;34m│\x1b[0m", 
                 color(info.capabilities.optimization), check_mark(info.capabilities.optimization));

        // Custom targets
        if !info.capabilities.custom_targets.is_empty() {
            println!(
                "\x1b[1;34m│\x1b[0m    \x1b[1;37mTargets:\x1b[0m {:<50} \x1b[1;34m│\x1b[0m",
                info.capabilities.custom_targets.join(", ")
            );
        }

        // Source information for external plugins
        if let Some(source) = &info.source {
            println!("\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m");
            println!("\x1b[1;34m│\x1b[0m  📦 \x1b[1;36mSource Information:\x1b[0m                                     \x1b[1;34m│\x1b[0m");

            match source {
                PluginSource::CratesIo { name, version } => {
                    println!("\x1b[1;34m│\x1b[0m    \x1b[1;37mRegistry:\x1b[0m crates.io                                   \x1b[1;34m│\x1b[0m");
                    println!(
                        "\x1b[1;34m│\x1b[0m    \x1b[1;37mPackage:\x1b[0m {:<50} \x1b[1;34m│\x1b[0m",
                        name
                    );
                    println!(
                        "\x1b[1;34m│\x1b[0m    \x1b[1;37mVersion:\x1b[0m {:<50} \x1b[1;34m│\x1b[0m",
                        version
                    );
                }
                PluginSource::Git { url, branch } => {
                    println!("\x1b[1;34m│\x1b[0m    \x1b[1;37mSource:\x1b[0m Git repository                               \x1b[1;34m│\x1b[0m");
                    let url_lines = self.wrap_text(url, 52);
                    for (i, line) in url_lines.iter().enumerate() {
                        if i == 0 {
                            println!("\x1b[1;34m│\x1b[0m    \x1b[1;37mURL:\x1b[0m {:<54} \x1b[1;34m│\x1b[0m", line);
                        } else {
                            println!("\x1b[1;34m│\x1b[0m         {:<54} \x1b[1;34m│\x1b[0m", line);
                        }
                    }
                    if let Some(branch) = branch {
                        println!("\x1b[1;34m│\x1b[0m    \x1b[1;37mBranch:\x1b[0m {:<51} \x1b[1;34m│\x1b[0m", branch);
                    }
                }
                PluginSource::Local { path } => {
                    println!("\x1b[1;34m│\x1b[0m    \x1b[1;37mSource:\x1b[0m Local directory                              \x1b[1;34m│\x1b[0m");
                    let path_str = path.display().to_string();
                    let path_lines = self.wrap_text(&path_str, 53);
                    for (i, line) in path_lines.iter().enumerate() {
                        if i == 0 {
                            println!("\x1b[1;34m│\x1b[0m    \x1b[1;37mPath:\x1b[0m {:<53} \x1b[1;34m│\x1b[0m", line);
                        } else {
                            println!(
                                "\x1b[1;34m│\x1b[0m          {:<53} \x1b[1;34m│\x1b[0m",
                                line
                            );
                        }
                    }
                }
            }
        }

        // Dependencies
        if !info.dependencies.is_empty() {
            println!("\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m");
            println!("\x1b[1;34m│\x1b[0m  🔗 \x1b[1;36mDependencies:\x1b[0m                                           \x1b[1;34m│\x1b[0m");
            for dep in &info.dependencies {
                println!("\x1b[1;34m│\x1b[0m    • {:<56} \x1b[1;34m│\x1b[0m", dep);
            }
        }

        println!("\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m\n");

        Ok(())
    }

    /// Update all external plugins
    pub fn update_all(&mut self) -> Result<()> {
        println!("Updating all external plugins...");

        let external_plugins: Vec<String> = self
            .manager
            .list_plugins()
            .iter()
            .filter(|p| p.plugin_type == PluginType::External)
            .map(|p| p.name.clone())
            .collect();

        if external_plugins.is_empty() {
            println!("No external plugins to update.");
            return Ok(());
        }

        let mut updated_count = 0;
        let mut failed_count = 0;

        for plugin_name in external_plugins {
            match self.manager.update_plugin(&plugin_name) {
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

    /// Search for available plugins (placeholder for future implementation)
    pub fn search(&self, query: &str) -> Result<()> {
        println!("Searching for plugins matching '{}'...", query);
        println!("Plugin search functionality is not yet implemented.");
        println!("You can install plugins directly if you know their name:");
        println!("  chakra plugin install <plugin-name>");
        Ok(())
    }

    /// Parse plugin source from user input
    fn parse_plugin_source(
        &self,
        plugin_spec: &str,
        version: Option<String>,
    ) -> Result<PluginSource> {
        if plugin_spec.starts_with("http://")
            || plugin_spec.starts_with("https://")
            || plugin_spec.starts_with("git://")
        {
            // Git repository
            Ok(PluginSource::Git {
                url: plugin_spec.to_string(),
                branch: None,
            })
        } else if plugin_spec.starts_with("./")
            || plugin_spec.starts_with("/")
            || plugin_spec.contains('/')
        {
            // Local path
            let path = std::path::PathBuf::from(plugin_spec);
            if !path.exists() {
                return Err(ChakraError::from(format!(
                    "Local path does not exist: {}",
                    plugin_spec
                )));
            }
            Ok(PluginSource::Local { path })
        } else {
            // Assume crates.io
            Ok(PluginSource::CratesIo {
                name: plugin_spec.to_string(),
                version: version.unwrap_or_else(|| "*".to_string()),
            })
        }
    }

    /// Wrap text to fit within a specific width
    fn wrap_text(&self, text: &str, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let words: Vec<&str> = text.split_whitespace().collect();

        if words.is_empty() {
            return lines;
        }

        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        lines
    }
}
