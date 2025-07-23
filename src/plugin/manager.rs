//! Plugin management commands and operations

use crate::error::{Result, WasmrunError};
use crate::plugin::registry::RegistryManager;
use crate::plugin::{PluginInfo, PluginManager, PluginSource, PluginType};
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
                        println!(
                            "    Targets: {}",
                            plugin_info.capabilities.custom_targets.join(", ")
                        );
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
        println!("🔄 Installing plugin: {}", plugin);

        // TODO: Remove this once we have a proper plugin registry
        match plugin {
            "wasmrust" => self.install_wasmrust(version),
            "wasmgo" => self.install_wasmgo(version),
            _ => self.install_generic_plugin(plugin, version),
        }
    }

    fn install_wasmrust(&mut self, version: Option<String>) -> Result<()> {
        println!("🔄 Installing wasmrust plugin...");

        if Self::is_plugin_available("wasmrust") {
            println!("✅ wasmrust is already available");
            let actual_version = self.get_actual_wasmrust_version();
            println!("📦 Found wasmrust version: {}", actual_version);
            println!("🔌 Plugin detected and ready to use");
            return Ok(());
        }

        let version_to_install = version.unwrap_or_else(|| "latest".to_string());

        println!("📦 Installing wasmrust from crates.io...");

        let mut cmd = Command::new("cargo");
        cmd.args(&["install", "wasmrust"]);

        if version_to_install != "latest" {
            cmd.args(&["--version", &version_to_install]);
        }

        let output = cmd
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo install: {}", e)))?;

        if output.status.success() {
            println!("✅ wasmrust installed successfully!");

            std::thread::sleep(std::time::Duration::from_millis(500));

            if Self::is_plugin_available("wasmrust") {
                let actual_version = self.get_actual_wasmrust_version();
                println!("📦 Installed version: {}", actual_version);
                println!("✓ Verification: wasmrust is available and ready to use");
            } else {
                println!("⚠️  Installation completed but wasmrust not immediately available");
                self.diagnose_path_issue();
            }

            println!("🔌 Plugin installed and ready to use");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to install wasmrust: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn get_actual_wasmrust_version(&self) -> String {
        if let Ok(output) = Command::new("wasmrust").arg("--version").output() {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = version_output.split_whitespace().nth(1) {
                    return version.to_string();
                }
            }
        }

        if let Ok(home_dir) = std::env::var("HOME") {
            let cargo_bin = format!("{}/.cargo/bin/wasmrust", home_dir);
            if let Ok(output) = Command::new(&cargo_bin).arg("--version").output() {
                if output.status.success() {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    if let Some(version) = version_output.split_whitespace().nth(1) {
                        return version.to_string();
                    }
                }
            }
        }

        if let Ok(output) = Command::new("wasmrust").arg("info").output() {
            if output.status.success() {
                let info_output = String::from_utf8_lossy(&output.stdout);
                for line in info_output.lines() {
                    if line.contains("WasmRust v") {
                        if let Some(version) = line.split("WasmRust v").nth(1) {
                            return version.trim().to_string();
                        }
                    }
                }
            }
        }

        "unknown".to_string()
    }

    fn diagnose_path_issue(&self) {
        println!("🔍 Diagnosing PATH issue...");

        if let Ok(home_dir) = std::env::var("HOME") {
            let cargo_bin_dir = format!("{}/.cargo/bin", home_dir);
            let cargo_bin_path = std::path::Path::new(&cargo_bin_dir);

            if cargo_bin_path.exists() {
                println!("✅ ~/.cargo/bin directory exists");

                let wasmrust_path = format!("{}/wasmrust", cargo_bin_dir);
                if std::path::Path::new(&wasmrust_path).exists() {
                    println!("✅ wasmrust binary found at: {}", wasmrust_path);

                    if let Ok(output) = Command::new(&wasmrust_path).arg("--version").output() {
                        if output.status.success() {
                            let version_output = String::from_utf8_lossy(&output.stdout);
                            println!(
                                "✅ wasmrust executable and version: {}",
                                version_output.trim()
                            );
                        } else {
                            println!("❌ wasmrust binary exists but not executable");
                        }
                    } else {
                        println!("❌ Failed to execute wasmrust binary");
                    }
                } else {
                    println!("❌ wasmrust binary not found in ~/.cargo/bin");

                    if let Ok(entries) = std::fs::read_dir(&cargo_bin_dir) {
                        println!("📁 Contents of ~/.cargo/bin:");
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.contains("wasm") || name.contains("rust") {
                                    println!("   🔍 {}", name);
                                }
                            }
                        }
                    }
                }
            } else {
                println!("❌ ~/.cargo/bin directory not found");
            }
        }

        if let Ok(path) = std::env::var("PATH") {
            let has_cargo_bin = path.contains("/.cargo/bin");
            if has_cargo_bin {
                println!("✅ ~/.cargo/bin is in PATH");
            } else {
                println!("❌ ~/.cargo/bin is NOT in PATH");
                println!("💡 Add this to your shell profile (.bashrc, .zshrc, etc.):");
                println!("   export PATH=\"$HOME/.cargo/bin:$PATH\"");
            }
        }
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
        println!("🗑️  Uninstalling plugin: {}", plugin);

        self.registry_manager
            .local_registry_mut()
            .remove_plugin(plugin)?;

        // Uninstall via cargo
        let output = Command::new("cargo")
            .args(&["uninstall", plugin])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo uninstall: {}", e)))?;

        if output.status.success() {
            println!("✅ {} uninstalled successfully!", plugin);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️  cargo uninstall failed: {}", stderr);
            println!("💡 Plugin removed from wasmrun registry anyway");
        }

        Ok(())
    }

    pub fn info(&self, plugin: &str) -> Result<()> {
        if let Some(plugin_info) = self.manager.get_plugin_info(plugin) {
            self.print_plugin_info(plugin_info)?;
            return Ok(());
        }

        if let Some(plugin_entry) = self
            .registry_manager
            .local_registry()
            .get_installed_plugin(plugin)
        {
            self.print_plugin_info(&plugin_entry.info)?;

            println!();
            println!("🔌 \x1b[1;36mExternal Plugin Details:\x1b[0m");
            println!("  📍 Source: {:?}", plugin_entry.source);
            println!("  📁 Install Path: {}", plugin_entry.install_path);
            println!(
                "  ⚡ Enabled: {}",
                if plugin_entry.enabled { "Yes" } else { "No" }
            );

            let available = Self::is_plugin_available(plugin);
            println!(
                "  🔍 Available: {}",
                if available { "✅ Yes" } else { "❌ No" }
            );

            if !available {
                println!();
                println!("💡 Plugin is registered but not available in PATH.");
                println!("   Try reinstalling: wasmrun plugin install {}", plugin);
            }

            return Ok(());
        }

        if Self::is_plugin_available(plugin) {
            println!("🔍 \x1b[1;36mAuto-detected Plugin: {}\x1b[0m", plugin);
            println!();
            println!("  ⚡ Status: Available in PATH but not formally registered");
            println!(
                "  💡 Run \x1b[1;37mwasmrun plugin install {}\x1b[0m to register and get full info",
                plugin
            );
            return Ok(());
        }

        Err(WasmrunError::from(format!("Plugin '{}' not found", plugin)))
    }

    fn print_plugin_info(&self, plugin_info: &PluginInfo) -> Result<()> {
        println!(
            "\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m"
        );
        println!("\x1b[1;34m│\x1b[0m  🔌 \x1b[1;36mPlugin Information\x1b[0m                                    \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );

        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mName:\x1b[0m {:<56} \x1b[1;34m│\x1b[0m",
            plugin_info.name
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mVersion:\x1b[0m {:<52} \x1b[1;34m│\x1b[0m",
            plugin_info.version
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mDescription:\x1b[0m {:<47} \x1b[1;34m│\x1b[0m",
            self.truncate_string(&plugin_info.description, 47)
        );
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mAuthor:\x1b[0m {:<53} \x1b[1;34m│\x1b[0m",
            plugin_info.author
        );

        let plugin_type = match plugin_info.plugin_type {
            PluginType::Builtin => "Built-in",
            PluginType::External => "External",
        };
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;37mType:\x1b[0m {:<55} \x1b[1;34m│\x1b[0m",
            plugin_type
        );

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
        Ok(())
    }

    fn truncate_string(&self, s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }

    #[allow(dead_code)]
    fn create_wasmrust_plugin_info(&self, version: &str) -> PluginInfo {
        PluginInfo {
            name: "wasmrust".to_string(),
            version: version.to_string(),
            description: "Rust WebAssembly plugin for Wasmrun".to_string(),
            author: "Kumar Anirudha".to_string(),
            extensions: vec!["rs".to_string()],
            entry_files: vec!["Cargo.toml".to_string()],
            plugin_type: PluginType::External,
            source: Some(PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: version.to_string(),
            }),
            dependencies: vec!["cargo".to_string(), "rustc".to_string()],
            capabilities: crate::plugin::PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
            },
        }
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

        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
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

    pub fn update(&mut self, plugin: &str) -> Result<()> {
        println!("🔄 Updating plugin: {}", plugin);

        self.uninstall(plugin)?;
        self.install(plugin, None)
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
            match self.update(&plugin) {
                Ok(_) => println!("✅ Updated {}", plugin),
                Err(e) => eprintln!("❌ Failed to update {}: {}", plugin, e),
            }
        }

        Ok(())
    }

    pub fn set_enabled(&mut self, plugin: &str, enabled: bool) -> Result<()> {
        let action = if enabled { "Enabling" } else { "Disabling" };
        println!("{} plugin: {}", action, plugin);

        self.registry_manager
            .local_registry_mut()
            .set_plugin_enabled(plugin, enabled)?;

        let status = if enabled { "enabled" } else { "disabled" };
        println!("✅ Plugin {} {}", plugin, status);

        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<()> {
        println!("🔍 Searching for plugins matching '{}'...", query);

        let known_plugins = [
            ("wasmrust", "Rust WebAssembly plugin for Wasmrun"),
            ("wasmgo", "Go WebAssembly plugin for Wasmrun (coming soon)"),
        ];

        let matches: Vec<_> = known_plugins
            .iter()
            .filter(|(name, desc)| {
                name.contains(query) || desc.to_lowercase().contains(&query.to_lowercase())
            })
            .collect();

        if matches.is_empty() {
            println!("No plugins found matching '{}'", query);
        } else {
            println!("Found {} plugins:", matches.len());
            for (name, desc) in matches {
                let available = Self::is_plugin_available(name);
                let status = if available {
                    "✅ Available"
                } else {
                    "📦 Not installed"
                };
                println!("  • {} - {} [{}]", name, desc, status);
            }
        }

        Ok(())
    }
}

// Plugin status
#[derive(Debug)]
#[allow(dead_code)]
enum PluginStatus {
    Ready,        // Plugin is properly installed and ready
    NotInstalled, // Plugin directory missing or installation failed
    AccessError,  // Cannot access plugin directory
}

#[allow(dead_code)]
fn check_plugin_status(plugin_dir: &std::path::Path) -> PluginStatus {
    let crates_toml = plugin_dir.join(".crates.toml");
    let crates2_json = plugin_dir.join(".crates2.json");

    if crates_toml.exists() || crates2_json.exists() {
        return PluginStatus::Ready;
    }

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
