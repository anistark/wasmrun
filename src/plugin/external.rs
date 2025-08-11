use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult, Result, WasmrunError};
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::metadata::PluginMetadata;
use crate::plugin::{Plugin, PluginInfo};
use crate::utils::{PluginUtils, SystemUtils};

#[cfg(not(target_os = "windows"))]
use crate::plugin::bridge::symbols;
#[cfg(not(target_os = "windows"))]
use libloading::Library;

/// Generic wrapper for all external plugins (no hardcoding)
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
    metadata: PluginMetadata,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl ExternalPluginWrapper {
    pub fn new(plugin_path: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let plugin_name = entry.info.name.clone();

        if !PluginUtils::is_plugin_available(&plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' not available"
            )));
        }

        // Load metadata for ALL plugins
        let metadata = PluginMetadata::from_installed_plugin(&plugin_path)
            .or_else(|_| PluginMetadata::from_crates_io(&plugin_name))?;

        metadata.validate()?;

        #[cfg(not(target_os = "windows"))]
        let library = Self::try_load_library(&plugin_name, &plugin_path)?;

        Ok(Self {
            info: entry.info,
            plugin_name,
            metadata,
            #[cfg(not(target_os = "windows"))]
            library,
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn try_load_library(plugin_name: &str, plugin_path: &Path) -> Result<Option<Arc<Library>>> {
        let lib_extensions = ["so", "dylib"];

        for ext in &lib_extensions {
            let path = plugin_path.join(format!("lib{plugin_name}.{ext}"));
            if path.exists() {
                unsafe {
                    match Library::new(&path) {
                        Ok(library) => {
                            if library
                                .get::<symbols::CreateBuilderFn>(b"create_wasm_builder")
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

    fn check_project_via_metadata(&self, project_path: &str) -> bool {
        let path = Path::new(project_path);

        // Check entry files from metadata
        for entry_file in &self.metadata.entry_files {
            if path.join(entry_file).exists() {
                return true;
            }
        }

        // Check supported extensions from metadata
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension().and_then(|e| e.to_str()) {
                    if self.metadata.extensions.contains(&extension.to_string()) {
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

    fn can_handle_project(&self, path: &str) -> bool {
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(library) = &self.library {
                if let Some(exports) = &self.metadata.exports {
                    unsafe {
                        if let Ok(can_handle) = library.get::<symbols::CanHandleProjectFn>(
                            exports.can_handle_project.as_bytes(),
                        ) {
                            let c_path = std::ffi::CString::new(path).unwrap();
                            return can_handle(std::ptr::null(), c_path.as_ptr());
                        }
                    }
                }
            }
        }

        self.check_project_via_metadata(path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder::new(
            self.plugin_name.clone(),
            self.metadata.clone(),
            #[cfg(not(target_os = "windows"))]
            self.library.clone(),
        ))
    }
}

/// Generic WASM builder for all external plugins
pub struct ExternalWasmBuilder {
    plugin_name: String,
    metadata: PluginMetadata,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl ExternalWasmBuilder {
    pub fn new(
        plugin_name: String,
        metadata: PluginMetadata,
        #[cfg(not(target_os = "windows"))] library: Option<Arc<Library>>,
    ) -> Self {
        Self {
            plugin_name,
            metadata,
            #[cfg(not(target_os = "windows"))]
            library,
        }
    }

    fn build_via_command(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let plugin_binary = format!("wasmrun-{}", self.plugin_name);

        let output = std::process::Command::new(&plugin_binary)
            .args(["build", &config.project_path])
            .args(["--output", &config.output_dir])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                let output_file = PathBuf::from(&config.output_dir).join("output.wasm");
                if output_file.exists() {
                    Ok(BuildResult {
                        wasm_path: output_file.to_string_lossy().to_string(),
                        js_path: None,
                        additional_files: vec![],
                        is_wasm_bindgen: false,
                    })
                } else {
                    Err(CompilationError::BuildFailed {
                        language: self.plugin_name.clone(),
                        reason: "Build completed but no output file found".to_string(),
                    })
                }
            }
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr);
                Err(CompilationError::BuildFailed {
                    language: self.plugin_name.clone(),
                    reason: format!("Build failed: {stderr}"),
                })
            }
            Err(e) => Err(CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!("Failed to execute plugin: {e}"),
            }),
        }
    }
}

impl WasmBuilder for ExternalWasmBuilder {
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(library) = &self.library {
                if let Some(exports) = &self.metadata.exports {
                    unsafe {
                        if let Ok(build_fn) =
                            library.get::<symbols::BuildFn>(exports.build.as_bytes())
                        {
                            let config_c =
                                crate::plugin::bridge::BuildConfigC::from_build_config(config);
                            let result_ptr = build_fn(std::ptr::null(), &config_c);

                            if !result_ptr.is_null() {
                                let result = crate::plugin::bridge::BuildResultC::to_build_result(
                                    result_ptr,
                                );
                                return Ok(result);
                            }
                        }
                    }
                }
            }
        }

        self.build_via_command(config)
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        for tool in &self.metadata.dependencies.tools {
            if !SystemUtils::is_tool_available(tool) {
                missing.push(tool.clone());
            }
        }

        missing
    }

    fn validate_project(&self, path: &str) -> CompilationResult<()> {
        let project_path = Path::new(path);

        if !project_path.exists() {
            return Err(CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: "Project path does not exist".to_string(),
            });
        }

        let has_entry_file = self
            .metadata
            .entry_files
            .iter()
            .any(|entry_file| project_path.join(entry_file).exists());

        if !has_entry_file {
            return Err(CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!(
                    "No entry files found. Expected one of: {}",
                    self.metadata.entry_files.join(", ")
                ),
            });
        }

        Ok(())
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        let path = Path::new(project_path);

        // Check for entry files
        for entry_file in &self.metadata.entry_files {
            if path.join(entry_file).exists() {
                return true;
            }
        }

        // Check file extensions
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension().and_then(|e| e.to_str()) {
                    if self.metadata.extensions.contains(&extension.to_string()) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(library) = &self.library {
                if let Some(exports) = &self.metadata.exports {
                    unsafe {
                        if let Ok(clean_fn) =
                            library.get::<symbols::CleanFn>(exports.clean.as_bytes())
                        {
                            let c_path = std::ffi::CString::new(project_path).unwrap();
                            let result = clean_fn(std::ptr::null(), c_path.as_ptr());
                            if result {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        // Fallback: clean common build artifacts
        let project_path = Path::new(project_path);
        let build_dirs = ["target", "build", "dist", "out"];

        for dir in &build_dirs {
            let build_path = project_path.join(dir);
            if build_path.exists() {
                let _ = std::fs::remove_dir_all(&build_path);
            }
        }

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder::new(
            self.plugin_name.clone(),
            self.metadata.clone(),
            #[cfg(not(target_os = "windows"))]
            self.library.clone(),
        ))
    }

    fn language_name(&self) -> &str {
        &self.plugin_name
    }

    fn entry_file_candidates(&self) -> &[&str] {
        // Return static references to avoid lifetime issues
        &[]
    }

    fn supported_extensions(&self) -> &[&str] {
        // Return static references to avoid lifetime issues
        &[]
    }
}

/// External plugin loader for managing plugin loading
pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        let plugin_path = PathBuf::from(&entry.install_path);
        let wrapper = ExternalPluginWrapper::new(plugin_path, entry.clone())?;
        Ok(Box::new(wrapper))
    }

    pub fn create_generic_entry(plugin_name: &str) -> Result<ExternalPluginEntry> {
        let metadata = PluginMetadata::from_crates_io(plugin_name)?;

        let info = PluginInfo {
            name: plugin_name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("{plugin_name} WebAssembly plugin"),
            author: "Community".to_string(),
            extensions: metadata.extensions.clone(),
            entry_files: metadata.entry_files.clone(),
            plugin_type: crate::plugin::PluginType::External,
            source: Some(crate::plugin::PluginSource::CratesIo {
                name: plugin_name.to_string(),
                version: "latest".to_string(),
            }),
            dependencies: metadata.dependencies.tools.clone(),
            capabilities: Default::default(),
        };

        Ok(ExternalPluginEntry {
            info,
            enabled: true,
            install_path: plugin_name.to_string(),
            executable_path: None,
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            source: crate::plugin::PluginSource::CratesIo {
                name: plugin_name.to_string(),
                version: "latest".to_string(),
            },
        })
    }
}
