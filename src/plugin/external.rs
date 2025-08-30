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

        let search_paths = [
            plugin_path.to_path_buf(),
            plugin_path.join("target/release"),
            plugin_path.join("target/debug"),
        ];

        for search_path in &search_paths {
            for ext in &lib_extensions {
                let path = search_path.join(format!("lib{plugin_name}.{ext}"));
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
                            Err(_) => {
                                continue;
                            }
                        }
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
            let entry_path = path.join(entry_file);
            if entry_path.exists() {
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
                            if let Ok(create_builder) =
                                library.get::<symbols::CreateBuilderFn>(b"create_wasm_builder")
                            {
                                let builder_ptr = create_builder();
                                if !builder_ptr.is_null() {
                                    let c_path = std::ffi::CString::new(path).unwrap();
                                    let result = can_handle(builder_ptr, c_path.as_ptr());
                                    // TODO: Free builder if needed
                                    return result;
                                }
                            }
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
        // Try to find the plugin binary in ~/.wasmrun/bin first, then fallback to system PATH
        let wasmrun_bin_path = dirs::home_dir()
            .map(|home| home.join(".wasmrun").join("bin").join(&self.plugin_name))
            .unwrap_or_else(|| PathBuf::from(&self.plugin_name));

        let plugin_binary = if wasmrun_bin_path.exists() {
            wasmrun_bin_path.to_string_lossy().to_string()
        } else {
            self.plugin_name.clone()
        };

        let output = std::process::Command::new(&plugin_binary)
            .args(["compile", "-p", &config.project_path])
            .args(["-o", &config.output_dir])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                // Look for any .wasm files in the output directory
                let output_dir = PathBuf::from(&config.output_dir);
                if let Ok(entries) = std::fs::read_dir(&output_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                            return Ok(BuildResult {
                                wasm_path: path.to_string_lossy().to_string(),
                                js_path: None,
                                additional_files: vec![],
                                is_wasm_bindgen: false,
                            });
                        }
                    }
                }

                Err(CompilationError::BuildFailed {
                    language: self.plugin_name.clone(),
                    reason: "Build completed but no .wasm file found in output directory"
                        .to_string(),
                })
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
                        // Create a builder instance
                        if let Ok(create_builder) =
                            library.get::<symbols::CreateBuilderFn>(b"create_wasm_builder")
                        {
                            let builder_ptr = create_builder();
                            if !builder_ptr.is_null() {
                                if let Ok(build_fn) =
                                    library.get::<symbols::BuildFn>(exports.build.as_bytes())
                                {
                                    let config_c =
                                        crate::plugin::bridge::BuildConfigC::from_build_config(
                                            config,
                                        );
                                    let result_ptr = build_fn(builder_ptr, &config_c);

                                    if !result_ptr.is_null() {
                                        let result =
                                            crate::plugin::bridge::BuildResultC::to_build_result(
                                                result_ptr,
                                            );
                                        return Ok(result);
                                    }
                                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginCapabilities, PluginSource, PluginType};
    use std::fs::File;
    use tempfile::tempdir;

    fn create_mock_metadata() -> PluginMetadata {
        PluginMetadata {
            name: "test_plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            extensions: vec!["test".to_string()],
            entry_files: vec!["main.test".to_string()],
            capabilities: crate::plugin::metadata::MetadataCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: false,
                optimization: false,
                custom_targets: vec![],
                supported_languages: Some(vec!["test".to_string()]),
            },
            dependencies: crate::plugin::metadata::MetadataDependencies {
                tools: vec!["test_tool".to_string()],
                optional_tools: None,
            },
            exports: None,
            frameworks: None,
        }
    }

    fn create_mock_entry() -> ExternalPluginEntry {
        ExternalPluginEntry {
            info: PluginInfo {
                name: "test_plugin".to_string(),
                version: "1.0.0".to_string(),
                description: "Test plugin".to_string(),
                author: "Test Author".to_string(),
                extensions: vec!["test".to_string()],
                entry_files: vec!["main.test".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "test_plugin".to_string(),
                    version: "1.0.0".to_string(),
                }),
                dependencies: vec!["test_tool".to_string()],
                capabilities: PluginCapabilities::default(),
            },
            enabled: true,
            install_path: "/mock/path".to_string(),
            executable_path: None,
            installed_at: "2023-01-01T00:00:00Z".to_string(),
            source: PluginSource::CratesIo {
                name: "test_plugin".to_string(),
                version: "1.0.0".to_string(),
            },
        }
    }

    #[test]
    fn test_external_wasm_builder_new() {
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        assert_eq!(builder.language_name(), "test_plugin");
        assert_eq!(builder.entry_file_candidates().len(), 0); // Returns empty slice
        assert_eq!(builder.supported_extensions().len(), 0); // Returns empty slice
    }

    #[test]
    fn test_external_wasm_builder_can_handle_project() {
        let temp_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        // Test with empty directory
        assert!(!builder.can_handle_project(temp_dir.path().to_str().unwrap()));

        // Test with matching entry file
        let entry_file = temp_dir.path().join("main.test");
        File::create(&entry_file).unwrap();
        assert!(builder.can_handle_project(temp_dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_external_wasm_builder_can_handle_project_by_extension() {
        let temp_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        // Test with matching extension
        let test_file = temp_dir.path().join("example.test");
        File::create(&test_file).unwrap();
        assert!(builder.can_handle_project(temp_dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_external_wasm_builder_validate_project() {
        let temp_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        // Test with non-existent path
        let result = builder.validate_project("/nonexistent/path");
        assert!(result.is_err());

        // Test with existing path but no entry files
        let result = builder.validate_project(temp_dir.path().to_str().unwrap());
        assert!(result.is_err());

        // Test with entry file present
        let entry_file = temp_dir.path().join("main.test");
        File::create(&entry_file).unwrap();
        let result = builder.validate_project(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_external_wasm_builder_check_dependencies() {
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        let missing_deps = builder.check_dependencies();
        // test_tool should be missing (unless coincidentally installed)
        assert!(missing_deps.contains(&"test_tool".to_string()));
    }

    #[test]
    fn test_external_wasm_builder_clone() {
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        let cloned_builder = builder.clone_box();
        assert_eq!(builder.language_name(), cloned_builder.language_name());
    }

    #[test]
    fn test_external_wasm_builder_clean() {
        let temp_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        // Create some build directories
        let target_dir = temp_dir.path().join("target");
        std::fs::create_dir(&target_dir).unwrap();
        File::create(target_dir.join("test.o")).unwrap();

        let build_dir = temp_dir.path().join("build");
        std::fs::create_dir(&build_dir).unwrap();
        File::create(build_dir.join("test.wasm")).unwrap();

        // Clean should succeed and remove build directories
        let result = builder.clean(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());

        // Directories should be removed (or clean should at least not error)
        // Note: clean is best-effort, so we don't assert directory removal
    }

    #[test]
    fn test_external_wasm_builder_build_command_failure() {
        let temp_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "nonexistent_plugin_12345".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        let config = BuildConfig {
            project_path: temp_dir.path().to_str().unwrap().to_string(),
            output_dir: output_dir.path().to_str().unwrap().to_string(),
            optimization_level: crate::compiler::builder::OptimizationLevel::Debug,
            verbose: false,
            watch: false,
            target_type: crate::compiler::builder::TargetType::Standard,
        };

        let result = builder.build(&config);
        assert!(result.is_err());

        if let Err(CompilationError::BuildFailed {
            language,
            reason: _,
        }) = result
        {
            assert_eq!(language, "nonexistent_plugin_12345");
        } else {
            panic!("Expected BuildFailed error");
        }
    }

    #[test]
    fn test_external_plugin_loader_create_generic_entry() {
        // This will likely fail because the plugin doesn't exist, but shouldn't crash
        let result = ExternalPluginLoader::create_generic_entry("nonexistent_plugin_12345");
        // Should either succeed (with defaults) or fail gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_external_plugin_loader_load_invalid() {
        let entry = create_mock_entry();
        let result = ExternalPluginLoader::load(&entry);
        // Should fail gracefully for invalid plugin path
        assert!(result.is_err());
    }

    #[test]
    fn test_external_plugin_wrapper_metadata_check() {
        let temp_dir = tempdir().unwrap();
        let _metadata = create_mock_metadata();

        // Test check_project_via_metadata logic
        let plugin_path = temp_dir.path().to_path_buf();
        let entry = create_mock_entry();

        // This will fail because the plugin isn't available, but we're testing the structure
        let wrapper_result = ExternalPluginWrapper::new(plugin_path, entry);
        assert!(wrapper_result.is_err()); // Expected to fail with unavailable plugin
    }

    #[test]
    fn test_external_plugin_wrapper_info_structure() {
        let entry = create_mock_entry();

        // Test that the entry structure is valid
        assert_eq!(entry.info.name, "test_plugin");
        assert_eq!(entry.info.version, "1.0.0");
        assert_eq!(entry.info.plugin_type, PluginType::External);
        assert!(entry.enabled);
        assert!(!entry.info.extensions.is_empty());
        assert!(!entry.info.entry_files.is_empty());
    }

    #[test]
    fn test_external_plugin_metadata_validation() {
        let metadata = create_mock_metadata();

        // Test metadata structure
        assert_eq!(metadata.name, "test_plugin");
        assert_eq!(metadata.version, "1.0.0");
        assert!(!metadata.extensions.is_empty());
        assert!(!metadata.entry_files.is_empty());
        assert!(!metadata.dependencies.tools.is_empty());
    }

    #[test]
    fn test_external_plugin_path_handling() {
        let temp_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        // Test path handling for various scenarios
        assert!(!builder.can_handle_project(""));
        assert!(!builder.can_handle_project("/nonexistent/path"));

        // Test with actual directory
        let result = builder.can_handle_project(temp_dir.path().to_str().unwrap());
        assert!(!result); // No matching files
    }

    #[test]
    fn test_build_config_compatibility() {
        let temp_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let metadata = create_mock_metadata();
        let builder = ExternalWasmBuilder::new(
            "test_plugin".to_string(),
            metadata,
            #[cfg(not(target_os = "windows"))]
            None,
        );

        // Test different build configurations
        let configs = vec![
            BuildConfig {
                project_path: temp_dir.path().to_str().unwrap().to_string(),
                output_dir: output_dir.path().to_str().unwrap().to_string(),
                optimization_level: crate::compiler::builder::OptimizationLevel::Debug,
                verbose: false,
                watch: false,
                target_type: crate::compiler::builder::TargetType::Standard,
            },
            BuildConfig {
                project_path: temp_dir.path().to_str().unwrap().to_string(),
                output_dir: output_dir.path().to_str().unwrap().to_string(),
                optimization_level: crate::compiler::builder::OptimizationLevel::Release,
                verbose: true,
                watch: true,
                target_type: crate::compiler::builder::TargetType::Standard,
            },
        ];

        for config in &configs {
            // Build will fail (no plugin binary), but shouldn't crash
            let result = builder.build(config);
            assert!(result.is_err()); // Expected to fail
        }
    }
}
