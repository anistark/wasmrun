//! Built-in plugin implementations

use crate::compiler::builder::WasmBuilder;
use crate::error::Result;
use crate::plugin::languages::{
    asc_plugin::AscBuilder, c_plugin::CBuilder, 
    python_plugin::PythonBuilder,
};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginRegistry, PluginType};
use std::sync::Arc;

/// Wrapper for built-in plugins
pub struct BuiltinPlugin {
    info: PluginInfo,
    builder: Arc<dyn WasmBuilder>,
}

impl BuiltinPlugin {
    /// Create a new built-in plugin
    pub fn new(
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

        Self { info, builder }
    }
}

impl Plugin for BuiltinPlugin {
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
}

/// Load all built-in plugins into the registry
pub fn load_all_builtin_plugins(registry: &mut PluginRegistry) -> Result<()> {
    // C plugin
    let c_plugin = create_c_plugin();
    registry.register_plugin(Box::new(c_plugin))?;

    // Asc plugin
    let asc_plugin = create_asc_plugin();
    registry.register_plugin(Box::new(asc_plugin))?;

    println!("Loaded {} built-in plugins", 3);
    Ok(())
}

/// Create the C built-in plugin
fn create_c_plugin() -> BuiltinPlugin {
    let capabilities = PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["wasm32".to_string()],
    };

    BuiltinPlugin::new(
        "c".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "C/C++ WebAssembly compiler using Emscripten".to_string(),
        vec![
            "c".to_string(),
            "cpp".to_string(),
            "h".to_string(),
            "hpp".to_string(),
        ],
        vec!["main.c".to_string(), "Makefile".to_string()],
        capabilities,
        Arc::new(CBuilder::new()),
    )
}

/// Create the Asc built-in plugin
fn create_asc_plugin() -> BuiltinPlugin {
    let capabilities = PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["wasm32".to_string()],
    };

    BuiltinPlugin::new(
        "asc".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "Asc WebAssembly compiler".to_string(),
        vec!["ts".to_string()],
        vec!["package.json".to_string(), "asconfig.json".to_string()],
        capabilities,
        Arc::new(AscBuilder::new()),
    )
}

/// Get information about all built-in plugins
#[allow(dead_code)]
pub fn get_builtin_plugin_info() -> Vec<PluginInfo> {
    vec![
        create_c_plugin().info().clone(),
        create_asc_plugin().info().clone(),
        create_python_plugin().info().clone(),
    ]
}

/// Check if a plugin name is a built-in plugin
#[allow(dead_code)]
pub fn is_builtin_plugin(name: &str) -> bool {
    matches!(name, "c" | "asc" | "python")
}

/// Get specific built-in plugin info by name
#[allow(dead_code)]
pub fn get_builtin_plugin_by_name(name: &str) -> Option<PluginInfo> {
    match name {
        "c" => Some(create_c_plugin().info().clone()),
        "asc" => Some(create_asc_plugin().info().clone()),
        "python" => Some(create_python_plugin().info().clone()),
        _ => None,
    }
}
