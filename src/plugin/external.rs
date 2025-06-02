//! External plugin loading and management
//!
//! Handles installation, loading, and management of external plugins

use crate::error::{ChakraError, Result};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// External plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPluginConfig {
    /// Plugin name
    pub name: String,
    /// Plugin source
    pub source: PluginSource,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Plugin-specific configuration
    pub config: HashMap<String, toml::Value>,
}

/// External plugin interface for dynamic loading
#[allow(dead_code)]
pub trait ExternalPlugin: Plugin {
    /// Get the plugin's dynamic library path
    fn lib_path(&self) -> &PathBuf;

    /// Get plugin metadata from the dynamic library
    fn load_metadata() -> Result<PluginInfo>
    where
        Self: Sized;
}

/// Plugin installation manager
pub struct PluginInstaller;

impl PluginInstaller {
    /// Install a plugin from a source
    pub fn install(source: PluginSource) -> Result<PathBuf> {
        match source {
            PluginSource::CratesIo { name, version } => {
                Self::install_from_crates_io(&name, &version)
            }
            PluginSource::Git { url, branch } => Self::install_from_git(&url, branch.as_deref()),
            PluginSource::Local { path } => Self::install_from_local(&path),
        }
    }

    /// Install plugin from crates.io
    fn install_from_crates_io(name: &str, version: &str) -> Result<PathBuf> {
        let install_dir = crate::plugin::config::PluginConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(name);

        // Create plugin directory
        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| ChakraError::from(format!("Failed to create plugin directory: {}", e)))?;

        println!("Installing {} v{} from crates.io...", name, version);

