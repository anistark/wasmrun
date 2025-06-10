//! External plugin loading and management

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

        // TODO: Figure out plugin metadata. Currently assuming plugin.toml
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
                // Copy executable files (for Unix systems)
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

    /// Check if a file is executable (for Unix-like systems)
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

    /// Check if a file is executable (for Windows systems)
    #[cfg(windows)]
    fn is_executable(path: &std::path::Path) -> bool {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            ext == "exe" || ext == "bat" || ext == "cmd"
        } else {
            false
        }
    }

    /// Check if a file is executable
    #[cfg(not(any(unix, windows)))]
    fn is_executable(_path: &std::path::Path) -> bool {
        false
    }

    /// Uninstall plugin
    pub fn uninstall(plugin_name: &str) -> Result<()> {
        let install_dir = crate::plugin::config::PluginConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(plugin_name);

        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
                ChakraError::from(format!("Failed to remove plugin directory: {}", e))
            })?;
            println!("Removed plugin directory: {}", plugin_dir.display());
        }

        // Clean cache
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
#[allow(dead_code)]
pub fn load_external_plugin(config: &ExternalPluginConfig) -> Result<Box<dyn Plugin>> {
    ExternalPluginLoader::load(config)
}

/// Uninstall external plugin
pub fn uninstall_plugin(name: &str) -> Result<()> {
    PluginInstaller::uninstall(name)
}

/// Validate external plugin
// TODO: Plugin validation - will be used when dynamic loading is implemented to verify plugin compatibility and security before loading
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
