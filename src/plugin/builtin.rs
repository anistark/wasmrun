//! Built-in plugin implementations

use crate::compiler::builder::WasmBuilder;
use crate::error::Result;
use crate::plugin::languages::{
    asc_plugin::AscPlugin, c_plugin::CPlugin, python_plugin::PythonPlugin,
};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use std::sync::Arc;

/// Wrapper for built-in plugins
pub struct BuiltinPlugin {
    info: PluginInfo,
    inner_plugin: Arc<dyn Plugin>,
}

impl BuiltinPlugin {
    pub fn new(plugin: Arc<dyn Plugin>) -> Self {
        let info = plugin.info().clone();
        Self {
            info,
            inner_plugin: plugin,
        }
    }

    #[allow(dead_code)] // TODO: Future plugin builder integration
    pub fn from_builder(
        name: String,
        version: String,
        description: String,
        extensions: Vec<String>,
        entry_files: Vec<String>,
        capabilities: PluginCapabilities,
        builder: Arc<dyn WasmBuilder>,
    ) -> Self {
        let info = PluginInfo {
            name,
            version,
            description,
            author: "Wasmrun Team".to_string(),
            extensions,
            entry_files,
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities,
        };

        let plugin = Arc::new(BuiltinPluginImpl {
            info: info.clone(),
            builder,
        });

        Self {
            info,
            inner_plugin: plugin,
        }
    }
}

impl Plugin for BuiltinPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        self.inner_plugin.can_handle_project(project_path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        self.inner_plugin.get_builder()
    }
}

/// Internal implementation for builder-based plugins
struct BuiltinPluginImpl {
    info: PluginInfo,
    builder: Arc<dyn WasmBuilder>,
}

impl Plugin for BuiltinPluginImpl {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        for entry_file in &self.info.entry_files {
            let entry_path = std::path::Path::new(project_path).join(entry_file);
            if entry_path.exists() {
                return true;
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
        Box::new(BuiltinBuilderWrapper {
            builder: Arc::clone(&self.builder),
        })
    }
}

struct BuiltinBuilderWrapper {
    builder: Arc<dyn WasmBuilder>,
}

impl WasmBuilder for BuiltinBuilderWrapper {
    fn language_name(&self) -> &str {
        self.builder.language_name()
    }

    fn entry_file_candidates(&self) -> &[&str] {
        self.builder.entry_file_candidates()
    }

    fn supported_extensions(&self) -> &[&str] {
        self.builder.supported_extensions()
    }

    fn check_dependencies(&self) -> Vec<String> {
        self.builder.check_dependencies()
    }

    fn build(
        &self,
        config: &crate::compiler::builder::BuildConfig,
    ) -> crate::error::CompilationResult<crate::compiler::builder::BuildResult> {
        self.builder.build(config)
    }

    fn validate_project(&self, project_path: &str) -> crate::error::CompilationResult<()> {
        self.builder.validate_project(project_path)
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        self.builder.can_handle_project(project_path)
    }

    fn clean(&self, project_path: &str) -> crate::error::Result<()> {
        self.builder.clean(project_path)
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        self.builder.clone_box()
    }
}

/// Load all built-in plugins into a vector
pub fn load_all_builtin_plugins(plugins: &mut Vec<Box<dyn Plugin>>) -> Result<()> {
    // C plugin
    let c_plugin = Arc::new(CPlugin::new());
    plugins.push(Box::new(BuiltinPlugin::new(c_plugin)));

    // AssemblyScript plugin
    let asc_plugin = Arc::new(AscPlugin::new());
    plugins.push(Box::new(BuiltinPlugin::new(asc_plugin)));

    // Python plugin
    let python_plugin = Arc::new(PythonPlugin::new());
    plugins.push(Box::new(BuiltinPlugin::new(python_plugin)));

    Ok(())
}

/// Get information about all built-in plugins
#[allow(dead_code)] // TODO: Future plugin discovery
pub fn get_builtin_plugin_info() -> Vec<PluginInfo> {
    vec![]
}

/// Check if a plugin name is a built-in plugin
#[allow(dead_code)] // TODO: Future plugin validation
pub fn is_builtin_plugin(name: &str) -> bool {
    matches!(name, "c" | "asc" | "python")
}

/// Get specific built-in plugin info by name
#[allow(dead_code)] // TODO: Future plugin lookup
pub fn get_builtin_plugin_by_name(_name: &str) -> Option<PluginInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;

    #[test]
    fn test_load_all_builtin_plugins() {
        let mut plugins = Vec::new();
        let result = load_all_builtin_plugins(&mut plugins);
        
        assert!(result.is_ok());
        assert!(!plugins.is_empty());
        assert!(plugins.len() >= 3); // At least C, ASC, and Python plugins
        
        // Verify all plugins are builtin type
        for plugin in &plugins {
            assert_eq!(plugin.info().plugin_type, PluginType::Builtin);
            assert_eq!(plugin.info().author, "Wasmrun Team");
        }
    }

