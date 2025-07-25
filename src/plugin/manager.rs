//! Plugin management

use crate::error::{Result, WasmrunError};
use crate::plugin::config::{ExternalPluginEntry, WasmrunConfig};
use crate::plugin::languages::{
    asc_plugin::AscPlugin, c_plugin::CPlugin, python_plugin::PythonPlugin,
};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

pub struct PluginManager {
    builtin_plugins: Vec<Box<dyn Plugin>>,
    external_plugins: HashMap<String, Box<dyn Plugin>>,
    config: WasmrunConfig,
    plugin_stats: PluginStats,
    verbose: bool,
}

#[derive(Debug, Clone)]
pub struct PluginStats {
    pub total_plugins: usize,
    pub builtin_count: usize,
    pub external_count: usize,
    pub enabled_external: usize,
    pub disabled_external: usize,
    pub failed_to_load: usize,
    pub supported_languages: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PluginLoadResult {
    pub loaded_count: usize,
    pub failed_count: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

// TODO: Implement health reporting system for plugin diagnostics
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PluginHealthReport {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
    pub stats: PluginStats,
}

// TODO: Implement plugin suggestions for project types
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PluginSuggestion {
    pub plugin_name: String,
    pub reason: String,
    pub command: String,
    pub priority: PluginPriority,
}

// TODO: Use in plugin suggestion system
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginPriority {
    High,
    Medium,
    Low,
}

impl Default for PluginStats {
    fn default() -> Self {
        Self {
            total_plugins: 0,
            builtin_count: 0,
            external_count: 0,
            enabled_external: 0,
            disabled_external: 0,
            failed_to_load: 0,
            supported_languages: Vec::new(),
        }
    }
}

impl PluginManager {
    pub fn new() -> Result<Self> {
        let mut manager = Self {
            builtin_plugins: vec![
                Box::new(CPlugin::new()),
                Box::new(AscPlugin::new()),
                Box::new(PythonPlugin::new()),
            ],
            external_plugins: HashMap::new(),
            config: WasmrunConfig::load_or_default()?,
            plugin_stats: PluginStats::default(),
            verbose: Self::is_verbose_mode(),
        };

        let load_result = manager.load_external_plugins()?;
        manager.update_stats();
        
        if manager.verbose && (!load_result.warnings.is_empty() || !load_result.errors.is_empty()) {
            manager.print_load_summary(&load_result);
        }
        
        Ok(manager)
    }

    fn is_verbose_mode() -> bool {
        std::env::var("WASMRUN_DEBUG").is_ok() || 
        std::env::var("WASMRUN_VERBOSE").is_ok()
    }

    /// Load all external plugins defined in the configuration.
    fn load_external_plugins(&mut self) -> Result<PluginLoadResult> {
        let mut loaded_count = 0;
        let mut failed_count = 0;
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        println!("🔍 Debug: Loading external plugins...");
        println!("🔍 Debug: Found {} external plugin configs", self.config.external_plugins.len());

        for (plugin_name, entry) in &self.config.external_plugins {
            println!("🔍 Debug: Processing plugin: {}", plugin_name);
            println!("🔍 Debug: Plugin enabled: {}", entry.enabled);
            
            if !entry.enabled {
                if self.verbose {
                    warnings.push(format!("Plugin '{}' is disabled", plugin_name));
                }
                continue;
            }

            println!("🔍 Debug: Attempting to load plugin: {}", plugin_name);
            match self.load_single_external_plugin(plugin_name, entry) {
                Ok(plugin) => {
                    println!("🔍 Debug: Successfully loaded plugin: {}", plugin_name);
                    self.external_plugins.insert(plugin_name.clone(), plugin);
                    loaded_count += 1;
                    if self.verbose {
                        println!("✅ Loaded external plugin: {}", plugin_name);
                    }
                }
                Err(e) => {
                    println!("🔍 Debug: Failed to load plugin {}: {}", plugin_name, e);
                    failed_count += 1;
                    let error_msg = format!("Failed to load plugin '{}': {}", plugin_name, e);
                    errors.push(error_msg.clone());
                    if self.verbose {
                        eprintln!("❌ {}", error_msg);
                    }
                }
            }
        }

        println!("🔍 Debug: Loading complete. Loaded: {}, Failed: {}", loaded_count, failed_count);

        Ok(PluginLoadResult {
            loaded_count,
            failed_count,
            warnings,
            errors,
        })
    }

    /// Load single external plugin
    fn load_single_external_plugin(&self, plugin_name: &str, entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        println!("🔍 Debug: load_single_external_plugin called for: {}", plugin_name);
        println!("🔍 Debug: Entry info: {:?}", entry.info.name);
        println!("🔍 Debug: Entry enabled: {}", entry.enabled);
        
        use crate::plugin::external::ExternalPluginLoader;
        
        println!("🔍 Debug: Calling ExternalPluginLoader::load");
        let result = ExternalPluginLoader::load(entry);
        
        match &result {
            Ok(_) => println!("🔍 Debug: ExternalPluginLoader::load succeeded"),
            Err(e) => println!("🔍 Debug: ExternalPluginLoader::load failed: {}", e),
        }
        
        result
    }

    fn print_load_summary(&self, result: &PluginLoadResult) {
        println!("Plugin loading summary:");
        println!("  Loaded: {}", result.loaded_count);
        if result.failed_count > 0 {
            println!("  Failed: {}", result.failed_count);
        }
        if !result.warnings.is_empty() {
            println!("  Warnings: {}", result.warnings.len());
        }
    }

    /// Install an external plugin from a specified source.
    pub fn install_external_plugin(&mut self, name: &str, source: PluginSource) -> Result<()> {
        if self.verbose {
            println!("🔧 Installing plugin '{}'...", name);
        }

        let plugin_dir = self.get_plugin_directory(name)?;
        
        let plugin_info = match &source {
            PluginSource::CratesIo { name: crate_name, version } => {
                self.install_from_crates_io(name, crate_name, version, &plugin_dir)?
            }
            PluginSource::Git { url, branch } => {
                self.install_from_git(name, url, branch.as_deref(), &plugin_dir)?
            }
            PluginSource::Local { path } => {
                self.install_from_local(name, path, &plugin_dir)?
            }
        };

        self.config.add_external_plugin(
            name.to_string(),
            plugin_info,
            source.clone(),
            plugin_dir.to_string_lossy().to_string(),
        )?;

        if let Some(entry) = self.config.get_external_plugin(name) {
            match self.load_single_external_plugin(name, entry) {
                Ok(plugin) => {
                    self.external_plugins.insert(name.to_string(), plugin);
                    self.update_stats();
                    
                    if self.verbose {
                        println!("✅ Successfully installed and loaded plugin: {}", name);
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("⚠️  Plugin installed but failed to load: {}", e);
                        eprintln!("   You can try 'wasmrun plugin reload' later");
                    }
                }
            }
        }

        Ok(())
    }

    pub fn uninstall_external_plugin(&mut self, name: &str) -> Result<()> {
        if self.verbose {
            println!("🗑️  Uninstalling plugin '{}'...", name);
        }

        if !self.config.is_external_plugin_installed(name) {
            return Err(WasmrunError::from(format!("Plugin '{}' is not installed", name)));
        }

        self.external_plugins.remove(name);
        let plugin_dir = self.get_plugin_directory(name)?;
        self.config.remove_external_plugin(name)?;
        
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir)?;
        }
        
        self.update_stats();
        
        if self.verbose {
            println!("✅ Successfully uninstalled plugin: {}", name);
        }
        
        Ok(())
    }

