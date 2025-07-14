//! External plugin loading and management

use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::CompilationResult;
use crate::error::{Result, WasmrunError};
use crate::plugin::config::{ExternalPluginEntry, WasmrunConfig};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub struct PluginInstaller;

impl PluginInstaller {
    pub fn install(source: PluginSource) -> Result<PathBuf> {
        match source {
            PluginSource::CratesIo { name, version } => {
                Self::install_from_crates_io(&name, &version)
            }
            PluginSource::Git { url, branch } => Self::install_from_git(&url, branch.as_deref()),
            PluginSource::Local { path } => Self::install_from_local(&path),
        }
    }

    fn install_from_crates_io(name: &str, version: &str) -> Result<PathBuf> {
        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(name);

        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {}", e)))?;

        println!("Installing {} v{} from crates.io...", name, version);

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
            .map_err(|e| WasmrunError::from(format!("Failed to execute cargo install: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to install plugin from crates.io: {}",
                stderr
            )));
        }

        // Validate plugin installation
        if !Self::validate_plugin_installation(name, &plugin_dir)? {
            std::fs::remove_dir_all(&plugin_dir).ok();
            return Err(WasmrunError::from(format!(
                "Crate '{}' does not appear to be a wasmrun plugin",
                name
            )));
        }

        println!("Successfully installed plugin: {}", name);
        Ok(plugin_dir)
    }

    fn install_from_git(url: &str, branch: Option<&str>) -> Result<PathBuf> {
        let install_dir = WasmrunConfig::plugin_dir()?;
        let cache_dir = WasmrunConfig::cache_dir()?;

        let plugin_name = url
            .split('/')
            .last()
            .unwrap_or("unknown-plugin")
            .trim_end_matches(".git");

        let cache_plugin_dir = cache_dir.join(format!("git-{}", plugin_name));
        let plugin_dir = install_dir.join(plugin_name);

        println!("Installing {} from Git: {}", plugin_name, url);

        let mut git_args = vec!["clone", url, cache_plugin_dir.to_str().unwrap()];
        if let Some(branch) = branch {
            git_args.extend(&["--branch", branch]);
        }

        let output = Command::new("git")
            .args(&git_args)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to execute git clone: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to clone Git repository: {}",
                stderr
            )));
        }

        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&cache_plugin_dir)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to execute cargo build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to build plugin: {}",
                stderr
            )));
        }

        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {}", e)))?;

        Self::copy_plugin_artifacts(&cache_plugin_dir, &plugin_dir)?;

        println!("Successfully installed plugin: {}", plugin_name);
        Ok(plugin_dir)
    }

    fn install_from_local(path: &PathBuf) -> Result<PathBuf> {
        if !path.exists() {
            return Err(WasmrunError::from(format!(
                "Local plugin path does not exist: {}",
                path.display()
            )));
        }

        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let plugin_dir = install_dir.join(&plugin_name);

        println!(
            "Installing {} from local path: {}",
            plugin_name,
            path.display()
        );

        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(path)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to execute cargo build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to build plugin: {}",
                stderr
            )));
        }

        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {}", e)))?;

        Self::copy_plugin_artifacts(path, &plugin_dir)?;

        println!("Successfully installed plugin: {}", plugin_name);
        Ok(plugin_dir)
    }

    fn validate_plugin_installation(name: &str, plugin_dir: &Path) -> Result<bool> {
        // Check for cargo install artifacts
        let crates_toml = plugin_dir.join(".crates.toml");
        let crates2_json = plugin_dir.join(".crates2.json");

        if crates_toml.exists() || crates2_json.exists() {
            return Ok(true);
        }

        // Validate by name/description for plugin-like crates
        let plugin_indicators = ["wasmrun", "wasm-run", "plugin", "compiler", "builder"];

        let name_lower = name.to_lowercase();
        if plugin_indicators
            .iter()
            .any(|indicator| name_lower.contains(indicator))
        {
            return Ok(true);
        }

        // Check description via cargo search
        if let Ok(description) = Self::fetch_crate_description(name) {
            let desc_lower = description.to_lowercase();
            let desc_suggests_plugin = plugin_indicators
                .iter()
                .any(|indicator| desc_lower.contains(indicator))
                || desc_lower.contains("wasmrun")
                || desc_lower.contains("webassembly")
                || desc_lower.contains("wasm");

            if desc_suggests_plugin {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn fetch_crate_description(name: &str) -> Result<String> {
        let output = Command::new("cargo")
            .args(["search", name, "--limit", "1"])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to search crates.io: {}", e)))?;

        if !output.status.success() {
            return Err(WasmrunError::from("Failed to fetch crate metadata"));
        }

        let search_output = String::from_utf8_lossy(&output.stdout);

        for line in search_output.lines() {
            if let Some(parts) = line.split_once(" = ") {
                let crate_name = parts.0.trim();
                if crate_name == name {
                    let rest = parts.1;
                    if let Some((_, description_part)) = rest.split_once("    # ") {
                        return Ok(description_part.trim().to_string());
                    }
                }
            }
        }

        Err(WasmrunError::from("Crate not found in search results"))
    }

    fn copy_plugin_artifacts(source_dir: &Path, dest_dir: &Path) -> Result<()> {
        let target_dir = source_dir.join("target").join("release");

        if !target_dir.exists() {
            return Err(WasmrunError::from(
                "No release build found in target directory",
            ));
        }

        let mut copied_files = 0;

        for entry in std::fs::read_dir(&target_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to read target directory: {}", e)))?
        {
            let entry = entry.map_err(|e| {
                WasmrunError::from(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if ext_str == "so" || ext_str == "dll" || ext_str == "dylib" {
                        let dest_path = dest_dir.join(path.file_name().unwrap());
                        std::fs::copy(&path, &dest_path).map_err(|e| {
                            WasmrunError::from(format!("Failed to copy artifact: {}", e))
                        })?;
                        copied_files += 1;
                    }
                } else if Self::is_executable(&path) {
                    let dest_path = dest_dir.join(path.file_name().unwrap());
                    std::fs::copy(&path, &dest_path).map_err(|e| {
                        WasmrunError::from(format!("Failed to copy artifact: {}", e))
                    })?;
                    copied_files += 1;
                }
            }
        }

        if copied_files == 0 {
            return Err(WasmrunError::from("No plugin artifacts found to copy"));
        }

        Ok(())
    }

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

    #[cfg(windows)]
    fn is_executable(path: &std::path::Path) -> bool {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            ext == "exe" || ext == "bat" || ext == "cmd"
        } else {
            false
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn is_executable(_path: &std::path::Path) -> bool {
        false
    }

    pub fn uninstall(plugin_name: &str) -> Result<()> {
        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(plugin_name);

        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
                WasmrunError::from(format!("Failed to remove plugin directory: {}", e))
            })?;
            println!("Removed plugin directory: {}", plugin_dir.display());
        }

        let cache_dir = WasmrunConfig::cache_dir()?;
        let cache_plugin_dir = cache_dir.join(format!("git-{}", plugin_name));
        if cache_plugin_dir.exists() {
            std::fs::remove_dir_all(&cache_plugin_dir)
                .map_err(|e| WasmrunError::from(format!("Failed to remove plugin cache: {}", e)))?;
        }

        Ok(())
    }
}

// External plugin wrapper for dynamic loading
// TODO: Implement dynamic loading
#[allow(dead_code)]
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
}

#[allow(dead_code)]
impl ExternalPluginWrapper {
    pub fn new(plugin_dir: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let info = Self::load_plugin_info(&plugin_dir, &entry)?;
        let plugin_name = entry.info.name.clone();

        Ok(Self { info, plugin_name })
    }

    fn load_plugin_info(plugin_dir: &Path, entry: &ExternalPluginEntry) -> Result<PluginInfo> {
        let toml_metadata_file = plugin_dir.join("plugin.toml");
        if toml_metadata_file.exists() {
            return Self::load_info_from_toml_metadata(&toml_metadata_file, entry);
        }

        Ok(entry.info.clone())
    }

    fn load_info_from_toml_metadata(
        metadata_file: &Path,
        entry: &ExternalPluginEntry,
    ) -> Result<PluginInfo> {
        let content = std::fs::read_to_string(metadata_file)
            .map_err(|e| WasmrunError::from(format!("Failed to read plugin metadata: {}", e)))?;

        #[derive(Deserialize)]
        #[allow(dead_code)] // TODO: Use when plugin.toml metadata is loaded
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
            .map_err(|e| WasmrunError::from(format!("Failed to parse plugin metadata: {}", e)))?;

        Ok(PluginInfo {
            name: metadata.name,
            version: metadata.version,
            description: metadata.description,
            author: metadata.author.unwrap_or_else(|| "Unknown".to_string()),
            extensions: metadata.extensions.unwrap_or_default(),
            entry_files: metadata.entry_files.unwrap_or_default(),
            plugin_type: PluginType::External,
            source: Some(entry.source.clone()),
            dependencies: metadata.dependencies.unwrap_or_default(),
            capabilities: metadata.capabilities.unwrap_or_default(),
        })
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, _project_path: &str) -> bool {
        // TODO: Implement project detection for external plugins
        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalPluginBuilder {
            plugin_name: self.plugin_name.clone(),
        })
    }
}