        // Use cargo install to build the plugin
        let output = Command::new("cargo")
            .args([
                "install",
                "--root",
                plugin_dir.to_str().unwrap(),
                "--version",
                version,
                name,
            ])
            .output()
            .map_err(|e| ChakraError::from(format!("Failed to execute cargo install: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChakraError::from(format!(
                "Failed to install plugin from crates.io: {}",
                stderr
            )));
        }

        println!("Successfully installed plugin: {}", name);
        Ok(plugin_dir)
    }

    /// Install plugin from Git repository
    fn install_from_git(url: &str, branch: Option<&str>) -> Result<PathBuf> {
        let install_dir = crate::plugin::config::PluginConfig::plugin_dir()?;
        let cache_dir = crate::plugin::config::PluginConfig::cache_dir()?;

        // Extract repository name from URL
        let repo_name = url
            .split('/')
            .last()
            .unwrap_or("unknown")
            .strip_suffix(".git")
            .unwrap_or("unknown");

        let clone_dir = cache_dir.join(format!("git-{}", repo_name));
        let plugin_dir = install_dir.join(repo_name);

        println!("Installing {} from git repository...", repo_name);

        // Remove existing clone if it exists
        if clone_dir.exists() {
            std::fs::remove_dir_all(&clone_dir).map_err(|e| {
                ChakraError::from(format!("Failed to remove existing clone: {}", e))
            })?;
        }

        // Clone the repository
        let mut clone_cmd = Command::new("git");
        clone_cmd.args(["clone", url, clone_dir.to_str().unwrap()]);

        if let Some(branch) = branch {
            clone_cmd.args(["--branch", branch]);
        }

        let output = clone_cmd
            .output()
            .map_err(|e| ChakraError::from(format!("Failed to execute git clone: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChakraError::from(format!(
                "Failed to clone git repository: {}",
                stderr
            )));
        }

        // Build the plugin
        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&clone_dir)
            .output()
            .map_err(|e| ChakraError::from(format!("Failed to execute cargo build: {}", e)))?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            return Err(ChakraError::from(format!(
                "Failed to build plugin from git: {}",
                stderr
            )));
        }

        // Copy built artifacts to plugin directory
        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| ChakraError::from(format!("Failed to create plugin directory: {}", e)))?;

        // Copy the built library
        let target_dir = clone_dir.join("target").join("release");
        Self::copy_plugin_artifacts(&target_dir, &plugin_dir)?;

        // Copy plugin metadata if it exists
        let metadata_file = clone_dir.join("plugin.toml");
        if metadata_file.exists() {
            let dest_metadata = plugin_dir.join("plugin.toml");
            std::fs::copy(&metadata_file, &dest_metadata)
                .map_err(|e| ChakraError::from(format!("Failed to copy plugin metadata: {}", e)))?;
        }

        println!("Successfully installed plugin: {}", repo_name);
        Ok(plugin_dir)
    }

    /// Install plugin from local path
    fn install_from_local(path: &PathBuf) -> Result<PathBuf> {
        let install_dir = crate::plugin::config::PluginConfig::plugin_dir()?;

        // Extract plugin name from Cargo.toml
        let cargo_toml_path = path.join("Cargo.toml");
        let plugin_name = Self::extract_plugin_name(&cargo_toml_path)?;

        let plugin_dir = install_dir.join(&plugin_name);

        println!("Installing {} from local path...", plugin_name);

        // Build the plugin
        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(path)
            .output()
            .map_err(|e| ChakraError::from(format!("Failed to execute cargo build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChakraError::from(format!(
                "Failed to build local plugin: {}",
                stderr
            )));
        }

        // Copy built artifacts to plugin directory
        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| ChakraError::from(format!("Failed to create plugin directory: {}", e)))?;

        let target_dir = path.join("target").join("release");
        Self::copy_plugin_artifacts(&target_dir, &plugin_dir)?;

        // Copy plugin metadata if it exists
        let metadata_file = path.join("plugin.toml");
        if metadata_file.exists() {
            let dest_metadata = plugin_dir.join("plugin.toml");
            std::fs::copy(&metadata_file, &dest_metadata)
                .map_err(|e| ChakraError::from(format!("Failed to copy plugin metadata: {}", e)))?;
        }

        println!("Successfully installed plugin: {}", plugin_name);
        Ok(plugin_dir)
    }

    /// Extract plugin name from Cargo.toml
    fn extract_plugin_name(cargo_toml_path: &PathBuf) -> Result<String> {
        let content = std::fs::read_to_string(cargo_toml_path)
            .map_err(|e| ChakraError::from(format!("Failed to read Cargo.toml: {}", e)))?;

        // Simple parsing - look for name = "..."
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("name") && line.contains('=') {
                if let Some(name_part) = line.split('=').nth(1) {
                    let name = name_part.trim().trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        return Ok(name.to_string());
                    }
                }
            }
        }

        Err(ChakraError::from(
            "Could not extract plugin name from Cargo.toml",
        ))
    }

    /// Copy plugin artifacts to installation directory
    fn copy_plugin_artifacts(source_dir: &Path, dest_dir: &Path) -> Result<()> {
        // Look for dynamic libraries and executables
        let entries = std::fs::read_dir(source_dir)
            .map_err(|e| ChakraError::from(format!("Failed to read source directory: {}", e)))?;

        let mut copied_files = 0;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();

                // Copy dynamic libraries and executables
                if ext == "so" || ext == "dll" || ext == "dylib" {
                    let dest_path = dest_dir.join(path.file_name().unwrap());
                    std::fs::copy(&path, &dest_path).map_err(|e| {
                        ChakraError::from(format!("Failed to copy artifact: {}", e))
                    })?;
                    copied_files += 1;
                }
            } else if Self::is_executable(&path) {
                // Copy executable files (Unix systems)
                let dest_path = dest_dir.join(path.file_name().unwrap());
                std::fs::copy(&path, &dest_path)
                    .map_err(|e| ChakraError::from(format!("Failed to copy artifact: {}", e)))?;
                copied_files += 1;
            }
        }

        if copied_files == 0 {
            return Err(ChakraError::from("No plugin artifacts found to copy"));
        }

        Ok(())
    }

    /// Check if a file is executable (Unix-like systems)
    #[cfg(unix)]
    fn is_executable(path: &std::path::Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let permissions = metadata.permissions();
            permissions.mode() & 0o111 != 0
        } else {
            false
        }
    }

    /// Check if a file is executable (Windows systems)
    #[cfg(windows)]
    fn is_executable(path: &std::path::Path) -> bool {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            ext == "exe" || ext == "bat" || ext == "cmd"
        } else {
            false
        }
    }

    /// Check if a file is executable (other systems)
    #[cfg(not(any(unix, windows)))]
    fn is_executable(_path: &std::path::Path) -> bool {
        false
    }

    /// Uninstall a plugin
    pub fn uninstall(plugin_name: &str) -> Result<()> {
        let install_dir = crate::plugin::config::PluginConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(plugin_name);

        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
                ChakraError::from(format!("Failed to remove plugin directory: {}", e))
            })?;
            println!("Removed plugin directory: {}", plugin_dir.display());
        }

        // Also clean up cache if it exists
        let cache_dir = crate::plugin::config::PluginConfig::cache_dir()?;
        let cache_plugin_dir = cache_dir.join(format!("git-{}", plugin_name));
        if cache_plugin_dir.exists() {
            std::fs::remove_dir_all(&cache_plugin_dir)
                .map_err(|e| ChakraError::from(format!("Failed to remove plugin cache: {}", e)))?;
        }

        Ok(())
    }
}

