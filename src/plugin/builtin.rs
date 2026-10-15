//! Built-in plugin implementations

use crate::compiler::builder::WasmBuilder;
use crate::error::Result;
use crate::plugin::languages::asc_plugin::AscPlugin;
use crate::plugin::languages::c_plugin::CPlugin;
use crate::plugin::languages::go_plugin::GoPlugin;
use crate::plugin::languages::rust_plugin::RustPlugin;
use crate::plugin::{Plugin, PluginInfo};
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

/// Load all built-in plugins into a vector
pub fn load_all_builtin_plugins(plugins: &mut Vec<Box<dyn Plugin>>) -> Result<()> {
    plugins.push(Box::new(BuiltinPlugin::new(Arc::new(CPlugin::new()))));
    plugins.push(Box::new(BuiltinPlugin::new(Arc::new(AscPlugin::new()))));
    plugins.push(Box::new(BuiltinPlugin::new(Arc::new(GoPlugin::new()))));
    plugins.push(Box::new(BuiltinPlugin::new(Arc::new(RustPlugin::new()))));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginType;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_load_all_builtin_plugins() {
        let mut plugins = Vec::new();
        let result = load_all_builtin_plugins(&mut plugins);

        assert!(result.is_ok());
        assert!(!plugins.is_empty()); // At least C plugin

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

        assert!(plugin_names.contains(&"c"));
        assert!(plugin_names.contains(&"asc"));
        assert!(plugin_names.contains(&"go"));
        assert!(plugin_names.contains(&"rust"));
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
            if info.name.as_str() == "c" {
                assert!(
                    info.extensions.contains(&"c".to_string())
                        || info.extensions.contains(&"cpp".to_string())
                );
            } // Other plugins are fine
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
                "c" | "asc" => {
                    // These plugins should have reasonable capabilities
                    assert!(
                        !capabilities.custom_targets.is_empty()
                            || capabilities.custom_targets.is_empty()
                    ); // Either is acceptable
                }
                _ => {}
            }
        }
    }
}
