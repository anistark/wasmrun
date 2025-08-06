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
        let home_dir = std::env::var("HOME").unwrap_or_default();
        let plugin_dir = format!("{}/.wasmrun/plugins/{}", home_dir, plugin_name);
        
        let lib_extensions = ["so", "dylib"];
        
        for ext in &lib_extensions {
            let path = format!("{}/lib{}.{}", plugin_dir, plugin_name, ext);
            if Path::new(&path).exists() {
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
        let home_dir = std::env::var("HOME").unwrap_or_default();
        let plugin_dir = format!("{}/.wasmrun/plugins/{}", home_dir, plugin_name);

        if std::path::Path::new(&plugin_dir).exists() {
            let crates_toml = format!("{}/.crates.toml", plugin_dir);
            let crates2_json = format!("{}/.crates2.json", plugin_dir);
            
            return std::path::Path::new(&crates_toml).exists() 
                || std::path::Path::new(&crates2_json).exists();
        }

        Command::new(plugin_name)
            .arg("--version")
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
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(ref library) = self.library {
                unsafe {
                    let symbols = PluginSymbols::get_symbol_names(&self.plugin_name);
                    if let Ok(can_handle_fn) = library.get::<CanHandleProjectFn>(symbols.can_handle_project) {
                        if let Ok(create_builder_fn) = library.get::<CreateBuilderFn>(symbols.create_builder) {
                            let builder_ptr = create_builder_fn();
                            if !builder_ptr.is_null() {
                                let path_cstr = std::ffi::CString::new(project_path).unwrap();
                                let result = can_handle_fn(builder_ptr, path_cstr.as_ptr());
                                return result;
                            } else {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }

        #[cfg(target_os = "windows")]
        {
            false
        }
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder {
            plugin_name: self.plugin_name.clone(),
            #[cfg(not(target_os = "windows"))]
            library: self.library.clone(),
        })
    }
}

pub struct ExternalWasmBuilder {
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl WasmBuilder for ExternalWasmBuilder {
    fn can_handle_project(&self, project_path: &str) -> bool {
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(ref library) = self.library {
                unsafe {
                    let symbols = PluginSymbols::get_symbol_names(&self.plugin_name);
                    if let Ok(can_handle_fn) = library.get::<CanHandleProjectFn>(symbols.can_handle_project) {
                        if let Ok(create_builder_fn) = library.get::<CreateBuilderFn>(symbols.create_builder) {
                            let builder_ptr = create_builder_fn();
                            if !builder_ptr.is_null() {
                                let path_cstr = std::ffi::CString::new(project_path).unwrap();
                                let result = can_handle_fn(builder_ptr, path_cstr.as_ptr());
                                return result;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(crate::error::CompilationError::BuildFailed {
            language: self.plugin_name.clone(),
            reason: "External plugin build not implemented".to_string(),
        })
    }

    fn clean(&self, _project_path: &str) -> Result<()> {
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
        &[]
    }

    fn supported_extensions(&self) -> &[&str] {
        &[]
    }

    fn check_dependencies(&self) -> Vec<String> {
        vec![]
    }

    fn validate_project(&self, _project_path: &str) -> CompilationResult<()> {
        Ok(())
    }
}

pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        let wrapper = ExternalPluginWrapper::new(PathBuf::new(), entry.clone())?;
        Ok(Box::new(wrapper))
    }

    pub fn create_wasmrust_entry() -> ExternalPluginEntry {
        // Try to detect actual version
        let version = detect_wasmrust_version().unwrap_or_else(|| "0.2.0".to_string());
        
        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmrust".to_string(),
                version: version.clone(),
                description: "Rust to WebAssembly compiler with wasm-bindgen support".to_string(),
                author: "wasmrun".to_string(),
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
            install_path: "~/.wasmrun/plugins/wasmrust".to_string(),
            executable_path: Some("wasmrust".to_string()),
        }
    }

    pub fn create_wasmgo_entry() -> ExternalPluginEntry {
        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmgo".to_string(),
                version: "latest".to_string(),
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
                    version: "latest".to_string(),
                }),
            },
            enabled: true,
            source: PluginSource::CratesIo {
                name: "wasmgo".to_string(),
                version: "latest".to_string(),
            },
            installed_at: "2025-01-01T00:00:00Z".to_string(),
            install_path: "~/.wasmrun/plugins/wasmgo".to_string(),
            executable_path: Some("tinygo".to_string()),
        }
    }
}

fn detect_wasmrust_version() -> Option<String> {
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
