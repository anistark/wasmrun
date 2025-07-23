//! Plugin system for Wasmrun - Built-in and External plugins

use crate::compiler::builder::WasmBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

pub mod config;
pub mod external;
pub mod languages;
pub mod manager;
pub mod registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    CratesIo { name: String, version: String },
    Git { url: String, branch: Option<String> },
    Local { path: PathBuf },
}

pub trait Plugin: Send + Sync {
    fn info(&self) -> &PluginInfo;
    fn can_handle_project(&self, project_path: &str) -> bool;
    fn get_builder(&self) -> Box<dyn WasmBuilder>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub extensions: Vec<String>,
    pub entry_files: Vec<String>,
    pub plugin_type: PluginType,
    pub source: Option<PluginSource>,
    pub dependencies: Vec<String>,
    pub capabilities: PluginCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    Builtin,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapabilities {
    pub compile_wasm: bool,
    pub compile_webapp: bool,
    pub live_reload: bool,
    pub optimization: bool,
    pub custom_targets: Vec<String>,
}

impl Default for PluginCapabilities {
    fn default() -> Self {
        Self {
            compile_wasm: true,
            compile_webapp: false,
            live_reload: false,
            optimization: false,
            custom_targets: vec![],
        }
    }
}

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> crate::error::Result<Self> {
        let mut plugins: Vec<Box<dyn Plugin>> = vec![
            Box::new(languages::c_plugin::CPlugin::new()),
            Box::new(languages::asc_plugin::AscPlugin::new()),
            Box::new(languages::python_plugin::PythonPlugin::new()),
        ];

        if let Ok(config) = config::WasmrunConfig::load() {
            for (_, entry) in config.external_plugins {
                if entry.enabled {
                    match external::ExternalPluginLoader::load(&entry) {
                        Ok(plugin) => {
                            plugins.push(plugin);
                        }
                        Err(_e) => {
                            eprintln!(
                                "Failed to load external plugin '{}'. Please check the configuration.",
                                entry.info.name
                            );
                        }
                    }
                }
            }
        }

        Self::load_auto_detected_plugins(&mut plugins);

        Ok(Self { plugins })
    }

    fn load_auto_detected_plugins(plugins: &mut Vec<Box<dyn Plugin>>) {
        let known_plugins = [
            ("wasmrust", Self::create_wasmrust_info),
            // ("wasmgo", Self::create_wasmgo_info),
        ];

        for (plugin_name, info_creator) in &known_plugins {
            if plugins.iter().any(|p| p.info().name == *plugin_name) {
                continue;
            }

            if Self::is_command_available(plugin_name) {
                let info = info_creator();
                let entry = config::ExternalPluginEntry {
                    info,
                    enabled: true,
                    install_path: plugin_name.to_string(),
                    source: PluginSource::CratesIo {
                        name: plugin_name.to_string(),
                        version: "auto-detected".to_string(),
                    },
                    executable_path: Some(plugin_name.to_string()),
                    installed_at: chrono::Utc::now().to_rfc3339(),
                };

                if let Ok(plugin) = external::ExternalPluginLoader::load(&entry) {
                    plugins.push(plugin);
                }
            }
        }
    }

