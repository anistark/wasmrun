//! Plugin management commands and operations

use crate::error::{Result, WasmrunError};
use crate::plugin::registry::RegistryManager;
use crate::plugin::{PluginCapabilities, PluginInfo, PluginManager, PluginSource, PluginType};
use std::process::Command;

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
        let external_plugins = self.registry_manager.local_registry().get_installed_plugins();

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
                        println!("    Targets: {}", plugin.capabilities.custom_targets.join(", "));
                    }
                    println!();
                }
            }
            println!();
        }

        if !external_plugins.is_empty() {
            println!("🔌 \x1b[1;36mExternal Plugins:\x1b[0m");
            for plugin_info in &external_plugins {
                let status = if Self::is_plugin_available(&plugin_info.name) {
                    "\x1b[1;32m✓ Available\x1b[0m"
                } else {
                    "\x1b[1;31m✗ Not Available\x1b[0m"
                };

                println!(
                    "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - {} [{}]",
                    plugin_info.name, plugin_info.version, plugin_info.description, status
                );

                if show_all {
                    println!("    Extensions: {}", plugin_info.extensions.join(", "));
                    println!("    Entry files: {}", plugin_info.entry_files.join(", "));
                    if !plugin_info.capabilities.custom_targets.is_empty() {
                        println!("    Targets: {}", plugin_info.capabilities.custom_targets.join(", "));
                    }
                    println!();
                }
            }
            println!();
        }

        let auto_detected = self.get_auto_detected_plugins();
        if !auto_detected.is_empty() {
            println!("🔍 \x1b[1;36mAuto-detected (not formally installed):\x1b[0m");
            for plugin_name in auto_detected {
                if !external_plugins.iter().any(|p| p.name == plugin_name) {
                    let status = "\x1b[1;33m⚡ Auto-detected\x1b[0m";
                    println!(
                        "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - {} [{}]",
                        plugin_name, "unknown", "Available in PATH", status
                    );
                    println!("    💡 Run \x1b[1;37mwasmrun plugin install {}\x1b[0m to formally register", plugin_name);
                }
            }
            println!();
        }

        Ok(())
    }

    pub fn install(&mut self, plugin: &str, version: Option<String>) -> Result<()> {
        if let Some(_) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            if Self::is_plugin_available(plugin) {
                println!("✅ Plugin '{}' is already installed and available", plugin);
                return Ok(());
            }
        }

        match plugin {
            "wasmrust" => self.install_wasmrust(version),
            "wasmgo" => self.install_wasmgo(version),
            _ => self.install_generic_plugin(plugin, version),
        }
    }

    fn install_wasmrust(&mut self, version: Option<String>) -> Result<()> {
        if Self::is_plugin_available("wasmrust") {
            return self.register_wasmrust_plugin();
        }

        let version_to_install = version.unwrap_or_else(|| "latest".to_string());

        let mut cmd = Command::new("cargo");
        cmd.args(&["install", "wasmrust"]);

        if version_to_install != "latest" {
            cmd.args(&["--version", &version_to_install]);
        }

        let output = cmd
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo install: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to install wasmrust: {}",
                stderr
            )));
        }

        println!("✅ wasmrust installed successfully!");
        std::thread::sleep(std::time::Duration::from_millis(100));

        self.register_wasmrust_plugin()
    }

    fn register_wasmrust_plugin(&mut self) -> Result<()> {
        let version = self.get_actual_plugin_version("wasmrust");
        
        // Debug output to see what's happening
        if std::env::var("WASMRUN_DEBUG").is_ok() {
            eprintln!("Debug: Registering wasmrust with version: {}", version);
            eprintln!("Debug: Is wasmrust available? {}", Self::is_plugin_available("wasmrust"));
        }
        
        let plugin_info = PluginInfo {
            name: "wasmrust".to_string(),
            version: version.clone(),
            description: "Rust WebAssembly plugin for Wasmrun".to_string(),
            author: "Kumar Anirudha".to_string(),
            extensions: vec!["rs".to_string()],
            entry_files: vec!["Cargo.toml".to_string()],
            plugin_type: PluginType::External,
            source: Some(PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: version.clone(),
            }),
            dependencies: vec!["cargo".to_string(), "rustc".to_string()],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
            },
        };

        let source = PluginSource::CratesIo {
            name: "wasmrust".to_string(),
            version,
        };

        self.registry_manager
            .local_registry_mut()
            .add_plugin("wasmrust".to_string(), plugin_info, source, "wasmrust".to_string())?;

        println!("✅ wasmrust registered and ready to use");
        Ok(())
    }

    fn install_wasmgo(&mut self, _version: Option<String>) -> Result<()> {
        println!("📦 wasmgo plugin installation coming soon!");
        println!("💡 For now, install manually: cargo install wasmgo");
        Ok(())
    }

    fn install_generic_plugin(&mut self, plugin: &str, version: Option<String>) -> Result<()> {
        println!("📦 Installing {} from crates.io...", plugin);

        let mut cmd = Command::new("cargo");
        cmd.args(&["install", plugin]);

        if let Some(v) = version {
            cmd.args(&["--version", &v]);
        }

        let output = cmd
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo install: {}", e)))?;

        if output.status.success() {
            println!("✅ {} installed successfully!", plugin);
            println!("💡 Plugin auto-detection will find it if it follows wasmrun conventions");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to install {}: {}",
                plugin, stderr
            )));
        }

        Ok(())
    }

    pub fn uninstall(&mut self, plugin: &str) -> Result<()> {
        let mut plugin_was_registered = false;

        if let Some(_) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            plugin_was_registered = true;
        }
        
        let plugin_was_available = Self::is_plugin_available(plugin);

        if !plugin_was_registered && !plugin_was_available {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not installed. Use 'wasmrun plugin list' to see available plugins.",
                plugin
            )));
        }

        println!("🗑️ Uninstalling plugin: {}", plugin);

        if plugin_was_registered {
            self.registry_manager
                .local_registry_mut()
                .remove_plugin(plugin)?;
            println!("✅ Removed {} from plugin registry", plugin);
        }

        if plugin_was_available {
            let cargo_result = Command::new("cargo")
                .args(&["uninstall", plugin])
                .output();

            match cargo_result {
                Ok(output) if output.status.success() => {
                    println!("✅ Uninstalled {} binary via cargo", plugin);
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    
                    if stderr.contains("did not match any packages") {
                        println!("⚠️ Binary not found via cargo uninstall (may have been installed differently)");
                        
                        if self.try_manual_binary_removal(plugin) {
                            println!("✅ Removed {} binary manually", plugin);
                        } else {
                            println!("💡 Binary may still exist in PATH. Remove manually if needed.");
                        }
                    } else {
                        println!("⚠️ Cargo uninstall failed: {}", stderr.trim());
                    }
                }
                Err(e) => {
                    println!("⚠️ Failed to run cargo uninstall: {}", e);
                    
                    if self.try_manual_binary_removal(plugin) {
                        println!("✅ Removed {} binary manually", plugin);
                    }
                }
            }
        }

        if let Ok(plugin_dir) = crate::plugin::config::WasmrunConfig::plugin_dir() {
            let plugin_path = plugin_dir.join(plugin);
            if plugin_path.exists() {
                match std::fs::remove_dir_all(&plugin_path) {
                    Ok(()) => println!("✅ Cleaned up plugin directory: {}", plugin_path.display()),
                    Err(e) => println!("⚠️ Failed to remove plugin directory: {}", e),
                }
            }
        }

        let still_available = Self::is_plugin_available(plugin);
        if still_available {
            println!("\n⚠️ Plugin binary may still be available in PATH");
            println!("   You may need to manually remove it from:");
            if let Ok(home) = std::env::var("HOME") {
                println!("   • {}/.cargo/bin/{}", home, plugin);
            }
            if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
                println!("   • {}/bin/{}", cargo_home, plugin);
            }
        } else {
            println!("✅ Plugin '{}' completely uninstalled", plugin);
        }

        Ok(())
    }

    fn try_manual_binary_removal(&self, plugin: &str) -> bool {
        let possible_paths = [
            std::env::var("HOME").ok().map(|home| format!("{}/.cargo/bin/{}", home, plugin)),
            std::env::var("CARGO_HOME").ok().map(|cargo_home| format!("{}/bin/{}", cargo_home, plugin)),
        ];

        for path_opt in &possible_paths {
            if let Some(path) = path_opt {
                let path_buf = std::path::Path::new(path);
                if path_buf.exists() {
                    match std::fs::remove_file(path_buf) {
                        Ok(()) => {
                            println!("🗑️ Removed binary: {}", path);
                            return true;
                        }
                        Err(e) => {
                            println!("⚠️ Failed to remove {}: {}", path, e);
                        }
                    }
                }
            }
        }

        false
    }

    pub fn update(&mut self, plugin: &str) -> Result<()> {
        println!("🔄 Updating plugin: {}", plugin);

        let current_version = if let Some(entry) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            entry.info.version.clone()
        } else {
            "unknown".to_string()
        };

        match plugin {
            "wasmrust" => {
                let mut cmd = Command::new("cargo");
                cmd.args(&["install", "wasmrust", "--force"]);
                
                let output = cmd.output()
                    .map_err(|e| WasmrunError::from(format!("Failed to update wasmrust: {}", e)))?;

                if output.status.success() {
                    self.register_wasmrust_plugin()?;
                    let new_version = self.get_actual_plugin_version("wasmrust");
                    if new_version != current_version {
                        println!("✅ {} updated from {} to {}", plugin, current_version, new_version);
                    } else {
                        println!("✅ {} is already up to date ({})", plugin, new_version);
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Failed to update {}: {}", plugin, stderr)));
                }
            }
            _ => {
                let mut cmd = Command::new("cargo");
                cmd.args(&["install", plugin, "--force"]);
                
                let output = cmd.output()
                    .map_err(|e| WasmrunError::from(format!("Failed to update {}: {}", plugin, e)))?;

                if output.status.success() {
                    println!("✅ {} updated successfully!", plugin);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Failed to update {}: {}", plugin, stderr)));
                }
            }
        }

        Ok(())
    }

    pub fn update_all(&mut self) -> Result<()> {
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

        println!("🔄 Updating {} external plugins...", external_plugins.len());

        for plugin in external_plugins {
            if let Err(e) = self.update(&plugin) {
                println!("❌ Failed to update {}: {}", plugin, e);
            }
        }

        Ok(())
    }

    // TODO: Use when plugin enable/disable functionality is fully implemented
    #[allow(dead_code)]
    pub fn set_enabled(&mut self, plugin: &str, enabled: bool) -> Result<()> {
        self.registry_manager
            .local_registry_mut()
            .set_plugin_enabled(plugin, enabled)?;

        let status = if enabled { "enabled" } else { "disabled" };
        println!("✅ Plugin {} {}", plugin, status);
        Ok(())
    }

    pub fn info(&self, plugin: &str) -> Result<()> {
        // Check built-in plugins first
        let builtin_plugins = self.manager.list_plugins();
        if let Some(builtin) = builtin_plugins.iter().find(|p| p.name == plugin) {
            self.print_plugin_info_box(builtin, None, true)?;
            return Ok(());
        }

        // Check external plugins
        if let Some(external) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            self.print_plugin_info_box(&external.info, Some(external), false)?;
            return Ok(());
        }

        // Check auto-detected plugins
        if self.get_auto_detected_plugins().contains(&plugin.to_string()) {
            let info = self.create_auto_detected_plugin_info(plugin);
            self.print_plugin_info_box(&info, None, false)?;
            println!("\n💡 Run \x1b[1;37mwasmrun plugin install {}\x1b[0m to formally register this plugin", plugin);
            return Ok(());
        }

        Err(WasmrunError::from(format!("Plugin '{}' not found", plugin)))
    }

    fn print_plugin_info_box(&self, info: &PluginInfo, external_entry: Option<&crate::plugin::config::ExternalPluginEntry>, is_builtin: bool) -> Result<()> {
        let is_available = if is_builtin { true } else { Self::is_plugin_available(&info.name) };
        let actual_version = if is_builtin { 
            info.version.clone() 
        } else { 
            self.get_actual_plugin_version(&info.name) 
        };
        
        // Box drawing characters
        let top_left = "╭";
        let top_right = "╮";
        let bottom_left = "╰";
        let bottom_right = "╯";
        let horizontal = "─";
        let vertical = "│";
        let junction_right = "├";
        let junction_left = "┤";

        let box_width = 70;
        let content_width = box_width - 4; // Account for borders and padding

        // Header
        println!("\x1b[1;36m{}{}{}\x1b[0m", top_left, horizontal.repeat(box_width - 2), top_right);
        println!("\x1b[1;36m{}\x1b[0m  🔌 \x1b[1;37mPlugin Information: {}\x1b[0m{}\x1b[1;36m{}\x1b[0m", 
            vertical, 
            info.name,
            " ".repeat(content_width.saturating_sub(info.name.len() + 22)),
            vertical
        );
        println!("\x1b[1;36m{}{}{}\x1b[0m", junction_right, horizontal.repeat(box_width - 2), junction_left);

        // Main info section
        self.print_info_row("Type", &format!("{}", if is_builtin { "Built-in" } else { "External" }), box_width);
        
        let status = if is_builtin {
            "\x1b[1;32m✅ Always Available\x1b[0m".to_string()
        } else if is_available {
            "\x1b[1;32m✅ Available\x1b[0m".to_string()
        } else {
            "\x1b[1;31m❌ Not Available\x1b[0m".to_string()
        };
        self.print_info_row("Status", &status, box_width);
        
        let version_display = if actual_version != "unknown" && actual_version != info.version {
            format!("{} \x1b[1;33m(registered: {})\x1b[0m", actual_version, info.version)
        } else {
            actual_version.clone()
        };
        self.print_info_row("Version", &version_display, box_width);
        self.print_info_row("Description", &info.description, box_width);
        self.print_info_row("Author", &info.author, box_width);
        self.print_info_row("Extensions", &info.extensions.join(", "), box_width);
        self.print_info_row("Entry files", &info.entry_files.join(", "), box_width);

        // External plugin specific info
        if let Some(external) = external_entry {
            println!("\x1b[1;36m{}{}{}\x1b[0m", junction_right, horizontal.repeat(box_width - 2), junction_left);
            self.print_info_row("Installed", &external.installed_at, box_width);
            self.print_info_row("Enabled", &format!("{}", if external.enabled { "✅ Yes" } else { "❌ No" }), box_width);
            
            if let Some(source) = &info.source {
                let source_str = match source {
                    PluginSource::CratesIo { name, version } => format!("crates.io/{} ({})", name, version),
                    PluginSource::Git { url, branch } => format!("Git {}{}", url, 
                        branch.as_ref().map(|b| format!(" ({})", b)).unwrap_or_default()),
                    PluginSource::Local { path } => format!("Local ({})", path.display()),
                };
                self.print_info_row("Source", &source_str, box_width);
            }
        }

        // Dependencies section
        if !info.dependencies.is_empty() {
            println!("\x1b[1;36m{}{}{}\x1b[0m", junction_right, horizontal.repeat(box_width - 2), junction_left);
            self.print_info_row("Dependencies", &info.dependencies.join(", "), box_width);
        }

        // Capabilities section
        println!("\x1b[1;36m{}{}{}\x1b[0m", junction_right, horizontal.repeat(box_width - 2), junction_left);
        println!("\x1b[1;36m{}\x1b[0m  \x1b[1;37mCapabilities:\x1b[0m{}\x1b[1;36m{}\x1b[0m", 
            vertical, 
            " ".repeat(content_width.saturating_sub(13)),
            vertical
        );

        let capabilities = [
            ("Compile WASM", info.capabilities.compile_wasm),
            ("Web Applications", info.capabilities.compile_webapp),
            ("Live Reload", info.capabilities.live_reload),
            ("Optimization", info.capabilities.optimization),
        ];

        for (name, enabled) in capabilities {
            let icon = if enabled { "✅" } else { "❌" };
            self.print_info_row(&format!("  • {}", name), icon, box_width);
        }

        if !info.capabilities.custom_targets.is_empty() {
            self.print_info_row("  • Targets", &info.capabilities.custom_targets.join(", "), box_width);
        }

        // Footer
        println!("\x1b[1;36m{}{}{}\x1b[0m", bottom_left, horizontal.repeat(box_width - 2), bottom_right);

        // Status warnings/info
        if !is_builtin && !is_available {
            println!("\n⚠️  \x1b[1;33mIssues detected:\x1b[0m");
            println!("   • Plugin is registered but executable not found");
            println!("   • Try: \x1b[1;37mcargo install {}\x1b[0m", info.name);
            println!("   • Or: \x1b[1;37mwasmrun plugin uninstall {} && wasmrun plugin install {}\x1b[0m", info.name, info.name);
        } else if !is_builtin && actual_version != info.version && actual_version != "unknown" {
            println!("\n⚠️  \x1b[1;33mVersion mismatch detected:\x1b[0m");
            println!("   • Try: \x1b[1;37mwasmrun plugin update {}\x1b[0m", info.name);
        }

        Ok(())
    }

    fn print_info_row(&self, label: &str, value: &str, box_width: usize) {
        let vertical = "│";
        let content_width = box_width - 4; // Account for borders and padding
        
        // Handle multi-line values
        let wrapped_lines = if value.len() > content_width - label.len() - 3 {
            let max_value_width = content_width - label.len() - 3;
            let mut lines = Vec::new();
            let mut current_line = String::new();
            
            for word in value.split_whitespace() {
                if current_line.len() + word.len() + 1 > max_value_width {
                    if !current_line.is_empty() {
                        lines.push(current_line);
                        current_line = String::new();
                    }
                }
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            lines
        } else {
            vec![value.to_string()]
        };

        for (i, line) in wrapped_lines.iter().enumerate() {
            if i == 0 {
                // First line with label
                let padding = content_width - label.len() - line.len() - 1;
                // Count ANSI escape sequences to adjust padding
                let ansi_len = line.matches('\x1b').count() * 10; // Approximate ANSI sequence length
                let adjusted_padding = padding.saturating_add(ansi_len);
                
                println!("\x1b[1;36m{}\x1b[0m  \x1b[1;34m{:<width$}\x1b[0m {}{}\x1b[1;36m{}\x1b[0m", 
                    vertical, 
                    format!("{}:", label),
                    line,
                    " ".repeat(adjusted_padding),
                    vertical,
                    width = 20
                );
            } else {
                // Continuation lines
                let padding = content_width - 21 - line.len();
                let ansi_len = line.matches('\x1b').count() * 10;
                let adjusted_padding = padding.saturating_add(ansi_len);
                
                println!("\x1b[1;36m{}\x1b[0m  {:<21} {}{}\x1b[1;36m{}\x1b[0m", 
                    vertical, 
                    "",
                    line,
                    " ".repeat(adjusted_padding),
                    vertical
                );
            }
        }
    }

    fn create_auto_detected_plugin_info(&self, plugin: &str) -> PluginInfo {
        let version = self.get_actual_plugin_version(plugin);
        PluginInfo {
            name: plugin.to_string(),
            version,
            description: format!("{} plugin (auto-detected)", plugin),
            author: "Unknown".to_string(),
            extensions: match plugin {
                "wasmrust" => vec!["rs".to_string()],
                "wasmgo" => vec!["go".to_string()],
                _ => vec![],
            },
            entry_files: match plugin {
                "wasmrust" => vec!["Cargo.toml".to_string()],
                "wasmgo" => vec!["go.mod".to_string()],
                _ => vec![],
            },
            plugin_type: PluginType::External,
            source: Some(PluginSource::CratesIo {
                name: plugin.to_string(),
                version: "auto-detected".to_string(),
            }),
            dependencies: vec![],
            capabilities: PluginCapabilities::default(),
        }
    }

    pub fn search(&self, query: &str) -> Result<()> {
        println!("🔍 Searching for plugins matching '{}'...\n", query);

        let query_lower = query.to_lowercase();
        let mut found_any = false;

        let builtin_plugins = self.manager.list_plugins();
        let matching_builtin: Vec<_> = builtin_plugins.iter()
            .filter(|p| p.name.to_lowercase().contains(&query_lower) 
                     || p.description.to_lowercase().contains(&query_lower))
            .collect();

        if !matching_builtin.is_empty() {
            found_any = true;
            println!("🔧 Built-in plugins:");
            for plugin in matching_builtin {
                println!("   • {} v{} - {}", plugin.name, plugin.version, plugin.description);
            }
            println!();
        }

        let external_plugins = self.registry_manager.local_registry().get_installed_plugins();
        let matching_external: Vec<_> = external_plugins.iter()
            .filter(|p| p.name.to_lowercase().contains(&query_lower) 
                     || p.description.to_lowercase().contains(&query_lower))
            .collect();

        if !matching_external.is_empty() {
            found_any = true;
            println!("🔌 External plugins:");
            for plugin in matching_external {
                let status = if Self::is_plugin_available(&plugin.name) { "✅" } else { "❌" };
                println!("   • {} v{} - {} [{}]", plugin.name, plugin.version, plugin.description, status);
            }
            println!();
        }

        let known_plugins = [
            ("wasmrust", "Rust WebAssembly plugin for Wasmrun"),
            ("wasmgo", "Go WebAssembly plugin for Wasmrun"),
        ];

        let matching_known: Vec<_> = known_plugins.iter()
            .filter(|(name, desc)| name.to_lowercase().contains(&query_lower) 
                                || desc.to_lowercase().contains(&query_lower))
            .filter(|(name, _)| !external_plugins.iter().any(|p| p.name == *name))
            .collect();

        if !matching_known.is_empty() {
            found_any = true;
            println!("📦 Available for installation:");
            for (name, desc) in matching_known {
                let available = Self::is_plugin_available(name);
                let status = if available { "⚡ Auto-detected" } else { "📦 Not installed" };
                println!("   • {} - {} [{}]", name, desc, status);
                if available {
                    println!("     💡 Run 'wasmrun plugin install {}' to register", name);
                } else {
                    println!("     💡 Run 'wasmrun plugin install {}' to install", name);
                }
            }
        }

        if !found_any {
            println!("No plugins found matching '{}'", query);
            println!("\n💡 Available commands:");
            println!("   • wasmrun plugin list - Show all plugins");
            println!("   • wasmrun plugin install <name> - Install a plugin");
        }

        Ok(())
    }

    // TODO: Use when advanced debugging features are needed
    #[allow(dead_code)]
    pub fn debug(&self, plugin: Option<&str>) -> Result<()> {
        match plugin {
            Some(name) => self.debug_plugin(name),
            None => self.debug_all(),
        }
    }

    #[allow(dead_code)]
    fn debug_plugin(&self, plugin: &str) -> Result<()> {
        println!("🐛 Debug information for plugin: {}\n", plugin);

        let builtin_plugins = self.manager.list_plugins();
        if let Some(builtin) = builtin_plugins.iter().find(|p| p.name == plugin) {
            println!("📋 Plugin Type: Built-in");
            println!("📍 Status: Always available");
            println!("🔧 Implementation: Compiled into wasmrun binary");
            println!("📦 Version: {}", builtin.version);
            return Ok(());
        }

        if let Some(external) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            println!("📋 Plugin Type: External (registered)");
            println!("📦 Version: {}", external.info.version);
            println!("📁 Install path: {}", external.install_path);
            println!("⚡ Enabled: {}", external.enabled);
            println!("📅 Installed at: {}", external.installed_at);
            
            let is_available = Self::is_plugin_available(plugin);
            println!("🔍 Executable available: {}", is_available);
            
            if !is_available {
                println!("\n🚨 Issues detected:");
                println!("   • Plugin is registered but executable not found");
                println!("   • Possible solutions:");
                println!("     - Run: cargo install {}", plugin);
                println!("     - Check PATH includes ~/.cargo/bin");
                println!("     - Run: wasmrun plugin uninstall {} && wasmrun plugin install {}", plugin, plugin);
            } else {
                let actual_version = self.get_actual_plugin_version(plugin);
                if actual_version != external.info.version && actual_version != "unknown" {
                    println!("\n⚠️ Version mismatch:");
                    println!("   • Registered version: {}", external.info.version);
                    println!("   • Actual version: {}", actual_version);
                    println!("   • Run: wasmrun plugin update {}", plugin);
                }
            }

            return Ok(());
        }

        if Self::is_plugin_available(plugin) {
            println!("📋 Plugin Type: External (auto-detected)");
            println!("📦 Version: {}", self.get_actual_plugin_version(plugin));
            println!("📍 Status: Available in PATH but not registered");
            println!("\n💡 Run 'wasmrun plugin install {}' to register", plugin);
            return Ok(());
        }

        println!("❌ Plugin '{}' not found", plugin);
        println!("\n🔍 Suggestions:");
        println!("   • Check spelling: wasmrun plugin list");
        println!("   • Install if needed: wasmrun plugin install {}", plugin);
        println!("   • Search available: wasmrun plugin search {}", plugin);

        Ok(())
    }

    #[allow(dead_code)]
    fn debug_all(&self) -> Result<()> {
        println!("🐛 Debug information for plugin system\n");

        match crate::plugin::config::WasmrunConfig::config_path() {
            Ok(config_path) => {
                println!("📁 Config file: {}", config_path.display());
                println!("📄 Config exists: {}", config_path.exists());
            }
            Err(e) => println!("❌ Config path error: {}", e),
        }

        match crate::plugin::config::WasmrunConfig::plugin_dir() {
            Ok(plugin_dir) => {
                println!("📁 Plugin directory: {}", plugin_dir.display());
                println!("📄 Directory exists: {}", plugin_dir.exists());
            }
            Err(e) => println!("❌ Plugin directory error: {}", e),
        }

        println!();

        let builtin_plugins = self.manager.list_plugins();
        println!("🔧 Built-in plugins: {}", builtin_plugins.len());
        for plugin in &builtin_plugins {
            println!("   • {} v{}", plugin.name, plugin.version);
        }

        println!();

        let external_plugins = self.registry_manager.local_registry().get_installed_plugins();
        println!("🔌 External plugins (registered): {}", external_plugins.len());
        for plugin in &external_plugins {
            let available = Self::is_plugin_available(&plugin.name);
            let status = if available { "✅" } else { "❌" };
            println!("   • {} v{} {}", plugin.name, plugin.version, status);
        }

        println!();

        let auto_detected = self.get_auto_detected_plugins();
        let unregistered: Vec<_> = auto_detected.iter()
            .filter(|name| !external_plugins.iter().any(|p| &p.name == *name))
            .collect();

        println!("🔍 Auto-detected (unregistered): {}", unregistered.len());
        for plugin in &unregistered {
            let version = self.get_actual_plugin_version(plugin);
            println!("   • {} v{} (run 'wasmrun plugin install {}' to register)", plugin, version, plugin);
        }

        println!();

        if let Ok(path) = std::env::var("PATH") {
            let cargo_bin_in_path = path.contains("/.cargo/bin");
            println!("🛣️ PATH includes ~/.cargo/bin: {}", if cargo_bin_in_path { "✅" } else { "❌" });
            
            if !cargo_bin_in_path {
                println!("   ⚠️ Add ~/.cargo/bin to PATH to use cargo-installed plugins");
            }
        }

        let cargo_available = std::process::Command::new("cargo")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        println!("📦 Cargo available: {}", if cargo_available { "✅" } else { "❌" });

        Ok(())
    }

    fn get_actual_plugin_version(&self, plugin_name: &str) -> String {
        let version_commands = [
            vec!["--version"],
            vec!["-V"],
            vec!["version"],
        ];

        for args in &version_commands {
            if let Ok(output) = Command::new(plugin_name).args(args).output() {
                if output.status.success() {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    let trimmed = version_output.trim();
                    
                    if std::env::var("WASMRUN_DEBUG").is_ok() {
                        eprintln!("Debug: {} {} output: '{}'", plugin_name, args.join(" "), trimmed);
                    }
                    
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    if words.len() >= 2 {
                        for i in 1..words.len() {
                            let potential_version = words[i];
                            if Self::is_valid_version(potential_version) {
                                return potential_version.to_string();
                            }
                        }
                    }
                    
                    for word in words {
                        let clean_word = word.trim_start_matches('v').trim_start_matches("version");
                        if Self::is_valid_version(clean_word) {
                            return clean_word.to_string();
                        }
                        
                        if let Some(dash_pos) = word.find('-') {
                            let after_dash = &word[dash_pos + 1..];
                            if Self::is_valid_version(after_dash) {
                                return after_dash.to_string();
                            }
                        }
                    }
                    
                    if !trimmed.is_empty() && std::env::var("WASMRUN_DEBUG").is_ok() {
                        eprintln!("Debug: Could not parse version from: '{}'", trimmed);
                    }
                }
            }
        }
        
        "unknown".to_string()
    }

    fn is_valid_version(s: &str) -> bool {
        if s.is_empty() || !s.chars().next().unwrap_or('x').is_ascii_digit() || !s.contains('.') {
            return false;
        }
        
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() < 2 {
            return false;
        }
        
        for (i, part) in parts.iter().enumerate() {
            let clean_part = if i == parts.len() - 1 {
                part.split(&['-', '+'][..]).next().unwrap_or(part)
            } else {
                part
            };
            
            if clean_part.parse::<u32>().is_err() {
                return false;
            }
        }
        
        true
    }

    fn is_plugin_available(plugin_name: &str) -> bool {
        let debug = std::env::var("WASMRUN_DEBUG").is_ok();
        
        if debug {
            eprintln!("Debug: Checking if {} is available...", plugin_name);
        }

        // Try --version command first
        if let Ok(output) = Command::new(plugin_name).arg("--version").output() {
            if output.status.success() {
                if debug {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    eprintln!("Debug: {} --version succeeded: '{}'", plugin_name, version_output.trim());
                }
                return true;
            } else if debug {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Debug: {} --version failed: {}", plugin_name, stderr.trim());
            }
        } else if debug {
            eprintln!("Debug: Failed to execute {} --version", plugin_name);
        }

        // For wasmrust, also try 'info' command
        if plugin_name == "wasmrust" {
            if let Ok(output) = Command::new(plugin_name).arg("info").output() {
                if output.status.success() {
                    if debug {
                        eprintln!("Debug: {} info succeeded", plugin_name);
                    }
                    return true;
                } else if debug {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("Debug: {} info failed: {}", plugin_name, stderr.trim());
                }
            }
        }

        // Check common installation paths
        if let Ok(home_dir) = std::env::var("HOME") {
            let cargo_bin = format!("{}/.cargo/bin/{}", home_dir, plugin_name);
            if std::path::Path::new(&cargo_bin).exists() {
                if debug {
                    eprintln!("Debug: Found {} binary at: {}", plugin_name, cargo_bin);
                }
                return true;
            } else if debug {
                eprintln!("Debug: No binary found at: {}", cargo_bin);
            }
        }

        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            let cargo_bin = format!("{}/bin/{}", cargo_home, plugin_name);
            if std::path::Path::new(&cargo_bin).exists() {
                if debug {
                    eprintln!("Debug: Found {} binary at: {}", plugin_name, cargo_bin);
                }
                return true;
            }
        }

        // Use which/where command
        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        if let Ok(output) = Command::new(which_cmd).arg(plugin_name).output() {
            if output.status.success() && !output.stdout.is_empty() {
                if debug {
                    let path = String::from_utf8_lossy(&output.stdout);
                    eprintln!("Debug: Found {} via {}: {}", plugin_name, which_cmd, path.trim());
                }
                return true;
            } else if debug {
                eprintln!("Debug: {} {} returned no results", which_cmd, plugin_name);
            }
        }

        if debug {
            eprintln!("Debug: {} is NOT available", plugin_name);
        }
        false
    }

    fn get_auto_detected_plugins(&self) -> Vec<String> {
        let mut plugins = Vec::new();
        let known_plugins = ["wasmrust", "wasmgo"];

        for plugin in &known_plugins {
            if Self::is_plugin_available(plugin) {
                plugins.push(plugin.to_string());
            }
        }

        plugins
    }
}