/// External plugin loader
pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    /// Load an external plugin from configuration
    pub fn load(config: &ExternalPluginConfig) -> Result<Box<dyn Plugin>> {
        if !config.enabled {
            return Err(ChakraError::from(format!(
                "Plugin '{}' is disabled",
                config.name
            )));
        }

        let install_dir = crate::plugin::config::PluginConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(&config.name);

        if !plugin_dir.exists() {
            return Err(ChakraError::from(format!(
                "Plugin '{}' is not installed. Run 'chakra plugin install {}'",
                config.name, config.name
            )));
        }

        // Create a wrapper that loads the plugin dynamically
        let plugin = ExternalPluginWrapper::new(plugin_dir, config.clone())?;

        Ok(Box::new(plugin))
    }
}

/// Wrapper for external plugins
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    #[allow(dead_code)]
    config: ExternalPluginConfig,
    plugin_dir: PathBuf,
    // TODO: hold the loaded dynamic library
    // _lib: libloading::Library,
}

impl ExternalPluginWrapper {
    /// Create a new external plugin wrapper
    pub fn new(plugin_dir: PathBuf, config: ExternalPluginConfig) -> Result<Self> {
        // Load plugin metadata
        let info = Self::load_plugin_info(&plugin_dir, &config)?;

        Ok(Self {
            info,
            config,
            plugin_dir,
        })
    }

    /// Load plugin information from the plugin directory
    fn load_plugin_info(plugin_dir: &Path, config: &ExternalPluginConfig) -> Result<PluginInfo> {
        // Try to load from plugin.toml first (TOML format is more standard)
        let toml_metadata_file = plugin_dir.join("plugin.toml");
        if toml_metadata_file.exists() {
            return Self::load_info_from_toml_metadata(&toml_metadata_file, config);
        }

        // Fallback to default info
        let info = PluginInfo {
            name: config.name.clone(),
            version: "1.0.0".to_string(),
            description: format!("External plugin: {}", config.name),
            author: "Unknown".to_string(),
            extensions: vec![],
            entry_files: vec![],
            plugin_type: PluginType::External,
            source: Some(config.source.clone()),
            dependencies: vec![],
            capabilities: PluginCapabilities::default(),
        };

        Ok(info)
    }

    /// Load plugin info from TOML metadata file
    fn load_info_from_toml_metadata(
        metadata_file: &Path,
        config: &ExternalPluginConfig,
    ) -> Result<PluginInfo> {
        let content = std::fs::read_to_string(metadata_file)
            .map_err(|e| ChakraError::from(format!("Failed to read plugin metadata: {}", e)))?;

        #[derive(Deserialize)]
        struct PluginMetadata {
            name: String,
            version: String,
            description: String,
            author: Option<String>,
            extensions: Option<Vec<String>>,
            entry_files: Option<Vec<String>>,
            dependencies: Option<Vec<String>>,
            capabilities: Option<PluginCapabilities>,
        }

        let metadata: PluginMetadata = toml::from_str(&content)
            .map_err(|e| ChakraError::from(format!("Failed to parse plugin metadata: {}", e)))?;

        let info = PluginInfo {
            name: metadata.name,
            version: metadata.version,
            description: metadata.description,
            author: metadata.author.unwrap_or_else(|| "Unknown".to_string()),
            extensions: metadata.extensions.unwrap_or_default(),
            entry_files: metadata.entry_files.unwrap_or_default(),
            plugin_type: PluginType::External,
            source: Some(config.source.clone()),
            dependencies: metadata.dependencies.unwrap_or_default(),
            capabilities: metadata.capabilities.unwrap_or_default(),
        };

        Ok(info)
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check if project contains any of the entry files
        for entry_file in &self.info.entry_files {
            let entry_path = std::path::Path::new(project_path).join(entry_file);
            if entry_path.exists() {
                return true;
            }
        }

        // Check if project contains files with supported extensions
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if self.info.extensions.contains(&ext) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn crate::compiler::builder::WasmBuilder> {
        // TODO: return the plugin's builder
        // For now, return a dummy builder that explains the limitation
        Box::new(ExternalBuilderProxy {
            plugin_name: self.info.name.clone(),
            plugin_dir: self.plugin_dir.clone(),
        })
    }

    fn initialize(&mut self) -> Result<()> {
        // TODO: this would call the plugin's initialize function
        println!("Initializing external plugin: {}", self.info.name);
        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        // TODO: this would call the plugin's cleanup function
        println!("Cleaning up external plugin: {}", self.info.name);
        Ok(())
    }
}

/// Proxy builder for external plugins
struct ExternalBuilderProxy {
    plugin_name: String,
    #[allow(dead_code)]
    plugin_dir: PathBuf,
}

impl crate::compiler::builder::WasmBuilder for ExternalBuilderProxy {
    fn language_name(&self) -> &str {
        &self.plugin_name
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &[]
    }

