use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationResult, Result, WasmrunError};
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};

#[cfg(not(target_os = "windows"))]
use crate::plugin::bridge::{symbols::*, PluginSymbols};
#[cfg(not(target_os = "windows"))]
use libloading::Library;

pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl ExternalPluginWrapper {
    pub fn new(_plugin_path: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let plugin_name = entry.info.name.clone();

        if !Self::is_plugin_available(&plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' not available"
            )));
        }

        #[cfg(not(target_os = "windows"))]
        let library = Self::try_load_library(&plugin_name)?;

        Ok(Self {
            info: entry.info,
            plugin_name,
            #[cfg(not(target_os = "windows"))]
            library,
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn try_load_library(plugin_name: &str) -> Result<Option<Arc<Library>>> {
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;

        let lib_extensions = ["so", "dylib"];

        for ext in &lib_extensions {
            let path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
            if path.exists() {
                unsafe {
                    match Library::new(&path) {
                        Ok(library) => {
                            let symbols = PluginSymbols::get_symbol_names(plugin_name);
                            if library
                                .get::<CreateBuilderFn>(symbols.create_builder)
                                .is_ok()
                            {
                                return Ok(Some(Arc::new(library)));
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn is_plugin_available(plugin_name: &str) -> bool {
        // Check if plugin directory exists with proper structure
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            if plugin_dir.exists() {
                // Primary check: Cargo.toml with plugin metadata
                let cargo_toml_path = plugin_dir.join("Cargo.toml");
                if cargo_toml_path.exists() {
                    // Verify it's a wasmrun plugin by checking metadata
                    if Self::is_valid_wasmrun_plugin(&cargo_toml_path) {
                        return true;
                    }
                    return true;
                }

                // Secondary check: manifest file
                let manifest_path = plugin_dir.join("wasmrun.toml");
                if manifest_path.exists() {
                    return true;
                }

                // Tertiary check: metadata file
                let metadata_path = plugin_dir.join(".wasmrun_metadata");
                if metadata_path.exists() {
                    return true;
                }

                // Quaternary check: shared library files (for dynamic loading)
                // TODO: Implement library builds
                let lib_extensions = ["so", "dylib", "dll"];
                for ext in &lib_extensions {
                    let lib_path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
                    if lib_path.exists() {
                        return true;
                    }
                }

                // If plugin directory exists with src/ folder, consider it available
                let src_path = plugin_dir.join("src");
                if src_path.exists() && src_path.is_dir() {
                    return true;
                }
            }
        }

        false
    }

    /// Verify if a Cargo.toml belongs to a wasmrun plugin
    fn is_valid_wasmrun_plugin(cargo_toml_path: &std::path::Path) -> bool {
        if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
            // Check for wasmrun plugin markers
            content.contains("[wasm_plugin]")
                || content.contains("wasm-bindgen")
                || content.contains("tinygo")
        } else {
            false
        }
    }

    /// Get plugin directory path
    fn get_plugin_directory(plugin_name: &str) -> Result<std::path::PathBuf> {
        use crate::plugin::config::WasmrunConfig;
        let config_dir = WasmrunConfig::config_dir()?;
        Ok(config_dir.join("plugins").join(plugin_name))
    }

    fn check_project_via_binary(&self, project_path: &str) -> bool {
        match &self.plugin_name as &str {
            "wasmrust" => {
                // Check for Cargo.toml
                Path::new(project_path).join("Cargo.toml").exists()
            }
            "wasmgo" => {
                // Check for go.mod or .go files
                Path::new(project_path).join("go.mod").exists() || self.has_go_files(project_path)
            }
            _ => false,
        }
    }

    fn check_project_via_manifest(&self, project_path: &str) -> bool {
        // Basic file extension checking based on plugin info
        let path = Path::new(project_path);

        // Check entry files
        for entry_file in &self.info.entry_files {
            if path.join(entry_file).exists() {
                return true;
            }
        }

        // Check extensions for files in directory
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if let Some(ext_str) = ext.to_str() {
                        if self.info.extensions.contains(&ext_str.to_string()) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    fn has_go_files(&self, project_path: &str) -> bool {
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "go" {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        if Self::is_plugin_available(&self.plugin_name) {
            return self.check_project_via_binary(project_path);
        }
        self.check_project_via_manifest(project_path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder {
            plugin_name: self.plugin_name.clone(),
            #[cfg(not(target_os = "windows"))]
            library: self.library.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ExternalWasmBuilder {
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl WasmBuilder for ExternalWasmBuilder {
    fn can_handle_project(&self, project_path: &str) -> bool {
        ExternalPluginWrapper::is_plugin_available(&self.plugin_name)
            && match &self.plugin_name as &str {
                "wasmrust" => Path::new(project_path).join("Cargo.toml").exists(),
                "wasmgo" => {
                    Path::new(project_path).join("go.mod").exists()
                        || self.has_go_files_in_project(project_path)
                }
                _ => false,
            }
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Call the plugin binary directly if available
        self.build_via_binary(config)
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        // Call plugin-specific clean command
        match &self.plugin_name as &str {
            "wasmrust" => {
                let output = Command::new("cargo")
                    .args(["clean"])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| {
                        WasmrunError::from(format!("Failed to clean Rust project: {e}"))
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Clean failed: {stderr}")));
                }
            }
            "wasmgo" => {
                let output = Command::new("go")
                    .args(["clean"])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| WasmrunError::from(format!("Failed to clean Go project: {e}")))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Clean failed: {stderr}")));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder {
            plugin_name: self.plugin_name.clone(),
            #[cfg(not(target_os = "windows"))]
            library: self.library.clone(),
        })
    }

    fn language_name(&self) -> &str {
        &self.plugin_name
    }

    fn entry_file_candidates(&self) -> &[&str] {
        match &self.plugin_name as &str {
            "wasmrust" => &["Cargo.toml", "src/main.rs", "src/lib.rs"],
            "wasmgo" => &["go.mod", "main.go"],
            _ => &[],
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        match &self.plugin_name as &str {
            "wasmrust" => &["rs", "toml"],
            "wasmgo" => &["go", "mod"],
            _ => &[],
        }
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        match &self.plugin_name as &str {
            "wasmrust" => {
                if !self.is_tool_available("cargo") {
                    missing.push("cargo".to_string());
                }
                if !self.is_tool_available("rustc") {
                    missing.push("rustc".to_string());
                }
                if !self.is_wasm_target_installed() {
                    missing.push("wasm32-unknown-unknown target".to_string());
                }
            }
            "wasmgo" => {
                if !self.is_tool_available("tinygo") {
                    missing.push("tinygo".to_string());
                }
            }
            _ => {}
        }

        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        if !self.can_handle_project(project_path) {
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!(
                    "Project at '{}' cannot be handled by {} plugin",
                    project_path, self.plugin_name
                ),
            });
        }
        Ok(())
    }
}

impl ExternalWasmBuilder {
    fn build_via_binary(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        // TODO: Implement actual binary-based compilation for external plugins
        Err(crate::error::CompilationError::BuildFailed {
            language: self.plugin_name.clone(),
            reason: "External plugin compilation via binary not yet implemented".to_string(),
        })
    }

    fn has_go_files_in_project(&self, project_path: &str) -> bool {
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "go" {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn is_tool_available(&self, tool: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        Command::new(which_cmd)
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Check if wasm target is installed for Rust
    fn is_wasm_target_installed(&self) -> bool {
        Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("wasm32-unknown-unknown")
            })
            .unwrap_or(false)
    }
}

pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        let wrapper = ExternalPluginWrapper::new(PathBuf::new(), entry.clone())?;
        Ok(Box::new(wrapper))
    }

    /// Get plugin directory path (public static method)
    pub fn get_plugin_directory(plugin_name: &str) -> Result<std::path::PathBuf> {
        use crate::plugin::config::WasmrunConfig;
        let config_dir = WasmrunConfig::config_dir()?;
        Ok(config_dir.join("plugins").join(plugin_name))
    }

    /// Check if plugin is available by looking for library files instead of binary
    #[allow(dead_code)]
    pub fn is_plugin_available(plugin_name: &str) -> bool {
        // Check if plugin directory exists with proper structure
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            if plugin_dir.exists() {
                // Primary check: Cargo.toml with plugin metadata
                let cargo_toml_path = plugin_dir.join("Cargo.toml");
                if cargo_toml_path.exists() {
                    if Self::is_valid_wasmrun_plugin(&cargo_toml_path) {
                        return true;
                    }
                    return true;
                }

                // Secondary check: manifest file
                let manifest_path = plugin_dir.join("wasmrun.toml");
                if manifest_path.exists() {
                    return true;
                }

                // Tertiary check: metadata file
                let metadata_path = plugin_dir.join(".wasmrun_metadata");
                if metadata_path.exists() {
                    return true;
                }

                // Quaternary check: shared library files (for dynamic loading)
                let lib_extensions = ["so", "dylib", "dll"];
                for ext in &lib_extensions {
                    let lib_path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
                    if lib_path.exists() {
                        return true;
                    }
                }

                // If plugin directory exists with src/ folder, consider it available
                let src_path = plugin_dir.join("src");
                if src_path.exists() && src_path.is_dir() {
                    return true;
                }
            }
        }

        false
    }

    /// Verify if a Cargo.toml belongs to a wasm plugin
    fn is_valid_wasmrun_plugin(cargo_toml_path: &std::path::Path) -> bool {
        if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
            content.contains("[wasm_plugin]")
                || content.contains("wasm-bindgen")
                || content.contains("tinygo")
        } else {
            false
        }
    }

    pub fn create_wasmrust_entry() -> ExternalPluginEntry {
        let version = detect_wasmrust_version().unwrap_or_else(|| "0.2.1".to_string());

        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmrust".to_string(),
                version: version.clone(),
                description: "Rust to WebAssembly compiler with wasm-bindgen support".to_string(),
                author: "Kumar Anirudha".to_string(),
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: true,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec![
                        "wasm32-unknown-unknown".to_string(),
                        "wasm32-wasi".to_string(),
                    ],
                },
                extensions: vec!["rs".to_string(), "toml".to_string()],
                entry_files: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmrust".to_string(),
                    version: version.clone(),
                }),
                dependencies: vec!["cargo".to_string(), "rustc".to_string()],
            },
            source: PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: version.clone(),
            },
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            install_path: String::new(),
            executable_path: None,
            enabled: true,
        }
    }

    pub fn create_wasmgo_entry() -> ExternalPluginEntry {
        let version = detect_wasmgo_version().unwrap_or_else(|| "0.1.0".to_string());

        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmgo".to_string(),
                version: version.clone(),
                description: "Go to WebAssembly compiler using TinyGo".to_string(),
                author: "Kumar Anirudha".to_string(),
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: false,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec!["wasm".to_string()],
                },
                extensions: vec!["go".to_string(), "mod".to_string()],
                entry_files: vec!["go.mod".to_string(), "main.go".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmgo".to_string(),
                    version: version.clone(),
                }),
                dependencies: vec!["tinygo".to_string()],
            },
            source: PluginSource::CratesIo {
                name: "wasmgo".to_string(),
                version: version.clone(),
            },
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            install_path: String::new(),
            executable_path: None,
            enabled: true,
        }
    }
}

