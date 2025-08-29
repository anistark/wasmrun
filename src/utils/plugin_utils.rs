use crate::compiler::ProjectLanguage;
use crate::error::{Result, WasmrunError};
use crate::plugin::registry::PluginRegistry;
use crate::plugin::Plugin;
use crate::utils::SystemUtils;
use std::path::{Path, PathBuf};

pub struct PluginUtils;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginValidationResult {
    pub is_installed: bool,
    pub is_functional: bool,
    pub has_metadata: bool,
    pub missing_dependencies: Vec<String>,
    pub version: Option<String>,
    pub install_path: Option<String>,
}

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

    #[allow(dead_code)]
    pub fn copy_dir_recursive(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
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

    #[allow(dead_code)]
    pub fn check_plugin_dependencies(plugin_name: &str) -> Vec<String> {
        Self::check_generic_plugin_dependencies(plugin_name)
    }

    #[allow(dead_code)]
    pub fn check_generic_plugin_dependencies(plugin_name: &str) -> Vec<String> {
        let mut missing = Vec::new();

        // Try to get metadata-based dependencies
        if let Ok(metadata) = PluginRegistry::get_plugin_metadata(plugin_name) {
            for tool in &metadata.dependencies.tools {
                if !SystemUtils::is_tool_available(tool) {
                    missing.push(tool.to_string()); // Fixed: changed from clone() to to_string()
                }
            }
        } else if plugin_name.contains("rust") {
            if !SystemUtils::is_tool_available("rustc") {
                missing.push("rustc".to_string());
            }
            if !SystemUtils::is_tool_available("cargo") {
                missing.push("cargo".to_string());
            }
        } else if plugin_name.contains("go") {
            if !SystemUtils::is_tool_available("tinygo") {
                missing.push("tinygo".to_string());
            }
        } else if plugin_name.contains("zig") {
            if !SystemUtils::is_tool_available("zig") {
                missing.push("zig".to_string());
            }
        } else if (plugin_name.contains("js") || plugin_name.contains("javascript"))
            && !SystemUtils::is_tool_available("node")
        {
            missing.push("node".to_string());
        }

        missing
    }

    #[allow(dead_code)]
    pub fn validate_plugin_installation(plugin_name: &str) -> Result<PluginValidationResult> {
        let mut result = PluginValidationResult {
            is_installed: false,
            is_functional: false,
            has_metadata: false,
            missing_dependencies: vec![],
            version: None,
            install_path: None,
        };

        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            result.install_path = Some(plugin_dir.to_string_lossy().to_string());

            result.is_installed = plugin_dir.exists();

            if result.is_installed {
                let metadata_path = plugin_dir.join(".wasmrun_metadata");
                result.has_metadata = metadata_path.exists();

                result.version = Self::detect_plugin_version_from_metadata(plugin_name);

                result.missing_dependencies = Self::check_generic_plugin_dependencies(plugin_name);
                result.is_functional = result.missing_dependencies.is_empty();
            }
        }

        Ok(result)
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

    /// Get the primary (first) language supported by a plugin
    /// Returns the main language that the plugin is designed for
    pub fn get_primary_language(plugin: &dyn Plugin) -> String {
        let languages = Self::get_supported_languages(plugin);
        languages
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
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

    /// Map plugin to ProjectLanguage enum for compatibility with existing code
    /// This function handles the mapping from plugin languages to the ProjectLanguage enum
    pub fn map_plugin_to_project_language(
        plugin: &dyn Plugin,
        project_path: &str,
    ) -> ProjectLanguage {
        let primary_language = Self::get_primary_language(plugin);

        match primary_language.to_lowercase().as_str() {
            "rust" => ProjectLanguage::Rust,
            "go" => ProjectLanguage::Go,
            "c" | "cpp" | "c++" => ProjectLanguage::C,
            "python" | "py" => ProjectLanguage::Python,
            "assemblyscript" | "asc" => ProjectLanguage::Asc,
            _ => {
                // Unknown language, fallback to project detection
                crate::compiler::detect_project_language(project_path)
            }
        }
    }

    /// Check if a plugin can handle projects of a specific language
    /// This is useful for finding plugins when you know the target language
    #[allow(dead_code)]
    pub fn can_handle_language(plugin: &dyn Plugin, target_language: &str) -> bool {
        Self::supports_language(plugin, target_language)
    }

    /// Get a human-readable description of languages supported by a plugin
    /// Returns a formatted string like "Rust, Go" or "C/C++"
    #[allow(dead_code)]
    pub fn get_languages_description(plugin: &dyn Plugin) -> String {
        let languages = Self::get_supported_languages(plugin);

        // Handle special cases for better formatting
        if languages.len() == 2
            && languages.contains(&"c".to_string())
            && languages.contains(&"cpp".to_string())
        {
            return "C/C++".to_string();
        }

        if languages.len() == 2
            && languages.contains(&"assemblyscript".to_string())
            && languages.contains(&"asc".to_string())
        {
            return "AssemblyScript".to_string();
        }

        // For other cases, just join with commas
        languages.join(", ")
    }
}