    pub fn enable_external_plugin(&mut self, name: &str) -> Result<()> {
        self.config.set_external_plugin_enabled(name, true)?;
        
        if let Some(entry) = self.config.get_external_plugin(name) {
            match self.load_single_external_plugin(name, entry) {
                Ok(plugin) => {
                    self.external_plugins.insert(name.to_string(), plugin);
                    self.update_stats();
                    
                    if self.verbose {
                        println!("✅ Plugin '{}' enabled and loaded", name);
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("⚠️  Plugin '{}' enabled but failed to load: {}", name, e);
                    }
                }
            }
        }
        
        Ok(())
    }

    pub fn disable_external_plugin(&mut self, name: &str) -> Result<()> {
        self.config.set_external_plugin_enabled(name, false)?;
        self.external_plugins.remove(name);
        self.update_stats();
        
        if self.verbose {
            println!("✅ Plugin '{}' disabled", name);
        }
        
        Ok(())
    }

    /// List all built-in plugins
    pub fn list_builtin_plugins(&self) -> Vec<&PluginInfo> {
        self.builtin_plugins.iter().map(|p| p.info()).collect()
    }

    /// List all installed external plugins
    pub fn list_installed_external_plugins(&self) -> Vec<&ExternalPluginEntry> {
        self.config.external_plugins.values().collect()
    }

    #[allow(dead_code)]
    pub fn get_plugin_info(&self, name: &str) -> Option<&PluginInfo> {
        self.find_plugin_by_name(name).map(|p| p.info())
    }

    pub fn get_external_plugin_entry(&self, name: &str) -> Option<&ExternalPluginEntry> {
        self.config.get_external_plugin(name)
    }

    pub fn is_plugin_loaded(&self, name: &str) -> bool {
        self.find_plugin_by_name(name).is_some()
    }

    #[allow(dead_code)]
    pub fn is_plugin_available(&self, name: &str) -> bool {
        if self.is_plugin_loaded(name) {
            return true;
        }
        Self::is_external_binary_available(name)
    }

    // TODO: Use in automatic plugin selection for projects
    #[allow(dead_code)]
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

    // TODO: Use in language selection UI
    #[allow(dead_code)]
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

    // TODO: Use in help command
    #[allow(dead_code)]
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

    // TODO: Implement auto-detection of common plugins in PATH
    pub fn get_auto_detected_plugins(&self) -> Vec<String> {
        let mut detected = Vec::new();
        let known_plugins = ["wasmrust", "wasmgo"];

        for plugin_name in &known_plugins {
            if Self::is_external_binary_available(plugin_name) 
                && !self.config.external_plugins.contains_key(*plugin_name) {
                detected.push(plugin_name.to_string());
            }
        }

        detected
    }