    fn supported_extensions(&self) -> &[&str] {
        &[]
    }

    fn check_dependencies(&self) -> Vec<String> {
        vec![format!("External plugin '{}' is installed but dynamic loading is not yet implemented. This will be added in a future update.", self.plugin_name)]
    }

    fn build(
        &self,
        _config: &crate::compiler::builder::BuildConfig,
    ) -> crate::error::CompilationResult<crate::compiler::builder::BuildResult> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: format!(
                "External plugin '{}' (dynamic loading not yet implemented)",
                self.plugin_name
            ),
        })
    }
}

/// Install a plugin from source
pub fn install_plugin(source: PluginSource) -> Result<Box<dyn Plugin>> {
    let plugin_dir = PluginInstaller::install(source.clone())?;

    // Extract plugin name from directory
    let plugin_name = plugin_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Create a temporary config for loading
    let config = ExternalPluginConfig {
        name: plugin_name,
        source,
        enabled: true,
        config: HashMap::new(),
    };

    ExternalPluginLoader::load(&config)
}

/// Load an external plugin from configuration
pub fn load_external_plugin(config: &ExternalPluginConfig) -> Result<Box<dyn Plugin>> {
    ExternalPluginLoader::load(config)
}

/// Uninstall an external plugin
pub fn uninstall_plugin(name: &str) -> Result<()> {
    PluginInstaller::uninstall(name)
}