fn detect_wasmrust_version() -> Option<String> {
    if let Ok(plugin_dir) = ExternalPluginLoader::get_plugin_directory("wasmrust") {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                    if let Some(package) = parsed.get("package") {
                        if let Some(version) = package.get("version") {
                            if let Some(version_str) = version.as_str() {
                                return Some(version_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok(plugin_dir) = ExternalPluginLoader::get_plugin_directory("wasmrust") {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("version") && line.contains('=') {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                let version = &line[start + 1..start + 1 + end];
                                return Some(version.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    get_latest_crates_version("wasmrust")
}

fn detect_wasmgo_version() -> Option<String> {
    if let Ok(plugin_dir) = ExternalPluginLoader::get_plugin_directory("wasmgo") {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                    if let Some(package) = parsed.get("package") {
                        if let Some(version) = package.get("version") {
                            if let Some(version_str) = version.as_str() {
                                return Some(version_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok(plugin_dir) = ExternalPluginLoader::get_plugin_directory("wasmgo") {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("version") && line.contains('=') {
                        if let Some(start) = line.find('"') {
                            if let Some(end) = line[start + 1..].find('"') {
                                let version = &line[start + 1..start + 1 + end];
                                return Some(version.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    get_latest_crates_version("wasmgo")
}

fn get_latest_crates_version(crate_name: &str) -> Option<String> {
    if let Ok(output) = std::process::Command::new("cargo")
        .args(["search", crate_name, "--limit", "1"])
        .output()
    {
        if output.status.success() {
            let search_output = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = search_output.lines().next() {
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let version = &line[start + 1..start + 1 + end];
                        return Some(version.to_string());
                    }
                }
            }
        }
    }

    None
}
