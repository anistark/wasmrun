use crate::error::{Result, WasmrunError};
use crate::plugin::Plugin;
use std::path::{Path, PathBuf};

pub struct PluginUtils;

impl PluginUtils {
    pub fn get_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        let wasmrun_dir = Self::get_wasmrun_directory()?;
        let plugin_dir = wasmrun_dir.join("plugins").join(plugin_name);
        Ok(plugin_dir)
    }

    pub fn get_wasmrun_directory() -> Result<PathBuf> {
        if let Some(home_dir) = dirs::home_dir() {
            Ok(home_dir.join(".wasmrun"))
        } else {
            Err(WasmrunError::from("Could not determine home directory"))
        }
    }

    pub fn is_plugin_available(plugin_name: &str) -> bool {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            let cargo_toml = plugin_dir.join("Cargo.toml");
            let src_lib = plugin_dir.join("src").join("lib.rs");

            if cargo_toml.exists() && src_lib.exists() {
                return true;
            }

            let manifest_path = plugin_dir.join("wasmrun.toml");
            if manifest_path.exists() {
                return true;
            }

            let metadata_path = plugin_dir.join(".wasmrun_metadata");
            if metadata_path.exists() {
                return true;
            }

            let lib_extensions = ["so", "dylib", "dll"];
            for ext in &lib_extensions {
                let lib_path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
                if lib_path.exists() {
                    return true;
                }
            }

            let bin_path = plugin_dir
                .join("bin")
                .join(format!("wasmrun-{plugin_name}"));
            if bin_path.exists() {
                return true;
            }
        }

        false
    }

    pub fn detect_plugin_version_from_metadata(plugin_name: &str) -> Option<String> {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            let metadata_file = plugin_dir.join(".wasmrun_metadata");
            if metadata_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&metadata_file) {
                    for line in content.lines() {
                        if line.starts_with("version=") {
                            return Some(line.replace("version=", "").trim().to_string());
                        }
                    }
                }
            }

            let cargo_toml = plugin_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    for line in content.lines() {
                        if line.starts_with("version") && line.contains('=') {
                            let version =
                                line.split('=').nth(1)?.trim().trim_matches('"').to_string();
                            return Some(version);
                        }
                    }
                }
            }
        }

        None
    }

    pub fn create_metadata_file(plugin_name: &str, plugin_dir: &Path, version: &str) -> Result<()> {
        let metadata_content = format!(
            "plugin_name={}\nversion={}\ninstall_date={}\n",
            plugin_name,
            version,
            chrono::Utc::now().to_rfc3339()
        );

        let metadata_path = plugin_dir.join(".wasmrun_metadata");
        std::fs::write(&metadata_path, metadata_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create metadata file: {e}")))?;

        Ok(())
    }

    // Plugin Language Detection Utilities

    /// Extract supported languages from a plugin's capabilities
    /// Returns a vector of language names supported by the plugin
    pub fn get_supported_languages(plugin: &dyn Plugin) -> Vec<String> {
        let plugin_info = plugin.info();

        // First check if plugin has supported_languages in capabilities
        if let Some(supported_langs) = &plugin_info.capabilities.supported_languages {
            return supported_langs.clone();
        }

        let plugin_name = plugin_info.name.to_lowercase();
        if plugin_name.contains("rust") {
            vec!["rust".to_string()]
        } else if plugin_name.contains("go") {
            vec!["go".to_string()]
        } else if plugin_name.contains("c") || plugin_name.contains("cpp") {
            vec!["c".to_string(), "cpp".to_string()]
        } else if plugin_name.contains("python") || plugin_name.contains("py") {
            vec!["python".to_string()]
        } else if plugin_name.contains("asc") || plugin_name.contains("assemblyscript") {
            vec!["assemblyscript".to_string(), "asc".to_string()]
        } else {
            // Unknown plugin, return the plugin name as the language
            vec![plugin_info.name.clone()]
        }
    }

    /// Check if a plugin supports a specific language
    /// Case-insensitive comparison
    pub fn supports_language(plugin: &dyn Plugin, language: &str) -> bool {
        let supported_languages = Self::get_supported_languages(plugin);
        let target_lang = language.to_lowercase();

        supported_languages
            .iter()
            .any(|lang| lang.to_lowercase() == target_lang)
    }
}