    /// Get plugin statistics
    pub fn get_stats(&self) -> &PluginStats {
        &self.plugin_stats
    }

    pub fn update_stats(&mut self) {
        let builtin_count = self.builtin_plugins.len();
        let external_count = self.external_plugins.len();
        let total_plugins = builtin_count + external_count;

        let mut enabled_external = 0;
        let mut disabled_external = 0;
        let mut failed_to_load = 0;

        for (name, entry) in &self.config.external_plugins {
            if entry.enabled {
                if self.external_plugins.contains_key(name) {
                    enabled_external += 1;
                } else {
                    failed_to_load += 1;
                }
            } else {
                disabled_external += 1;
            }
        }

        let mut supported_languages = Vec::new();
        for plugin in &self.builtin_plugins {
            for ext in &plugin.info().extensions {
                if !supported_languages.contains(ext) {
                    supported_languages.push(ext.clone());
                }
            }
        }
        for plugin in self.external_plugins.values() {
            for ext in &plugin.info().extensions {
                if !supported_languages.contains(ext) {
                    supported_languages.push(ext.clone());
                }
            }
        }

        self.plugin_stats = PluginStats {
            total_plugins,
            builtin_count,
            external_count,
            enabled_external,
            disabled_external,
            failed_to_load,
            supported_languages,
        };
    }

    // TODO: Implement health check command
    #[allow(dead_code)]
    pub fn health_check(&self) -> PluginHealthReport {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();
        let mut recommendations = Vec::new();

        if self.plugin_stats.failed_to_load > 0 {
            issues.push(format!("{} plugins failed to load", self.plugin_stats.failed_to_load));
            recommendations.push("Try 'wasmrun plugin reload' or check plugin dependencies".to_string());
        }

        for (name, entry) in &self.config.external_plugins {
            if entry.enabled && !self.external_plugins.contains_key(name) {
                issues.push(format!("Plugin '{}' is enabled but failed to load", name));
                recommendations.push(format!("Try 'wasmrun plugin reload' or check plugin dependencies for '{}'", name));
            }
        }

        let common_plugins = ["wasmrust", "wasmgo"];
        for plugin_name in common_plugins {
            if !self.is_plugin_available(plugin_name) {
                warnings.push(format!("Popular plugin '{}' is not installed", plugin_name));
                recommendations.push(format!("Consider installing '{}' with 'wasmrun plugin install {}'", plugin_name, plugin_name));
            }
        }

        PluginHealthReport {
            healthy: issues.is_empty(),
            issues,
            warnings,
            recommendations,
            stats: self.plugin_stats.clone(),
        }
    }

    /// Get the directory where plugins are stored
    pub fn get_plugin_directory(&self, plugin_name: &str) -> Result<PathBuf> {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| WasmrunError::from("Could not determine home directory"))?;
        
