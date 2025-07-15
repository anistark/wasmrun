//! External plugin system with dynamic loading support

use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult, Result, WasmrunError};
use crate::plugin::config::{ExternalPluginEntry, WasmrunConfig};
use crate::plugin::protocol::{PluginProtocol, PluginRequest, PluginResponse};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ExternalPluginInstaller;

impl ExternalPluginInstaller {
    pub fn install(plugin_source: &PluginSource, name: &str, force: bool) -> Result<()> {
        let install_dir = WasmrunConfig::plugin_dir()?;
        let plugin_dir = install_dir.join(name);

        if plugin_dir.exists() && !force {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is already installed. Use --force to overwrite",
                name
            )));
        }

        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
                WasmrunError::from(format!("Failed to remove existing plugin: {}", e))
            })?;
        }

        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {}", e)))?;

        match plugin_source {
            PluginSource::CratesIo { name, version } => {
                Self::install_from_crates_io(name, version, &plugin_dir)?;
            }
            PluginSource::Git { url, branch } => {
                Self::install_from_git(url, branch.as_deref(), &plugin_dir)?;
            }
            PluginSource::Local { path } => {
                Self::install_from_local(path, &plugin_dir)?;
            }
        }

        println!("✅ Plugin '{}' installed successfully", name);
        Ok(())
    }

    fn install_from_crates_io(crate_name: &str, version: &str, plugin_dir: &Path) -> Result<()> {
        println!("📦 Installing from crates.io: {}@{}", crate_name, version);

        let plugin_dir_str = plugin_dir.to_string_lossy();
        let mut args = vec!["install", crate_name, "--root"];
        args.push(&plugin_dir_str);

        if version != "*" {
            args.extend(["--version", version]);
        }

        let output = Command::new("cargo")
            .args(&args)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run cargo install: {}", e)))?;

        if !output.status.success() {
            return Err(WasmrunError::from(format!(
                "Cargo install failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    fn install_from_git(url: &str, branch: Option<&str>, plugin_dir: &Path) -> Result<()> {
        println!("🌐 Installing from Git: {}", url);

        let cache_dir = WasmrunConfig::cache_dir()?;
        let git_cache = cache_dir.join(format!("git-{}", url.replace('/', "-")));

        if git_cache.exists() {
            std::fs::remove_dir_all(&git_cache)
                .map_err(|e| WasmrunError::from(format!("Failed to clean git cache: {}", e)))?;
        }

        let git_cache_str = git_cache.to_string_lossy();
        let mut git_args = vec!["clone", url, &git_cache_str];
        if let Some(branch) = branch {
            git_args.extend(["--branch", branch]);
        }

        let clone_output = Command::new("git")
            .args(&git_args)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to run git clone: {}", e)))?;

        if !clone_output.status.success() {
            return Err(WasmrunError::from(format!(
                "Git clone failed: {}",
                String::from_utf8_lossy(&clone_output.stderr)
            )));
        }

        Self::build_plugin(&git_cache, plugin_dir)?;
        Ok(())
    }

    fn install_from_local(local_path: &Path, plugin_dir: &Path) -> Result<()> {
        println!("📁 Installing from local path: {}", local_path.display());

        if !local_path.exists() {
            return Err(WasmrunError::from(format!(
                "Local path does not exist: {}",
                local_path.display()
            )));
        }

        Self::copy_dir_recursive(local_path, plugin_dir)?;

        if plugin_dir.join("Cargo.toml").exists() {
            Self::build_plugin(plugin_dir, plugin_dir)?;
        }

        Ok(())
    }

    fn build_plugin(source_dir: &Path, install_dir: &Path) -> Result<()> {
        let cargo_toml = source_dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            if source_dir != install_dir {
                Self::copy_dir_recursive(source_dir, install_dir)?;
            }
            return Ok(());
        }

        println!("🔨 Building plugin...");

        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(source_dir)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to build plugin: {}", e)))?;

        if !build_output.status.success() {
            return Err(WasmrunError::from(format!(
                "Plugin build failed: {}",
                String::from_utf8_lossy(&build_output.stderr)
            )));
        }

        let target_dir = source_dir.join("target").join("release");
        if target_dir.exists() {
            Self::copy_plugin_artifacts(&target_dir, install_dir)?;
        }

        Self::copy_dir_recursive(source_dir, install_dir)?;

        Ok(())
    }

    fn copy_plugin_artifacts(target_dir: &Path, dest_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dest_dir).map_err(|e| {
            WasmrunError::from(format!("Failed to create destination directory: {}", e))
        })?;

        let bin_dir = dest_dir.join("bin");
        std::fs::create_dir_all(&bin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create bin directory: {}", e)))?;

        if let Ok(entries) = std::fs::read_dir(target_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && Self::is_executable(&path) {
                    let dest_path = bin_dir.join(path.file_name().unwrap());
                    std::fs::copy(&path, &dest_path).map_err(|e| {
                        WasmrunError::from(format!("Failed to copy executable: {}", e))
                    })?;
                }
            }
        }

        Ok(())
    }

    fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
        std::fs::create_dir_all(dest).map_err(|e| {
            WasmrunError::from(format!(
                "Failed to create directory {}: {}",
                dest.display(),
                e
            ))
        })?;

        if let Ok(entries) = std::fs::read_dir(src) {
            for entry in entries.flatten() {
                let src_path = entry.path();
                let dest_path = dest.join(entry.file_name());

                if src_path.is_dir() {
                    let dir_name = src_path.file_name().unwrap().to_string_lossy();
                    if dir_name == "target" || dir_name == ".git" {
                        continue;
                    }
                    Self::copy_dir_recursive(&src_path, &dest_path)?;
                } else {
                    std::fs::copy(&src_path, &dest_path)
                        .map_err(|e| WasmrunError::from(format!("Failed to copy file: {}", e)))?;
                }
            }
        }

        Ok(())
    }

    #[cfg(unix)]
    fn is_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let permissions = metadata.permissions();
            permissions.mode() & 0o111 != 0
        } else {
            false
        }
    }

    #[cfg(windows)]
    fn is_executable(path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            ext == "exe" || ext == "bat" || ext == "cmd"
        } else {
            false
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn is_executable(_path: &Path) -> bool {
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

/// Dynamic plugin wrapper
pub struct DynamicPlugin {
    protocol: PluginProtocol,
    info: PluginInfo,
}

impl DynamicPlugin {
    pub fn new(executable_path: String, info: PluginInfo) -> Self {
        let protocol = PluginProtocol::new(executable_path);
        Self { protocol, info }
    }
}

impl Plugin for DynamicPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        match self.protocol.send_request(PluginRequest::CanHandle {
            project_path: project_path.to_string(),
        }) {
            Ok(PluginResponse::CanHandle { can_handle }) => can_handle,
            _ => false,
        }
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(DynamicPluginBuilder {
            protocol: PluginProtocol::new(self.protocol.executable_path.clone()),
            info: self.info.clone(),
        })
    }
}

