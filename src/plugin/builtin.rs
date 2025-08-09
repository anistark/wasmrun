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
