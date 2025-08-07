use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationResult, Result, WasmrunError};
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};

#[cfg(not(target_os = "windows"))]
use libloading::Library;
#[cfg(not(target_os = "windows"))]
use crate::plugin::bridge::{PluginSymbols, symbols::*};

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
            return Err(WasmrunError::from(format!("Plugin '{}' not available", plugin_name)));
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
            let path = plugin_dir.join(format!("lib{}.{}", plugin_name, ext));
            if path.exists() {
                unsafe {
                    match Library::new(&path) {
                        Ok(library) => {
                            let symbols = PluginSymbols::get_symbol_names(plugin_name);
                            if library.get::<CreateBuilderFn>(symbols.create_builder).is_ok() {
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
                // Check for Cargo.toml with plugin metadata
                let cargo_toml_path = plugin_dir.join("Cargo.toml");
                if cargo_toml_path.exists() {
                    // Verify it's a wasm plugin by checking metadata
                    if Self::is_valid_wasm_plugin(&cargo_toml_path) {
                        return true;
                    }
                }

                // Check for manifest file
                let manifest_path = plugin_dir.join("wasmrun.toml");
                if manifest_path.exists() {
                    return true;
                }

                // Check for metadata file
                let metadata_path = plugin_dir.join(".wasmrun_metadata");
                if metadata_path.exists() {
                    return true;
                }

                // Check for shared library files
                let lib_extensions = ["so", "dylib", "dll"];
                for ext in &lib_extensions {
                    let lib_path = plugin_dir.join(format!("lib{}.{}", plugin_name, ext));
                    if lib_path.exists() {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn is_valid_wasm_plugin(cargo_toml_path: &Path) -> bool {
        if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
            // Check if it has wasm-plugin metadata
            content.contains("[package.metadata.wasm-plugin]") ||
            content.contains("crate-type") && (content.contains("cdylib") || content.contains("rlib"))
        } else {
            false
        }
    }

    fn get_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not determine home directory"))?;
        Ok(home_dir.join(".wasmrun").join("plugins").join(plugin_name))
    }

    fn is_plugin_binary_available(plugin_name: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        std::process::Command::new(which_cmd)
            .arg(plugin_name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Use library-based project detection
        if Self::is_plugin_available(&self.plugin_name) {
            return self.check_project_via_binary(project_path);
        }

        // Fallback to basic file extension checking
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

impl ExternalPluginWrapper {
    fn check_project_via_binary(&self, project_path: &str) -> bool {
        // Since plugins are libraries, we check based on project structure
        // rather than calling a binary
        match &self.plugin_name as &str {
            "wasmrust" => {
                // Check for Cargo.toml
                Path::new(project_path).join("Cargo.toml").exists()
            }
            "wasmgo" => {
                // Check for go.mod or .go files
                Path::new(project_path).join("go.mod").exists() ||
                self.has_go_files(project_path)
            }
            _ => false
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

#[derive(Clone)]
pub struct ExternalWasmBuilder {
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl WasmBuilder for ExternalWasmBuilder {
    fn can_handle_project(&self, project_path: &str) -> bool {
        ExternalPluginWrapper::is_plugin_available(&self.plugin_name) && 
        match &self.plugin_name as &str {
            "wasmrust" => Path::new(project_path).join("Cargo.toml").exists(),
            "wasmgo" => Path::new(project_path).join("go.mod").exists() || 
                       self.has_go_files_in_project(project_path),
            _ => false,
        }
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Try to call the plugin binary directly for compilation
        self.build_via_binary(config)
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        // Call plugin-specific clean command
        match &self.plugin_name as &str {
            "wasmrust" => {
                let output = Command::new("cargo")
                    .args(&["clean"])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| WasmrunError::from(format!("Failed to clean Rust project: {}", e)))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Clean failed: {}", stderr)));
                }
            }
            "wasmgo" => {
                let output = Command::new("go")
                    .args(&["clean"])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| WasmrunError::from(format!("Failed to clean Go project: {}", e)))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Clean failed: {}", stderr)));
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
                reason: format!("Project at '{}' cannot be handled by {} plugin", project_path, self.plugin_name),
            });
        }
        Ok(())
    }
}

impl ExternalWasmBuilder {
    fn build_via_binary(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        // For now, return a basic error since we need to implement
        // the actual plugin binary communication protocol
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

    fn is_wasm_target_installed(&self) -> bool {
        Command::new("rustup")
            .args(&["target", "list", "--installed"])
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
                    custom_targets: vec!["wasm32-unknown-unknown".to_string(), "wasm32-wasi".to_string()],
                },
                extensions: vec!["rs".to_string(), "toml".to_string()],
                entry_files: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                dependencies: vec!["cargo".to_string(), "rustc".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmrust".to_string(),
                    version: version.clone(),
                }),
            },
            enabled: true,
            source: PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version,
            },
            installed_at: chrono::Utc::now().to_rfc3339(),
            install_path: get_plugin_install_path("wasmrust"),
            executable_path: Some("wasmrust".to_string()),
        }
    }

    pub fn create_wasmgo_entry() -> ExternalPluginEntry {
        let version = detect_wasmgo_version().unwrap_or_else(|| "latest".to_string());
        
        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmgo".to_string(),
                version: version.clone(),
                description: "Go to WebAssembly compiler using TinyGo".to_string(),
                author: "wasmrun".to_string(),
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: false,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec!["wasm".to_string()],
                },
                extensions: vec!["go".to_string(), "mod".to_string()],
                entry_files: vec!["go.mod".to_string(), "main.go".to_string()],
                dependencies: vec!["tinygo".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmgo".to_string(),
                    version: version.clone(),
                }),
            },
            enabled: true,
            source: PluginSource::CratesIo {
                name: "wasmgo".to_string(),
                version,
            },
            installed_at: chrono::Utc::now().to_rfc3339(),
            install_path: get_plugin_install_path("wasmgo"),
            executable_path: Some("tinygo".to_string()),
        }
    }
}

