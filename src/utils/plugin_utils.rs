use crate::error::{Result, WasmrunError};
use std::path::{Path, PathBuf};

/// Plugin-specific utilities
pub struct PluginUtils;

impl PluginUtils {
    /// Get the plugin directory path
    pub fn get_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        use crate::plugin::config::WasmrunConfig;
        let config_dir = WasmrunConfig::config_dir()?;
        Ok(config_dir.join("plugins").join(plugin_name))
    }

    /// Check if a plugin is available (has proper directory structure)
    pub fn is_plugin_available(plugin_name: &str) -> bool {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            if plugin_dir.exists() {
                let cargo_toml_path = plugin_dir.join("Cargo.toml");
                if cargo_toml_path.exists() {
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

                let src_path = plugin_dir.join("src");
                if src_path.exists() && src_path.is_dir() {
                    return true;
                }
            }
        }
        false
    }

    /// Detect plugin version from metadata file
    pub fn detect_plugin_version_from_metadata(plugin_name: &str) -> Option<String> {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            let metadata_path = plugin_dir.join(".wasmrun_metadata");
            if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                for line in content.lines() {
                    if line.starts_with("version = ") {
                        if let Some(version) = line.split(" = ").nth(1) {
                            return Some(version.trim_matches('"').to_string());
                        }
                    }
                }
            }

            let cargo_toml_path = plugin_dir.join("Cargo.toml");
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                return crate::utils::SystemUtils::detect_version_from_cargo_toml(&content);
            }
        }
        None
    }

    /// Check if a Cargo.toml belongs to a wasmrun plugin
    pub fn is_valid_wasmrun_plugin(cargo_toml_path: &Path) -> bool {
        if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
            content.contains("[wasmrun.plugin]")
                || content.contains("[wasm_plugin]")
                || content.contains("wasm-bindgen")
                || content.contains("tinygo")
        } else {
            false
        }
    }

    /// Create plugin metadata file
    pub fn create_metadata_file(plugin_name: &str, plugin_dir: &Path, version: &str) -> Result<()> {
        let metadata_content = format!(
            "[metadata]\ninstalled_at = \"{}\"\nversion = \"{}\"\nplugin_name = \"{}\"\ninstall_method = \"wasmrun\"\n",
            chrono::Utc::now().to_rfc3339(),
            version,
            plugin_name
        );

        let metadata_path = plugin_dir.join(".wasmrun_metadata");
        std::fs::write(&metadata_path, metadata_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create metadata file: {e}")))?;

        Ok(())
    }

    /// Recursively copy directory contents
    #[allow(dead_code)]
    pub fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
        if !from.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(to)
            .map_err(|e| WasmrunError::from(format!("Failed to create directory: {e}")))?;

        for entry in std::fs::read_dir(from)
            .map_err(|e| WasmrunError::from(format!("Failed to read directory: {e}")))?
        {
            let entry = entry
                .map_err(|e| WasmrunError::from(format!("Failed to read directory entry: {e}")))?;
            let file_type = entry
                .file_type()
                .map_err(|e| WasmrunError::from(format!("Failed to get file type: {e}")))?;
            let from_path = entry.path();
            let to_path = to.join(entry.file_name());

            if file_type.is_dir() {
                Self::copy_dir_recursive(&from_path, &to_path)?;
            } else {
                std::fs::copy(&from_path, &to_path)
                    .map_err(|e| WasmrunError::from(format!("Failed to copy file: {e}")))?;
            }
        }

        Ok(())
    }

    /// Check plugin dependencies based on plugin type
    pub fn check_plugin_dependencies(plugin_name: &str) -> Vec<String> {
        use crate::utils::SystemUtils;

        match plugin_name {
            "wasmrust" => {
                let mut missing = Vec::new();
                if !SystemUtils::is_tool_available("cargo") {
                    missing.push("cargo".to_string());
                }
                if !SystemUtils::is_tool_available("rustc") {
                    missing.push("rustc".to_string());
                }
                if !SystemUtils::is_wasm_target_installed() {
                    missing.push("wasm32-unknown-unknown target (run: rustup target add wasm32-unknown-unknown)".to_string());
                }
                missing
            }
            "wasmgo" => {
                if SystemUtils::is_tool_available("tinygo") {
                    vec![]
                } else {
                    vec!["tinygo".to_string()]
                }
            }
            _ => vec![],
        }
    }
}