pub struct ExternalPluginBuilder {
    plugin_name: String,
}

impl WasmBuilder for ExternalPluginBuilder {
    fn language_name(&self) -> &str {
        "external"
    }

    fn check_dependencies(&self) -> Vec<String> {
        vec![]
    }

    fn entry_file_candidates(&self) -> &[&str] {
        // TODO: Get from plugin metadata
        &[]
    }

    fn supported_extensions(&self) -> &[&str] {
        // TODO: Get from plugin metadata
        &[]
    }

    fn validate_project(&self, _project_path: &str) -> crate::error::CompilationResult<()> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: format!(
                "External plugin '{}' is installed but dynamic loading is not yet implemented.",
                self.plugin_name
            ),
        })
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: format!(
                "External plugin '{}' (dynamic loading not yet implemented)",
                self.plugin_name
            ),
        })
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        self.build(config)
    }
}

// Plugin loader - placeholder for future dynamic loading implementation
// TODO: Implement actual plugin loading via dynamic libraries or subprocess execution
#[allow(dead_code)]
pub struct ExternalPluginLoader;

#[allow(dead_code)]
impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        if !entry.enabled {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is disabled",
                entry.info.name
            )));
        }

        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(&entry.install_path);

        if !plugin_dir.exists() {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not installed. Run 'wasmrun plugin install {}'",
                entry.info.name, entry.info.name
            )));
        }

        let plugin = ExternalPluginWrapper::new(plugin_dir, entry.clone())?;
        Ok(Box::new(plugin))
    }
}