        Ok(PathBuf::from(home_dir).join(".wasmrun/plugins").join(plugin_name))
    }

    fn is_external_binary_available(binary_name: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        
        Command::new(which_cmd)
            .arg(binary_name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Install an external plugin from crates.io
    fn install_from_crates_io(&self, plugin_name: &str, crate_name: &str, version: &str, plugin_dir: &PathBuf) -> Result<PluginInfo> {
        if self.verbose {
            println!("📥 Installing {} v{} from crates.io to {}", crate_name, version, plugin_dir.display());
        }

        std::fs::create_dir_all(plugin_dir)?;

        let version_spec = if version == "*" || version == "latest" { 
            "".to_string() 
        } else { 
            format!("@{}", version) 
        };
        
        let cargo_args = vec![
            "install".to_string(),
            format!("{}{}", crate_name, version_spec),
            "--root".to_string(),
            plugin_dir.to_string_lossy().to_string(),
            "--force".to_string(),
        ];
        
        if self.verbose {
            println!("  Executing: cargo {}", cargo_args.join(" "));
        }
        
        let output = Command::new("cargo")
            .args(&cargo_args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(WasmrunError::from(format!(
                "Failed to install plugin '{}' from crates.io.\nError: {}\nOutput: {}", 
                crate_name, stderr.trim(), stdout.trim()
            )));
        }

        if self.verbose {
            println!("  ✅ Cargo install completed successfully");
        }

        self.extract_plugin_info(plugin_name, crate_name, plugin_dir)
    }

    /// Install an external plugin from a git repository
    fn install_from_git(&self, plugin_name: &str, url: &str, branch: Option<&str>, plugin_dir: &PathBuf) -> Result<PluginInfo> {
        if self.verbose {
            println!("📥 Cloning {} from git...", url);
        }

        let mut cmd = Command::new("git");
        cmd.args(&["clone", url, &plugin_dir.to_string_lossy()]);
        
        if let Some(branch) = branch {
            cmd.args(&["--branch", branch]);
        }

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!("Failed to clone git repository: {}", stderr)));
        }

        self.build_plugin_from_source(plugin_dir)?;
        self.extract_plugin_info(plugin_name, plugin_name, plugin_dir)
    }

    /// Install an external plugin from a local path
    fn install_from_local(&self, plugin_name: &str, source_path: &PathBuf, plugin_dir: &PathBuf) -> Result<PluginInfo> {
        if self.verbose {
            println!("📁 Copying from local path: {}", source_path.display());
        }

        self.copy_dir_recursive(source_path, plugin_dir)?;
        self.build_plugin_from_source(plugin_dir)?;
        self.extract_plugin_info(plugin_name, plugin_name, plugin_dir)
    }

    /// Build a plugin from source code in the specified directory
    fn build_plugin_from_source(&self, plugin_dir: &PathBuf) -> Result<()> {
        if self.verbose {
            println!("🔨 Building plugin from source...");
        }

        let output = Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(plugin_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!("Failed to build plugin: {}", stderr)));
        }

        Ok(())
    }

    fn extract_plugin_info(&self, plugin_name: &str, _crate_name: &str, plugin_dir: &PathBuf) -> Result<PluginInfo> {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        
        if cargo_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                return Ok(self.parse_cargo_toml_for_plugin_info(plugin_name, &content));
            }
        }
        
        Ok(self.create_default_plugin_info(plugin_name))
    }

    fn parse_cargo_toml_for_plugin_info(&self, plugin_name: &str, content: &str) -> PluginInfo {
        let mut version = "unknown".to_string();
        let mut description = format!("External {} plugin", plugin_name);
        let mut author = "External".to_string();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("version") && line.contains('=') {
                if let Some(version_part) = line.split('=').nth(1) {
                    version = version_part
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                }
            } else if line.starts_with("description") && line.contains('=') {
                if let Some(desc_part) = line.split('=').nth(1) {
                    description = desc_part
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                }
            } else if line.starts_with("authors") && line.contains('=') {
                if let Some(author_part) = line.split('=').nth(1) {
                    author = author_part
                        .trim()
                        .trim_matches('[')
                        .trim_matches(']')
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                }
            }
        }

        self.create_plugin_info_with_metadata(plugin_name, version, description, author)
    }

    fn create_default_plugin_info(&self, plugin_name: &str) -> PluginInfo {
        self.create_plugin_info_with_metadata(
            plugin_name,
            "unknown".to_string(),
            format!("External {} plugin", plugin_name),
            "External".to_string(),
        )
    }

    fn create_plugin_info_with_metadata(&self, plugin_name: &str, version: String, description: String, author: String) -> PluginInfo {
        PluginInfo {
            name: plugin_name.to_string(),
            version,
            description,
            author,
            extensions: match plugin_name {
                "wasmrust" => vec!["rs".to_string()],
                "wasmgo" => vec!["go".to_string()],
                _ => vec![],
            },
            entry_files: match plugin_name {
                "wasmrust" => vec!["Cargo.toml".to_string(), "main.rs".to_string()],
                "wasmgo" => vec!["go.mod".to_string(), "main.go".to_string()],
                _ => vec![],
            },
            plugin_type: PluginType::External,
            source: None,
            dependencies: match plugin_name {
                "wasmrust" => vec!["cargo".to_string(), "rustc".to_string()],
                "wasmgo" => vec!["tinygo".to_string()],
                _ => vec![],
            },
            capabilities: match plugin_name {
                "wasmrust" => PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: true,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
                },
                "wasmgo" => PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: false,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec!["wasm".to_string()],
                },
                _ => PluginCapabilities::default(),
            },
        }
    }

    fn copy_dir_recursive(&self, from: &PathBuf, to: &PathBuf) -> Result<()> {
        std::fs::create_dir_all(to)?;
        
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let from_path = entry.path();
            let to_path = to.join(entry.file_name());

            if file_type.is_dir() {
                self.copy_dir_recursive(&from_path, &to_path)?;
            } else {
                std::fs::copy(&from_path, &to_path)?;
            }
        }

        Ok(())
    }
}

/// Plugin commands interface
pub struct PluginCommands {
    manager: PluginManager,
}

impl PluginCommands {
    pub fn new() -> Result<Self> {
        let manager = PluginManager::new()?;
        Ok(Self {
            manager,
        })
    }