/// Validate external plugin
#[allow(dead_code)]
pub fn validate_plugin(plugin_dir: &PathBuf) -> Result<()> {
    // Check if plugin directory exists
    if !plugin_dir.exists() {
        return Err(ChakraError::from("Plugin directory does not exist"));
    }

    // Check for required files (either executable or library)
    let has_executable = std::fs::read_dir(plugin_dir)?
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            let path = entry.path();
            PluginInstaller::is_executable(&path)
                || path.extension().map_or(false, |ext| {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    ext_str == "so" || ext_str == "dll" || ext_str == "dylib"
                })
        });

    if !has_executable {
        return Err(ChakraError::from(
            "No executable or library files found in plugin directory",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_external_plugin_config() {
        let config = ExternalPluginConfig {
            name: "test-plugin".to_string(),
            source: PluginSource::CratesIo {
                name: "test-plugin".to_string(),
                version: "1.0.0".to_string(),
            },
            enabled: true,
            config: HashMap::new(),
        };

        assert_eq!(config.name, "test-plugin");
        assert!(config.enabled);
        assert!(config.config.is_empty());
    }

    #[test]
    fn test_extract_plugin_name() {
        let temp_dir = tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");

        let content = r#"
[package]
name = "my-plugin"
version = "1.0.0"
"#;

        std::fs::write(&cargo_toml, content).unwrap();

        let name = PluginInstaller::extract_plugin_name(&cargo_toml).unwrap();
        assert_eq!(name, "my-plugin");
    }

    #[test]
    fn test_extract_plugin_name_missing() {
        let temp_dir = tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");

        let content = r#"
[package]
version = "1.0.0"
"#;

        std::fs::write(&cargo_toml, content).unwrap();

        let result = PluginInstaller::extract_plugin_name(&cargo_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_wrapper_creation() {
        let temp_dir = tempdir().unwrap();
        let plugin_dir = temp_dir.path().to_path_buf();

        let config = ExternalPluginConfig {
            name: "test-plugin".to_string(),
            source: PluginSource::Local {
                path: plugin_dir.clone(),
            },
            enabled: true,
            config: HashMap::new(),
        };

        let wrapper = ExternalPluginWrapper::new(plugin_dir, config);
        assert!(wrapper.is_ok());

        let wrapper = wrapper.unwrap();
        assert_eq!(wrapper.info().name, "test-plugin");
        assert_eq!(wrapper.info().plugin_type, PluginType::External);
    }

    #[test]
    fn test_plugin_with_metadata() {
        let temp_dir = tempdir().unwrap();
        let plugin_dir = temp_dir.path().to_path_buf();

        // Create plugin metadata file with proper TOML syntax
        let metadata_content = r#"
name = "awesome-plugin"
version = "2.0.0"
description = "An awesome external plugin"
author = "Plugin Developer"
extensions = ["awesome"]
entry_files = ["awesome.toml"]
dependencies = ["awesome-tool"]

[capabilities]
compile_wasm = true
compile_webapp = false
live_reload = true
optimization = true
custom_targets = ["awesome-target"]
"#;

        std::fs::write(plugin_dir.join("plugin.toml"), metadata_content).unwrap();

        let config = ExternalPluginConfig {
            name: "awesome-plugin".to_string(),
            source: PluginSource::Local {
                path: plugin_dir.clone(),
            },
            enabled: true,
            config: HashMap::new(),
        };

        let wrapper = ExternalPluginWrapper::new(plugin_dir, config).unwrap();
        let info = wrapper.info();

        assert_eq!(info.name, "awesome-plugin");
        assert_eq!(info.version, "2.0.0");
        assert_eq!(info.author, "Plugin Developer");
        assert!(info.extensions.contains(&"awesome".to_string()));
        assert!(info.entry_files.contains(&"awesome.toml".to_string()));
        assert!(info.dependencies.contains(&"awesome-tool".to_string()));
        assert!(info.capabilities.compile_wasm);
        assert!(!info.capabilities.compile_webapp);
        assert!(info
            .capabilities
            .custom_targets
            .contains(&"awesome-target".to_string()));
    }

    #[test]
    fn test_can_handle_project() {
        let temp_dir = tempdir().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        let project_dir = temp_dir.path().join("project");

        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        // Create plugin with specific entry files and extensions using proper TOML syntax
        let metadata_content = r#"
name = "test-plugin"
version = "1.0.0"
description = "Test plugin"
extensions = ["test"]
entry_files = ["test.config"]
"#;

        std::fs::write(plugin_dir.join("plugin.toml"), metadata_content).unwrap();

        let config = ExternalPluginConfig {
            name: "test-plugin".to_string(),
            source: PluginSource::Local {
                path: plugin_dir.clone(),
            },
            enabled: true,
            config: HashMap::new(),
        };

        let wrapper = ExternalPluginWrapper::new(plugin_dir, config).unwrap();

        // Test with no matching files
        assert!(!wrapper.can_handle_project(project_dir.to_str().unwrap()));

        // Test with matching entry file
        std::fs::write(project_dir.join("test.config"), "test").unwrap();
        assert!(wrapper.can_handle_project(project_dir.to_str().unwrap()));

        // Remove entry file and test with matching extension
        std::fs::remove_file(project_dir.join("test.config")).unwrap();
        std::fs::write(project_dir.join("example.test"), "test").unwrap();
        assert!(wrapper.can_handle_project(project_dir.to_str().unwrap()));
    }

    #[test]
    fn test_validate_plugin() {
        let temp_dir = tempdir().unwrap();
        let plugin_dir = temp_dir.path().to_path_buf();

        // Test with non-existent directory
        let result = validate_plugin(&plugin_dir);
        assert!(result.is_err());

        // Create directory but no executable
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("readme.txt"), "test").unwrap();

        let result = validate_plugin(&plugin_dir);
        assert!(result.is_err());

        // Add a library file
        std::fs::write(plugin_dir.join("libplugin.so"), "fake lib").unwrap();

        let result = validate_plugin(&plugin_dir);
        assert!(result.is_ok());
    }
}