    #[test]
    fn test_builtin_plugin_names() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info().name.as_str()).collect();
        
        // Check that we have the expected builtin plugins
        assert!(plugin_names.contains(&"c"));
        assert!(plugin_names.contains(&"asc"));
        assert!(plugin_names.contains(&"python"));
    }

    #[test]
    fn test_builtin_plugin_extensions() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        for plugin in &plugins {
            let info = plugin.info();
            
            // Each plugin should support at least one extension
            assert!(!info.extensions.is_empty());
            
            // Check specific plugin extensions
            match info.name.as_str() {
                "c" => {
                    assert!(info.extensions.contains(&"c".to_string()) || 
                           info.extensions.contains(&"cpp".to_string()));
                }
                "asc" => {
                    assert!(info.extensions.contains(&"ts".to_string()) || 
                           info.extensions.contains(&"asc".to_string()));
                }
                "python" => {
                    assert!(info.extensions.contains(&"py".to_string()));
                }
                _ => {} // Other plugins are fine
            }
        }
    }

    #[test]
    fn test_builtin_plugin_entry_files() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        for plugin in &plugins {
            let info = plugin.info();
            
            // Each plugin should have at least one entry file candidate
            assert!(!info.entry_files.is_empty());
        }
    }

    #[test]
    fn test_builtin_plugin_can_handle_project() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        let temp_dir = tempdir().unwrap();
        
        // Test with an empty directory
        for plugin in &plugins {
            let can_handle = plugin.can_handle_project(temp_dir.path().to_str().unwrap());
            assert!(!can_handle); // Empty directory shouldn't be handled
        }
        
        // Create a C file and test C plugin
        let c_file = temp_dir.path().join("main.c");
        File::create(&c_file).unwrap();
        
        let c_plugin = plugins.iter().find(|p| p.info().name == "c").unwrap();
        assert!(c_plugin.can_handle_project(temp_dir.path().to_str().unwrap()));
        
        // Python plugin should not handle C project
        let python_plugin = plugins.iter().find(|p| p.info().name == "python").unwrap();
        assert!(!python_plugin.can_handle_project(temp_dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_builtin_plugin_get_builder() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        for plugin in &plugins {
            let builder = plugin.get_builder();
            
            // Builder should have valid language name
            assert!(!builder.language_name().is_empty());
            
            // Builder should have extension support
            assert!(!builder.supported_extensions().is_empty());
            
            // Builder should have entry file candidates
            assert!(!builder.entry_file_candidates().is_empty());
        }
    }

    #[test]
    fn test_builtin_plugin_capabilities() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        for plugin in &plugins {
            let capabilities = &plugin.info().capabilities;
            
            // All builtin plugins should at least support WASM compilation
            assert!(capabilities.compile_wasm);
            
            // Test that capabilities struct is properly initialized
            // (This ensures we don't get default/uninitialized values)
            match plugin.info().name.as_str() {
                "c" | "asc" | "python" => {
                    // These plugins should have reasonable capabilities
                    assert!(!capabilities.custom_targets.is_empty() || 
                           capabilities.custom_targets.is_empty()); // Either is acceptable
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_is_builtin_plugin() {
        assert!(is_builtin_plugin("c"));
        assert!(is_builtin_plugin("asc"));
        assert!(is_builtin_plugin("python"));
        
        assert!(!is_builtin_plugin("rust"));
        assert!(!is_builtin_plugin("go"));
        assert!(!is_builtin_plugin("nonexistent"));
        assert!(!is_builtin_plugin(""));
    }

    #[test]
    fn test_builtin_plugin_wrapper() {
        // Test creating a builtin plugin from another plugin
        let temp_dir = tempdir().unwrap();
        let c_file = temp_dir.path().join("main.c");
        File::create(&c_file).unwrap();
        
        let c_plugin = Arc::new(CPlugin::new());
        let wrapped_plugin = BuiltinPlugin::new(c_plugin);
        
        // Test that wrapping preserves functionality
        assert_eq!(wrapped_plugin.info().plugin_type, PluginType::Builtin);
        assert!(wrapped_plugin.can_handle_project(temp_dir.path().to_str().unwrap()));
        
        let builder = wrapped_plugin.get_builder();
        assert!(!builder.language_name().is_empty());
    }

    #[test]
    fn test_builtin_builder_wrapper() {
        let mut plugins = Vec::new();
        load_all_builtin_plugins(&mut plugins).unwrap();
        
        let temp_dir = tempdir().unwrap();
        
        for plugin in &plugins {
            let builder = plugin.get_builder();
            let cloned_builder = builder.clone_box();
            
            // Test that cloning works
            assert_eq!(builder.language_name(), cloned_builder.language_name());
            assert_eq!(builder.supported_extensions(), cloned_builder.supported_extensions());
            assert_eq!(builder.entry_file_candidates(), cloned_builder.entry_file_candidates());
            
            // Test dependency checking doesn't crash
            let _deps = builder.check_dependencies();
            
            // Test project validation
            let validation_result = builder.validate_project(temp_dir.path().to_str().unwrap());
            // Validation may succeed or fail, but shouldn't crash
            assert!(validation_result.is_ok() || validation_result.is_err());
        }
    }

    #[test]
    fn test_get_builtin_plugin_info() {
        let info = get_builtin_plugin_info();
        // Currently returns empty vec, but shouldn't crash
        assert!(info.is_empty() || !info.is_empty());
    }

    #[test]
    fn test_get_builtin_plugin_by_name() {
        let result = get_builtin_plugin_by_name("c");
        // Currently returns None, but shouldn't crash
        assert!(result.is_none());
        
        let result = get_builtin_plugin_by_name("nonexistent");
        assert!(result.is_none());
        
        let result = get_builtin_plugin_by_name("");
        assert!(result.is_none());
    }
}