    /// List all available plugins
    pub fn list(&self, show_all: bool) -> Result<()> {
        let builtin_plugins = self.manager.list_builtin_plugins();
        let external_plugins = self.manager.list_installed_external_plugins();

        if builtin_plugins.is_empty() && external_plugins.is_empty() {
            println!("No plugins installed.");
            return Ok(());
        }

        println!("Available plugins:\n");

        // Built-in plugins
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

        // External plugins
        if !external_plugins.is_empty() {
            println!("🔌 \x1b[1;36mExternal Plugins:\x1b[0m");
            for plugin_entry in &external_plugins {
                let plugin_info = &plugin_entry.info;
                let is_loaded = self.manager.is_plugin_loaded(&plugin_info.name);

                let actual_version = self.get_actual_plugin_version(&plugin_info.name);
                
                let status = if !plugin_entry.enabled {
                    "\x1b[1;33m⏸ Disabled\x1b[0m"
                } else if is_loaded {
                    "\x1b[1;32m✓ Loaded\x1b[0m"
                } else {
                    "\x1b[1;31m✗ Failed\x1b[0m"
                };

                println!(
                    "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - {} [{}]",
                    plugin_info.name, actual_version, plugin_info.description, status
                );

                if show_all {
                    println!("    Extensions: {}", plugin_info.extensions.join(", "));
                    println!("    Entry files: {}", plugin_info.entry_files.join(", "));
                    println!("    Installed: {}", plugin_entry.installed_at);
                    if let Some(source) = &plugin_info.source {
                        match source {
                            PluginSource::CratesIo { name, version } => {
                                println!("    Source: crates.io/{} ({})", name, version);
                            }
                            PluginSource::Git { url, branch } => {
                                println!("    Source: Git {} {}", url, 
                                    branch.as_ref().map(|b| format!("({})", b)).unwrap_or_default());
                            }
                            PluginSource::Local { path } => {
                                println!("    Source: Local ({})", path.display());
                            }
                        }
                    }
                    if !plugin_info.capabilities.custom_targets.is_empty() {
                        println!("    Targets: {}", plugin_info.capabilities.custom_targets.join(", "));
                    }

                    if actual_version != plugin_info.version && plugin_info.version != "unknown" {
                        println!("    Registered version: {}", plugin_info.version);
                    }
                    
                    println!();
                }
            }
            println!();
        }

        // Auto-detected plugins
        let auto_detected = self.manager.get_auto_detected_plugins();
        if !auto_detected.is_empty() {
            println!("🔍 \x1b[1;36mAuto-detected Plugins:\x1b[0m");
            for plugin_name in &auto_detected {
                // FIX: Get actual version for auto-detected plugins too
                let detected_version = self.get_actual_plugin_version(plugin_name);
                
                println!(
                    "  • \x1b[1;37m{:<15}\x1b[0m v{:<8} - Available binary [\x1b[1;34m⚡ Install\x1b[0m]",
                    plugin_name, detected_version
                );
            }
            println!("\n💡 Run \x1b[1;37mwasmrun plugin install <n>\x1b[0m to formally install auto-detected plugins");
            println!();
        }

        // Statistics
        let stats = self.manager.get_stats();
        println!("🌏 \x1b[1;36mStatistics:\x1b[0m");
        println!("  Total plugins: {} ({} built-in, {} external)", 
                stats.total_plugins, stats.builtin_count, stats.external_count);
        
        if stats.external_count > 0 {
            println!("  External status: {} enabled, {} disabled, {} failed to load", 
                    stats.enabled_external, stats.disabled_external, stats.failed_to_load);
        }
        
        println!("  Supported languages: {}", stats.supported_languages.join(", "));

        Ok(())
    }