    fn create_wasmrust_info() -> PluginInfo {
        PluginInfo {
            name: "wasmrust".to_string(),
            version: Self::get_plugin_version("wasmrust").unwrap_or_else(|| "unknown".to_string()),
            description: "Rust WebAssembly plugin for Wasmrun".to_string(),
            author: "Kumar Anirudha".to_string(),
            extensions: vec!["rs".to_string()],
            entry_files: vec!["Cargo.toml".to_string()],
            plugin_type: PluginType::External,
            source: Some(PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: Self::get_plugin_version("wasmrust")
                    .unwrap_or_else(|| "latest".to_string()),
            }),
            dependencies: vec!["cargo".to_string(), "rustc".to_string()],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
            },
        }
    }

    fn get_plugin_version(plugin_name: &str) -> Option<String> {
        if let Ok(output) = Command::new(plugin_name).arg("--version").output() {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = version_output.split_whitespace().nth(1) {
                    return Some(version.to_string());
                }
            }
        }

        if let Ok(output) = Command::new(plugin_name).arg("info").output() {
            if output.status.success() {
                let info_output = String::from_utf8_lossy(&output.stdout);
                for line in info_output.lines() {
                    if line.contains("WasmRust v") {
                        if let Some(version) = line.split("WasmRust v").nth(1) {
                            return Some(version.trim().to_string());
                        }
                    }
                }
            }
        }

        None
    }

    fn is_command_available(command: &str) -> bool {
        if let Ok(output) = Command::new(command).arg("--version").output() {
            if output.status.success() {
                return true;
            }
        }

        if command == "wasmrust" {
            if let Ok(output) = Command::new(command).arg("info").output() {
                if output.status.success() {
                    return true;
                }
            }
        }

        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        if let Ok(output) = Command::new(which_cmd).arg(command).output() {
            return output.status.success();
        }

        false
    }

    pub fn get_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    pub fn find_plugin_for_project(&self, project_path: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.can_handle_project(project_path))
            .map(|boxed| boxed.as_ref())
    }

    pub fn get_plugin_by_name(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|plugin| {
                plugin.info().name == name
                    || plugin.info().name.contains(name)
                    || (name == "rust" && plugin.info().name == "wasmrust")
            })
            .map(|boxed| boxed.as_ref())
    }

    #[allow(dead_code)]
    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.iter().map(|plugin| plugin.info()).collect()
    }

    pub fn get_plugin_info(&self, name: &str) -> Option<&PluginInfo> {
        self.get_plugin_by_name(name).map(|plugin| plugin.info())
    }

    #[allow(dead_code)]
    pub fn check_all_dependencies(&self) -> Vec<(String, Vec<String>)> {
        self.plugins
            .iter()
            .map(|plugin| {
                let builder = plugin.get_builder();
                (plugin.info().name.clone(), builder.check_dependencies())
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn verify_dependencies(&self, required_plugins: &[String]) -> crate::error::Result<()> {
        let available_plugins: Vec<String> =
            self.plugins.iter().map(|p| p.info().name.clone()).collect();

        let missing_plugins: Vec<String> = required_plugins
            .iter()
            .filter(|&name| !available_plugins.contains(name))
            .cloned()
            .collect();

        if !missing_plugins.is_empty() {
            let missing_names = missing_plugins.join(", ");
            return Err(crate::error::WasmrunError::from(format!(
                "Missing required plugins: {}. Install them with 'wasmrun plugin install <plugin-name>'",
                missing_names
            )));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn has_plugin(&self, name: &str) -> bool {
        self.get_plugin_by_name(name).is_some()
    }

    pub fn plugin_counts(&self) -> (usize, usize) {
        let builtin_count = self
            .plugins
            .iter()
            .filter(|p| p.info().plugin_type == PluginType::Builtin)
            .count();
        let external_count = self
            .plugins
            .iter()
            .filter(|p| p.info().plugin_type == PluginType::External)
            .count();

        (builtin_count, external_count)
    }

    #[allow(dead_code)]
    pub fn reload(&mut self) -> crate::error::Result<()> {
        *self = Self::new()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_plugins_for_language(&self, language: &str) -> Vec<&dyn Plugin> {
        self.plugins
            .iter()
            .filter(|plugin| {
                plugin.info().name.contains(language)
                    || plugin.info().extensions.iter().any(|ext| {
                        match language.to_lowercase().as_str() {
                            "rust" => ext == "rs",
                            "go" => ext == "go",
                            "c" | "cpp" => ext == "c" || ext == "cpp",
                            "python" => ext == "py",
                            "javascript" | "typescript" => ext == "js" || ext == "ts",
                            _ => false,
                        }
                    })
            })
            .map(|boxed| boxed.as_ref())
            .collect()
    }

    #[allow(dead_code)]
    pub fn suggest_plugin_registration(&self) {
        let auto_detected = ["wasmrust", "wasmgo"];

        for plugin_name in &auto_detected {
            if Self::is_command_available(plugin_name) {
                // Check if it's already properly registered
                if let Ok(config) = config::WasmrunConfig::load() {
                    if !config.external_plugins.contains_key(*plugin_name) {
                        println!("💡 Found {} in PATH but not registered.", plugin_name);
                        println!("   Run: wasmrun plugin install {}", plugin_name);
                    }
                }
            }
        }
    }
}
