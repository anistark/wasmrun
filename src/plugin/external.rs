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

// TODO: Dynamic plugin loading
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

// TODO: Dynamic plugin wrapper
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    #[allow(dead_code)]
    entry: ExternalPluginEntry,
    #[allow(dead_code)]
    plugin_dir: PathBuf,
    plugin_name: String,
}

#[allow(dead_code)]
impl ExternalPluginWrapper {
    pub fn new(plugin_dir: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let info = Self::load_plugin_info(&plugin_dir, &entry)?;
        let plugin_name = entry.info.name.clone();

        Ok(Self {
            info,
            entry,
            plugin_dir,
            plugin_name,
        })
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
        #[allow(dead_code)]
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

// TODO: Future plugin utilities - These will be used when dynamic loading is implemented
#[allow(dead_code)]
pub fn install_plugin(source: PluginSource) -> Result<Box<dyn Plugin>> {
    let plugin_dir = PluginInstaller::install(source.clone())?;

    let plugin_name = plugin_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let entry = ExternalPluginEntry {
        info: PluginInfo {
            name: plugin_name,
            version: "1.0.0".to_string(),
            description: "External plugin".to_string(),
            author: "Unknown".to_string(),
            extensions: vec![],
            entry_files: vec![],
            plugin_type: PluginType::External,
            source: Some(source),
            dependencies: vec![],
            capabilities: PluginCapabilities::default(),
        },
        source: PluginSource::Local {
            path: plugin_dir.clone(),
        },
        installed_at: chrono::Utc::now().to_rfc3339(),
        enabled: true,
        install_path: plugin_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        executable_path: None,
    };

    ExternalPluginLoader::load(&entry)
}

#[allow(dead_code)]
pub fn load_external_plugin(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
    ExternalPluginLoader::load(entry)
}

#[allow(dead_code)]
pub fn validate_plugin(plugin_dir: &PathBuf) -> Result<()> {
    if !plugin_dir.exists() {
        return Err(WasmrunError::from("Plugin directory does not exist"));
    }

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
        return Err(WasmrunError::from(
            "No executable or library files found in plugin directory",
        ));
    }

    Ok(())
}
