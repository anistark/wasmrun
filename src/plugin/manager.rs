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
        if let Some(_) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            self.registry_manager
                .local_registry_mut()
                .remove_plugin(plugin)?;
            println!("🗑️ Removed {} from plugin registry", plugin);
        }

        let output = Command::new("cargo")
            .args(&["uninstall", plugin])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo uninstall: {}", e)))?;

        if output.status.success() {
            println!("✅ {} uninstalled successfully!", plugin);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️ Cargo uninstall failed: {}", stderr);
            println!("💡 Plugin may not have been installed via cargo");
        }

        Ok(())
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
        let builtin_plugins = self.manager.list_plugins();
        if let Some(builtin) = builtin_plugins.iter().find(|p| p.name == plugin) {
            println!("📋 Plugin Information: {}\n", plugin);
            println!("Type: Built-in");
            println!("Version: {}", builtin.version);
            println!("Description: {}", builtin.description);
            println!("Author: {}", builtin.author);
            println!("Extensions: {}", builtin.extensions.join(", "));
            println!("Entry files: {}", builtin.entry_files.join(", "));
            println!("Dependencies: {}", builtin.dependencies.join(", "));
            
            println!("\nCapabilities:");
            println!("  • Compile WASM: {}", if builtin.capabilities.compile_wasm { "✅" } else { "❌" });
            println!("  • Web Applications: {}", if builtin.capabilities.compile_webapp { "✅" } else { "❌" });
            println!("  • Live Reload: {}", if builtin.capabilities.live_reload { "✅" } else { "❌" });
            println!("  • Optimization: {}", if builtin.capabilities.optimization { "✅" } else { "❌" });
            
            if !builtin.capabilities.custom_targets.is_empty() {
                println!("  • Targets: {}", builtin.capabilities.custom_targets.join(", "));
            }
            
            return Ok(());
        }

        if let Some(external) = self.registry_manager.local_registry().get_installed_plugin(plugin) {
            let is_available = Self::is_plugin_available(plugin);
            
            println!("📋 Plugin Information: {}\n", plugin);
            println!("Type: External");
            println!("Status: {}", if is_available { "✅ Available" } else { "❌ Not Available" });
            println!("Version: {}", external.info.version);
            println!("Description: {}", external.info.description);
            println!("Author: {}", external.info.author);
            println!("Extensions: {}", external.info.extensions.join(", "));
            println!("Entry files: {}", external.info.entry_files.join(", "));
            println!("Dependencies: {}", external.info.dependencies.join(", "));
            println!("Installed: {}", external.installed_at);
            println!("Enabled: {}", if external.enabled { "✅" } else { "❌" });
            
            if let Some(source) = &external.info.source {
                match source {
                    PluginSource::CratesIo { name, version } => {
                        println!("Source: crates.io/{} ({})", name, version);
                    }
                    PluginSource::Git { url, branch } => {
                        println!("Source: Git {}{}", url, 
                            branch.as_ref().map(|b| format!(" ({})", b)).unwrap_or_default());
                    }
                    PluginSource::Local { path } => {
                        println!("Source: Local ({})", path.display());
                    }
                }
            }

            println!("\nCapabilities:");
            println!("  • Compile WASM: {}", if external.info.capabilities.compile_wasm { "✅" } else { "❌" });
            println!("  • Web Applications: {}", if external.info.capabilities.compile_webapp { "✅" } else { "❌" });
            println!("  • Live Reload: {}", if external.info.capabilities.live_reload { "✅" } else { "❌" });
            println!("  • Optimization: {}", if external.info.capabilities.optimization { "✅" } else { "❌" });
            
            if !external.info.capabilities.custom_targets.is_empty() {
                println!("  • Targets: {}", external.info.capabilities.custom_targets.join(", "));
            }
            
            return Ok(());
        }

        if self.get_auto_detected_plugins().contains(&plugin.to_string()) {
            println!("📋 Plugin Information: {} (Auto-detected)\n", plugin);
            println!("Type: External (auto-detected)");
            println!("Status: ⚡ Available in PATH but not formally installed");
            println!("Version: {}", self.get_actual_plugin_version(plugin));
            println!("\n💡 Run 'wasmrun plugin install {}' to formally register this plugin", plugin);
            return Ok(());
        }

        Err(WasmrunError::from(format!("Plugin '{}' not found", plugin)))
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
        if let Ok(output) = Command::new(plugin_name).arg("--version").output() {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = version_output.split_whitespace().nth(1) {
                    return version.to_string();
                }
            }
        }
        "unknown".to_string()
    }

    fn is_plugin_available(plugin_name: &str) -> bool {
        if let Ok(output) = Command::new(plugin_name).arg("--version").output() {
            if output.status.success() {
                return true;
            }
        }

        if plugin_name == "wasmrust" {
            if let Ok(output) = Command::new(plugin_name).arg("info").output() {
                if output.status.success() {
                    return true;
                }
            }
        }

        if let Ok(home_dir) = std::env::var("HOME") {
            let cargo_bin = format!("{}/.cargo/bin/{}", home_dir, plugin_name);
            if std::path::Path::new(&cargo_bin).exists() {
                return true;
            }
        }

        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            let cargo_bin = format!("{}/bin/{}", cargo_home, plugin_name);
            if std::path::Path::new(&cargo_bin).exists() {
                return true;
            }
        }

        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        if let Ok(output) = Command::new(which_cmd).arg(plugin_name).output() {
            if output.status.success() && !output.stdout.is_empty() {
                return true;
            }
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