/// Builder implementation for dynamic plugins
pub struct DynamicPluginBuilder {
    protocol: PluginProtocol,
    info: PluginInfo,
}

impl WasmBuilder for DynamicPluginBuilder {
    fn language_name(&self) -> &str {
        &self.info.name
    }

    fn check_dependencies(&self) -> Vec<String> {
        match self.protocol.send_request(PluginRequest::CheckDependencies) {
            Ok(PluginResponse::Dependencies { missing }) => missing,
            _ => vec!["Failed to check plugin dependencies".to_string()],
        }
    }

    fn entry_file_candidates(&self) -> &[&str] {
        static EMPTY: &[&str] = &[];
        EMPTY
    }

    fn supported_extensions(&self) -> &[&str] {
        static EMPTY: &[&str] = &[];
        EMPTY
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        match self.protocol.send_request(PluginRequest::Validate {
            project_path: project_path.to_string(),
        }) {
            Ok(PluginResponse::Validate { valid, errors }) => {
                if valid {
                    Ok(())
                } else {
                    Err(CompilationError::InvalidProjectStructure {
                        language: self.language_name().to_string(),
                        reason: errors.join("; "),
                    })
                }
            }
            Ok(PluginResponse::Error { message }) => Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: message,
            }),
            Err(e) => Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Plugin communication error: {}", e),
            }),
            _ => Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "Unexpected response from plugin".to_string(),
            }),
        }
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let external_config = config.into();

        match self.protocol.send_request(PluginRequest::Build {
            config: external_config,
        }) {
            Ok(PluginResponse::Build { result }) => match result {
                Ok(external_result) => Ok(external_result.into()),
                Err(error_msg) => Err(CompilationError::BuildFailed {
                    language: self.language_name().to_string(),
                    reason: error_msg,
                }),
            },
            Ok(PluginResponse::Error { message }) => Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: message,
            }),
            Err(e) => Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Plugin communication error: {}", e),
            }),
            _ => Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "Unexpected response from plugin".to_string(),
            }),
        }
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!("🔨 Building with external plugin: {}", self.info.name);
        println!("📂 Project path: {}", config.project_path);
        println!("📦 Output directory: {}", config.output_dir);

        let result = self.build(config)?;

        println!("✅ {} build completed successfully", self.info.name);
        println!("📦 WASM file: {}", result.wasm_path);

        if let Some(js_path) = &result.js_path {
            println!("📝 JS file: {}", js_path);
        }

        if !result.additional_files.is_empty() {
            println!(
                "📋 Additional files: {}",
                result.additional_files.join(", ")
            );
        }

        Ok(result)
    }
}