    fn get_actual_plugin_version(&self, plugin_name: &str) -> String {
        if let Some(plugin_entry) = self.manager.config.get_external_plugin(plugin_name) {
            let config_version = &plugin_entry.info.version;
            if config_version != "unknown" && config_version != "latest" && config_version != "*" {
                if let Ok(plugin_dir) = self.manager.get_plugin_directory(plugin_name) {
                    if let Some(actual_version) = self.read_version_from_cargo_toml(&plugin_dir) {
                        if actual_version != *config_version {
                            return actual_version;
                        }
                        return actual_version;
                    }
                }
                return config_version.clone();
            }
        }

        if let Ok(plugin_dir) = self.manager.get_plugin_directory(plugin_name) {
            if let Some(version) = self.read_version_from_cargo_toml(&plugin_dir) {
                return version;
            }
        }

        if ["wasmrust", "wasmgo"].contains(&plugin_name) {
            if let Ok(output) = Command::new("cargo")
                .args(["search", plugin_name, "--limit", "1"])
                .output()
            {
                if output.status.success() {
                    let search_output = String::from_utf8_lossy(&output.stdout);
                    for line in search_output.lines() {
                        if line.starts_with(plugin_name) && line.contains('=') {
                            if let Some(version_part) = line.split('=').nth(1) {
                                if let Some(quoted_version) = version_part.split('#').next() {
                                    let version = quoted_version
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .trim();
                                    if !version.is_empty() && version.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                                        return version.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        "unknown".to_string()
    }

    fn read_version_from_cargo_toml(&self, plugin_dir: &PathBuf) -> Option<String> {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return None;
        }

        if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("version") && line.contains('=') {
                    if let Some(version_part) = line.split('=').nth(1) {
                        let version = version_part
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .trim();
                        if !version.is_empty() && version.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                            return Some(version.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    #[allow(dead_code)]
    pub fn install(&mut self, plugin_name: &str) -> Result<()> {
        let source = PluginSource::CratesIo {
            name: plugin_name.to_string(),
            version: "latest".to_string(),
        };
        self.manager.install_external_plugin(plugin_name, source)?;
        println!("✅ Plugin '{}' installed successfully", plugin_name);
        Ok(())
    }

    pub fn uninstall(&mut self, plugin_name: &str) -> Result<()> {
        self.manager.uninstall_external_plugin(plugin_name)?;
        println!("✅ Plugin '{}' uninstalled successfully", plugin_name);
        Ok(())
    }

    pub fn enable(&mut self, plugin_name: &str) -> Result<()> {
        self.manager.enable_external_plugin(plugin_name)?;
        println!("✅ Plugin '{}' enabled", plugin_name);
        Ok(())
    }

    pub fn disable(&mut self, plugin_name: &str) -> Result<()> {
        self.manager.disable_external_plugin(plugin_name)?;
        println!("✅ Plugin '{}' disabled", plugin_name);
        Ok(())
    }

    pub fn info(&self, plugin: &str) -> Result<()> {
        if let Some(builtin) = self.manager.list_builtin_plugins().iter().find(|p| p.name == plugin) {
            self.print_plugin_info_box(builtin, None, true)?;
            return Ok(());
        }

        if let Some(external) = self.manager.get_external_plugin_entry(plugin) {
            self.print_plugin_info_box(&external.info, Some(external), false)?;
            return Ok(());
        }

        if self.manager.get_auto_detected_plugins().contains(&plugin.to_string()) {
            let info = self.create_auto_detected_plugin_info(plugin);
            self.print_plugin_info_box(&info, None, false)?;
            println!("\n💡 Run \x1b[1;37mwasmrun plugin install {}\x1b[0m to formally register this plugin", plugin);
            return Ok(());
        }

        Err(WasmrunError::from(format!("Plugin '{}' not found", plugin)))
    }

    fn print_plugin_info_box(&self, info: &PluginInfo, external_entry: Option<&ExternalPluginEntry>, is_builtin: bool) -> Result<()> {
        let is_loaded = self.manager.is_plugin_loaded(&info.name);
        let actual_version = if is_builtin { 
            info.version.clone() 
        } else { 
            self.get_actual_plugin_version(&info.name) 
        };
        
        let box_width = 70;

        println!("\x1b[1;36m╭{}\x1b[0m", "─".repeat(box_width - 2));
        println!("\x1b[1;36m│\x1b[0m  🔌 \x1b[1;37mPlugin Information: {}\x1b[0m{}\x1b[1;36m│\x1b[0m", 
            info.name,
            " ".repeat(box_width.saturating_sub(info.name.len() + 22))
        );
        println!("\x1b[1;36m├{}\x1b[0m", "─".repeat(box_width - 2));

        self.print_info_row("Type", &format!("{}", if is_builtin { "Built-in" } else { "External" }), box_width);
        
        let status = if is_builtin {
            "\x1b[1;32m✅ Always Available\x1b[0m".to_string()
        } else if is_loaded {
            "\x1b[1;32m✅ Loaded\x1b[0m".to_string()
        } else {
            "\x1b[1;31m❌ Not Loaded\x1b[0m".to_string()
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

        if let Some(external) = external_entry {
            println!("\x1b[1;36m├{}\x1b[0m", "─".repeat(box_width - 2));
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

        if !info.dependencies.is_empty() {
            println!("\x1b[1;36m├{}\x1b[0m", "─".repeat(box_width - 2));
            self.print_info_row("Dependencies", &info.dependencies.join(", "), box_width);
        }

        println!("\x1b[1;36m├{}\x1b[0m", "─".repeat(box_width - 2));
        let capabilities = vec![
            if info.capabilities.compile_wasm { "✅ WebAssembly" } else { "❌ WebAssembly" },
            if info.capabilities.compile_webapp { "✅ Web Apps" } else { "❌ Web Apps" },
            if info.capabilities.live_reload { "✅ Live Reload" } else { "❌ Live Reload" },
            if info.capabilities.optimization { "✅ Optimization" } else { "❌ Optimization" },
        ];
        self.print_info_row("Capabilities", &capabilities.join(", "), box_width);
        
        if !info.capabilities.custom_targets.is_empty() {
            self.print_info_row("Targets", &info.capabilities.custom_targets.join(", "), box_width);
        }

        println!("\x1b[1;36m╰{}\x1b[0m", "─".repeat(box_width - 2));

        Ok(())
    }

    fn print_info_row(&self, label: &str, value: &str, box_width: usize) {
        let content_width = box_width - 4;
        let label_width = 12;
        let value_width = content_width - label_width - 2;

        let truncated_value = if value.chars().count() > value_width {
            let truncated: String = value.chars().take(value_width.saturating_sub(3)).collect();
            format!("{}...", truncated)
        } else {
            value.to_string()
        };
        
        println!("\x1b[1;36m│\x1b[0m \x1b[1;33m{:<width$}\x1b[0m: {:<value_width$} \x1b[1;36m│\x1b[0m", 
            label, 
            truncated_value,
            width = label_width,
            value_width = value_width
        );
    }

    fn create_auto_detected_plugin_info(&self, plugin_name: &str) -> PluginInfo {
        PluginInfo {
            name: plugin_name.to_string(),
            version: "auto-detected".to_string(),
            description: format!("Auto-detected {} plugin", plugin_name),
            author: "External".to_string(),
            extensions: match plugin_name {
                "wasmrust" => vec!["rs".to_string()],
                "wasmgo" => vec!["go".to_string()],
                _ => vec![],
            },
            entry_files: match plugin_name {
                "wasmrust" => vec!["Cargo.toml".to_string(), "main.rs".to_string()],
                "wasmgo" => vec!["go.mod".to_string(), "main.go".to_string()],
                _ => vec![],
            },
            plugin_type: PluginType::External,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: plugin_name == "wasmrust",
                live_reload: true,
                optimization: true,
                custom_targets: match plugin_name {
                    "wasmrust" => vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
                    "wasmgo" => vec!["wasm".to_string()],
                    _ => vec![],
                },
            },
        }
    }

    // TODO: Implement health check command
    #[allow(dead_code)]
    pub fn health(&self) -> Result<()> {
        let report = self.manager.health_check();
        
        println!("🏥 \x1b[1;36mPlugin Health Report\x1b[0m\n");
        
        if report.healthy {
            println!("✅ \x1b[1;32mAll plugins are healthy\x1b[0m");
        } else {
            println!("⚠️  \x1b[1;33mFound {} issues\x1b[0m", report.issues.len());
            for issue in &report.issues {
                println!("  ❌ {}", issue);
            }
        }
        
        if !report.warnings.is_empty() {
            println!("\n⚠️  \x1b[1;33mWarnings:\x1b[0m");
            for warning in &report.warnings {
                println!("  • {}", warning);
            }
        }
        
        if !report.recommendations.is_empty() {
            println!("\n💡 \x1b[1;36mRecommendations:\x1b[0m");
            for rec in &report.recommendations {
                println!("  • {}", rec);
            }
        }
        
        let stats = &report.stats;
        println!("\n📊 \x1b[1;36mStatistics:\x1b[0m");
        println!("  Total plugins: {}", stats.total_plugins);
        println!("  Built-in: {}", stats.builtin_count);
        println!("  External: {} ({} enabled, {} disabled)", 
                stats.external_count, stats.enabled_external, stats.disabled_external);
        
        if stats.failed_to_load > 0 {
            println!("  Failed to load: {}", stats.failed_to_load);
        }
        
        Ok(())
    }

    // TODO: Implement plugin update functionality
    pub fn update(&mut self, plugin_name: &str) -> Result<()> {
        println!("🔄 Updating plugin '{}'...", plugin_name);
        
        if !self.manager.config.is_external_plugin_installed(plugin_name) {
            return Err(WasmrunError::from(format!("Plugin '{}' is not installed", plugin_name)));
        }

        let entry = self.manager.config.get_external_plugin(plugin_name)
            .ok_or_else(|| WasmrunError::from(format!("Plugin '{}' not found in config", plugin_name)))?;

        let source = entry.info.source.clone()
            .ok_or_else(|| WasmrunError::from(format!("Plugin '{}' has no source information", plugin_name)))?;

        self.manager.uninstall_external_plugin(plugin_name)?;
        self.manager.install_external_plugin(plugin_name, source)?;
        
        println!("✅ Plugin '{}' updated successfully", plugin_name);
        Ok(())
    }

    // TODO: Implement plugin reload functionality
    #[allow(dead_code)]
    pub fn reload(&mut self) -> Result<()> {
        println!("🔄 Reloading all plugins...");
        
        self.manager.external_plugins.clear();
        self.manager.config = WasmrunConfig::load_or_default()?;
        
        let result = self.manager.load_external_plugins()?;
        self.manager.update_stats();
        
        println!("✅ Reload complete: {} loaded, {} failed", 
                result.loaded_count, result.failed_count);
        
        if !result.errors.is_empty() {
            println!("⚠️  Errors during reload:");
            for error in &result.errors {
                println!("  • {}", error);
            }
        }
        
        Ok(())
    }

    // Version-aware install method
    pub fn install_with_version(&mut self, plugin_name: &str, version: Option<&str>) -> Result<()> {
        let actual_version = if let Some(v) = version {
            v.to_string()
        } else {
            self.get_latest_crate_version(plugin_name)?
        };

        let source = PluginSource::CratesIo {
            name: plugin_name.to_string(),
            version: actual_version.clone(),
        };

        self.manager.install_external_plugin(plugin_name, source)?;
        println!("✅ Plugin '{}' v{} installed successfully", plugin_name, actual_version);
        Ok(())
    }

    // Get latest version from crates.io
    fn get_latest_crate_version(&self, crate_name: &str) -> Result<String> {
        use std::process::Command;
        
        let output = Command::new("cargo")
            .args(["search", crate_name, "--limit", "1"])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to search crates.io: {}", e)))?;

        if output.status.success() {
            let search_output = String::from_utf8_lossy(&output.stdout);
            for line in search_output.lines() {
                if line.starts_with(crate_name) && line.contains('=') {
                    if let Some(version_part) = line.split('=').nth(1) {
                        if let Some(quoted_version) = version_part.split('#').next() {
                            let version = quoted_version
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'')
                                .trim();
                            if !version.is_empty() {
                                return Ok(version.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok("latest".to_string())
    }

    pub fn search(&self, query: &str) -> Result<()> {
        println!("🔍 Searching for plugins matching '{}'...", query);
        println!("Search functionality coming soon!");
        Ok(())
    }

    pub fn update_all(&mut self) -> Result<()> {
        println!("🔄 Update all functionality coming soon!");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn sync_plugin_versions(&mut self) -> Result<()> {
        let mut updated_count = 0;
        
        let external_plugins: Vec<String> = self.manager
            .list_installed_external_plugins()
            .iter()
            .map(|entry| entry.info.name.clone())
            .collect();

        for plugin_name in external_plugins {
            let stored_version = self.manager.config
                .get_external_plugin(&plugin_name)
                .map(|entry| entry.info.version.clone())
                .unwrap_or_else(|| "unknown".to_string());

            let actual_version = self.get_actual_plugin_version(&plugin_name);
            
            if actual_version != "unknown" && actual_version != stored_version {
                if let Some(entry) = self.manager.config.external_plugins.get_mut(&plugin_name) {
                    entry.info.version = actual_version.clone();
                    updated_count += 1;
                    
                    if self.manager.verbose {
                        println!("Updated {} version: {} -> {}", plugin_name, stored_version, actual_version);
                    }
                }
            }
        }

        if updated_count > 0 {
            self.manager.config.save()?;
            
            if self.manager.verbose {
                println!("✅ Synced {} plugin versions to config", updated_count);
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn sync_versions(&mut self) -> Result<()> {
        println!("🔄 Syncing plugin versions...");
        self.sync_plugin_versions()?;
        println!("✅ Plugin versions synced");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn debug_plugin(&self, plugin_name: &str) -> Result<()> {
        println!("🔍 Debugging plugin: {}", plugin_name);

        if let Some(entry) = self.manager.config.get_external_plugin(plugin_name) {
            println!("✅ Plugin found in config");
            println!("  Name: {}", entry.info.name);
            println!("  Version: {}", entry.info.version);
            println!("  Enabled: {}", entry.enabled);
            println!("  Install path: {}", entry.install_path);
            if let Some(exec_path) = &entry.executable_path {
                println!("  Executable path: {}", exec_path);
            }
        } else {
            println!("❌ Plugin not found in config");
            return Ok(());
        }

        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        
        println!("\n🔍 Checking binary availability:");
        if let Ok(output) = Command::new(which_cmd).arg(plugin_name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                println!("✅ Plugin binary found at: {}", path.trim());
            } else {
                println!("❌ Plugin binary not found in PATH");
                println!("💡 Try: export PATH=\"$HOME/.wasmrun/plugins/{}/bin:$PATH\"", plugin_name);
            }
        } else {
            println!("❌ Failed to run which command");
        }

        if let Ok(plugin_dir) = self.manager.get_plugin_directory(plugin_name) {
            println!("\n🔍 Checking installation directory:");
            println!("  Plugin dir: {}", plugin_dir.display());
            println!("  Directory exists: {}", plugin_dir.exists());
            
            if plugin_dir.exists() {
                let bin_dir = plugin_dir.join("bin");
                println!("  Bin dir: {}", bin_dir.display());
                println!("  Bin directory exists: {}", bin_dir.exists());
                
                if bin_dir.exists() {
                    println!("  Contents of bin directory:");
                    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let is_executable = path.is_file() && {
                                #[cfg(unix)]
                                {
                                    use std::os::unix::fs::PermissionsExt;
                                    if let Ok(metadata) = std::fs::metadata(&path) {
                                        metadata.permissions().mode() & 0o111 != 0
                                    } else {
                                        false
                                    }
                                }
                                #[cfg(not(unix))]
                                {
                                    true
                                }
                            };
                            
                            let marker = if is_executable { "✅" } else { "📄" };
                            println!("    {} {}", marker, path.file_name().unwrap_or_default().to_string_lossy());
                        }
                    }
                } else {
                    println!("  ❌ Bin directory does not exist");
                }
            } else {
                println!("  ❌ Plugin directory does not exist");
            }
        }

        println!("\n🔍 Attempting to load plugin:");
        if let Some(entry) = self.manager.config.get_external_plugin(plugin_name) {
            match self.manager.load_single_external_plugin(plugin_name, entry) {
                Ok(_) => println!("✅ Plugin loaded successfully"),
                Err(e) => println!("❌ Failed to load plugin: {}", e),
            }
        }
        
        Ok(())
    }

    #[allow(dead_code)]
    pub fn setup_plugin_path(&self, plugin_name: &str) -> Result<()> {
        if let Ok(plugin_dir) = self.manager.get_plugin_directory(plugin_name) {
            let bin_dir = plugin_dir.join("bin");
            
            if bin_dir.exists() {
                println!("To make {} available globally, add this to your shell profile:", plugin_name);
                println!();
                println!("export PATH=\"{}:$PATH\"", bin_dir.display());
                println!();
                println!("Or run this command in your current shell:");
                println!("export PATH=\"{}:$PATH\"", bin_dir.display());

                if let Ok(current_path) = std::env::var("PATH") {
                    let bin_dir_str = bin_dir.to_string_lossy();
                    if current_path.contains(&*bin_dir_str) {
                        println!("\n✅ Plugin directory is already in PATH");
                    } else {
                        println!("\n⚠️  Plugin directory is NOT in current PATH");
                    }
                }
            } else {
                println!("❌ Plugin bin directory not found: {}", bin_dir.display());
            }
        } else {
            println!("❌ Plugin not installed");
        }
        
        Ok(())
    }
}