fn detect_wasmrust_version() -> Option<String> {
    // Try to get version from the binary
    if let Ok(output) = std::process::Command::new("wasmrust")
        .arg("--version")
        .output() 
    {
        if output.status.success() {
            let version_output = String::from_utf8_lossy(&output.stdout);
            if let Some(version_line) = version_output.lines().next() {
                let parts: Vec<&str> = version_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    return Some(parts[1].to_string());
                }
            }
        }
    }

    // Try to get version from cargo install list
    if let Ok(output) = std::process::Command::new("cargo")
        .args(&["install", "--list"])
        .output()
    {
        if output.status.success() {
            let install_output = String::from_utf8_lossy(&output.stdout);
            for line in install_output.lines() {
                if line.starts_with("wasmrust") {
                    if let Some(start) = line.find('v') {
                        if let Some(end) = line[start..].find(':') {
                            return Some(line[start+1..start+end].to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

fn detect_wasmgo_version() -> Option<String> {
    // Try to get version from tinygo
    if let Ok(output) = std::process::Command::new("tinygo")
        .args(&["version"])
        .output() 
    {
        if output.status.success() {
            let version_output = String::from_utf8_lossy(&output.stdout);
            if let Some(version_line) = version_output.lines().next() {
                if let Some(start) = version_line.find("tinygo version ") {
                    let version_part = &version_line[start + 15..];
                    if let Some(end) = version_part.find(' ') {
                        return Some(version_part[..end].to_string());
                    }
                    return Some(version_part.to_string());
                }
            }
        }
    }

    None
}

fn get_plugin_install_path(plugin_name: &str) -> String {
    if let Some(home_dir) = dirs::home_dir() {
        home_dir.join(".wasmrun").join("plugins").join(plugin_name).to_string_lossy().to_string()
    } else {
        format!("~/.wasmrun/plugins/{}", plugin_name)
    }
}