// TODO: Figure out a better way to do this. Open an issue?
pub fn find_plugin_executable(plugin_dir: &Path, plugin_name: &str) -> Result<String> {
    let candidates = vec![
        plugin_dir.join(plugin_name),
        plugin_dir.join(format!("{}.exe", plugin_name)),
        plugin_dir.join("bin").join(plugin_name),
        plugin_dir.join("bin").join(format!("{}.exe", plugin_name)),
        plugin_dir.join("target").join("release").join(plugin_name),
        plugin_dir
            .join("target")
            .join("release")
            .join(format!("{}.exe", plugin_name)),
    ];

    for candidate in candidates {
        if candidate.exists() && is_executable(&candidate) {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    if let Ok(entries) = std::fs::read_dir(plugin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_executable(&path) {
                let protocol = PluginProtocol::new(path.to_string_lossy().to_string());
                if protocol.is_wasmrun_plugin() {
                    return Ok(path.to_string_lossy().to_string());
                }
            }
        }
    }

    Err(WasmrunError::from(format!(
        "No executable found for plugin '{}' in {}",
        plugin_name,
        plugin_dir.display()
    )))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        ext == "exe" || ext == "bat" || ext == "cmd"
    } else {
        false
    }
}

#[cfg(not(any(unix, windows)))]
fn is_executable(_path: &Path) -> bool {
    false
}

// External plugin wrapper for dynamic loading
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
    executable_path: Option<String>,
}

impl ExternalPluginWrapper {
    pub fn new(plugin_dir: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let info = Self::load_plugin_info(&plugin_dir, &entry)?;
        let plugin_name = entry.info.name.clone();

        let executable_path = find_plugin_executable(&plugin_dir, &plugin_name).ok();

        Ok(Self {
            info,
            plugin_name,
            executable_path,
        })
    }

    fn load_plugin_info(plugin_dir: &Path, entry: &ExternalPluginEntry) -> Result<PluginInfo> {
        if let Ok(executable) = find_plugin_executable(plugin_dir, &entry.info.name) {
            let protocol = PluginProtocol::new(executable);
            if protocol.is_wasmrun_plugin() {
                if let Ok(info) = protocol.get_info() {
                    return Ok(info);
                }
            }
        }

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

    fn can_handle_project(&self, project_path: &str) -> bool {
        if let Some(ref executable_path) = self.executable_path {
            let protocol = PluginProtocol::new(executable_path.clone());
            if let Ok(PluginResponse::CanHandle { can_handle }) =
                protocol.send_request(PluginRequest::CanHandle {
                    project_path: project_path.to_string(),
                })
            {
                return can_handle;
            }
        }

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

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        if let Some(ref executable_path) = self.executable_path {
            Box::new(DynamicPluginBuilder {
                protocol: PluginProtocol::new(executable_path.clone()),
                info: self.info.clone(),
            })
        } else {
            Box::new(ExternalPluginBuilder {
                plugin_name: self.plugin_name.clone(),
                info: self.info.clone(),
            })
        }
    }
}

// TODO: Remove once dynamic external plugin is stable
pub struct ExternalPluginBuilder {
    plugin_name: String,
    info: PluginInfo,
}

impl WasmBuilder for ExternalPluginBuilder {
    fn language_name(&self) -> &str {
        &self.plugin_name
    }

    fn check_dependencies(&self) -> Vec<String> {
        self.info.dependencies.clone()
    }

    fn entry_file_candidates(&self) -> &[&str] {
        static EMPTY: &[&str] = &[];
        EMPTY
    }

    fn supported_extensions(&self) -> &[&str] {
        static EMPTY: &[&str] = &[];
        EMPTY
    }

    fn validate_project(&self, _project_path: &str) -> CompilationResult<()> {
        Err(CompilationError::UnsupportedLanguage {
            language: format!(
                "External plugin '{}' is installed but dynamic loading is not available for this plugin.",
                self.plugin_name
            ),
        })
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(CompilationError::UnsupportedLanguage {
            language: format!(
                "External plugin '{}' (dynamic loading not available)",
                self.plugin_name
            ),
        })
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        self.build(config)
    }
}

// Plugin loader
pub struct ExternalPluginLoader;

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

        if let Ok(executable_path) = find_plugin_executable(&plugin_dir, &entry.info.name) {
            let protocol = PluginProtocol::new(executable_path.clone());
            if protocol.is_wasmrun_plugin() {
                if let Ok(info) = protocol.get_info() {
                    return Ok(Box::new(DynamicPlugin::new(executable_path, info)));
                }
            }
        }

        let plugin = ExternalPluginWrapper::new(plugin_dir, entry.clone())?;
        Ok(Box::new(plugin))
    }
}

/// Get plugin metadata from a plugin directory
pub fn detect_plugin_metadata(
    plugin_dir: &Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    if let Ok(executable_path) = find_plugin_executable(plugin_dir, plugin_name) {
        let protocol = PluginProtocol::new(executable_path);
        if protocol.is_wasmrun_plugin() {
            if let Ok(info) = protocol.get_info() {
                return Ok(info);
            }
        }
    }

    let config_path = plugin_dir.join("plugin.toml");
    if config_path.exists() {
        return detect_from_plugin_toml(&config_path, plugin_name, source);
    }

    let cargo_toml = plugin_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        return detect_from_cargo_toml(&cargo_toml, plugin_name, source);
    }

    Ok(PluginInfo {
        name: plugin_name.to_string(),
        version: "0.1.0".to_string(),
        description: "External plugin".to_string(),
        author: "Unknown".to_string(),
        extensions: vec![],
        entry_files: vec![],
        plugin_type: PluginType::External,
        source: Some(source.clone()),
        dependencies: vec![],
        capabilities: PluginCapabilities::default(),
    })
}

fn detect_from_plugin_toml(
    config_path: &Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read plugin.toml: {}", e)))?;

    #[derive(Deserialize)]
    struct PluginToml {
        name: Option<String>,
        version: Option<String>,
        description: Option<String>,
        author: Option<String>,
        extensions: Option<Vec<String>>,
        entry_files: Option<Vec<String>>,
        dependencies: Option<Vec<String>>,
        capabilities: Option<PluginCapabilities>,
    }

    let config: PluginToml = toml::from_str(&content)
        .map_err(|e| WasmrunError::from(format!("Failed to parse plugin.toml: {}", e)))?;

    Ok(PluginInfo {
        name: config.name.unwrap_or_else(|| plugin_name.to_string()),
        version: config.version.unwrap_or_else(|| "0.1.0".to_string()),
        description: config
            .description
            .unwrap_or_else(|| "External plugin".to_string()),
        author: config.author.unwrap_or_else(|| "Unknown".to_string()),
        extensions: config.extensions.unwrap_or_default(),
        entry_files: config.entry_files.unwrap_or_default(),
        plugin_type: PluginType::External,
        source: Some(source.clone()),
        dependencies: config.dependencies.unwrap_or_default(),
        capabilities: config.capabilities.unwrap_or_default(),
    })
}

fn detect_from_cargo_toml(
    cargo_path: &Path,
    plugin_name: &str,
    source: &PluginSource,
) -> Result<PluginInfo> {
    let content = std::fs::read_to_string(cargo_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read Cargo.toml: {}", e)))?;

    #[derive(Deserialize)]
    struct CargoPackage {
        name: Option<String>,
        version: Option<String>,
        description: Option<String>,
        authors: Option<Vec<String>>,
    }

    #[derive(Deserialize)]
    struct CargoToml {
        package: Option<CargoPackage>,
    }

    let cargo: CargoToml = toml::from_str(&content)
        .map_err(|e| WasmrunError::from(format!("Failed to parse Cargo.toml: {}", e)))?;

    let package = cargo.package.unwrap_or(CargoPackage {
        name: None,
        version: None,
        description: None,
        authors: None,
    });

    Ok(PluginInfo {
        name: package.name.unwrap_or_else(|| plugin_name.to_string()),
        version: package.version.unwrap_or_else(|| "0.1.0".to_string()),
        description: package
            .description
            .unwrap_or_else(|| "External plugin".to_string()),
        author: package
            .authors
            .and_then(|authors| authors.first().cloned())
            .unwrap_or_else(|| "Unknown".to_string()),
        extensions: vec![],
        entry_files: vec![],
        plugin_type: PluginType::External,
        source: Some(source.clone()),
        dependencies: vec![],
        capabilities: PluginCapabilities::default(),
    })
}
