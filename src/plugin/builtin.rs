//! Built-in plugin implementations
//!
//! This module contains all the built-in plugins that ship with Chakra

use crate::compiler::builder::WasmBuilder;
use crate::error::Result;
use crate::plugin::languages::{
    assemblyscript_plugin::AssemblyScriptBuilder, c_plugin::CBuilder, go_plugin::GoBuilder,
    python_plugin::PythonBuilder, rust_plugin::RustPlugin,
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
            author: "Chakra Team".to_string(),
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
        // Check if project contains any of the entry files
        for entry_file in &self.info.entry_files {
            let entry_path = std::path::Path::new(project_path).join(entry_file);
            if entry_path.exists() {
                return true;
            }
        }

        // Check if project contains files with supported extensions
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
        // Clone the Arc to create a new instance
        Box::new(BuiltinBuilderWrapper {
            builder: Arc::clone(&self.builder),
        })
    }
}

/// Wrapper to make Arc<dyn WasmBuilder> implement WasmBuilder
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
    // Rust plugin
    let rust_plugin = create_rust_plugin();
    registry.register_plugin(Box::new(rust_plugin))?;

    // Go plugin
    let go_plugin = create_go_plugin();
    registry.register_plugin(Box::new(go_plugin))?;

    // C plugin
    let c_plugin = create_c_plugin();
    registry.register_plugin(Box::new(c_plugin))?;

    // AssemblyScript plugin
    let asc_plugin = create_assemblyscript_plugin();
    registry.register_plugin(Box::new(asc_plugin))?;

    // Python plugin (placeholder for future implementation)
    let python_plugin = create_python_plugin();
    registry.register_plugin(Box::new(python_plugin))?;

    println!("Loaded {} built-in plugins", 5);
    Ok(())
}

/// Create the Rust built-in plugin
fn create_rust_plugin() -> BuiltinPlugin {
    let capabilities = PluginCapabilities {
        compile_wasm: true,
        compile_webapp: true,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
    };

    BuiltinPlugin::new(
        "rust".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "Rust WebAssembly compiler with wasm-bindgen and web application support".to_string(),
        vec!["rs".to_string()],
        vec!["Cargo.toml".to_string()],
        capabilities,
        Arc::new(RustPlugin::new()),
    )
}

/// Create the Go built-in plugin
fn create_go_plugin() -> BuiltinPlugin {
    let capabilities = PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["wasm".to_string()],
    };

    BuiltinPlugin::new(
        "go".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "Go WebAssembly compiler using TinyGo".to_string(),
        vec!["go".to_string()],
        vec!["go.mod".to_string(), "main.go".to_string()],
        capabilities,
        Arc::new(GoBuilder::new()),
    )
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

/// Create the AssemblyScript built-in plugin
fn create_assemblyscript_plugin() -> BuiltinPlugin {
    let capabilities = PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["wasm32".to_string()],
    };

    BuiltinPlugin::new(
        "assemblyscript".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "AssemblyScript WebAssembly compiler".to_string(),
        vec!["ts".to_string()],
        vec!["package.json".to_string(), "asconfig.json".to_string()],
        capabilities,
        Arc::new(AssemblyScriptBuilder::new()),
    )
}

/// Create the Python built-in plugin (placeholder)
fn create_python_plugin() -> BuiltinPlugin {
    let capabilities = PluginCapabilities {
        compile_wasm: false, // Not yet implemented
        compile_webapp: false,
        live_reload: false,
        optimization: false,
        custom_targets: vec![],
    };

    BuiltinPlugin::new(
        "python".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "Python WebAssembly compiler (coming soon)".to_string(),
        vec!["py".to_string()],
        vec!["main.py".to_string(), "pyproject.toml".to_string()],
        capabilities,
        Arc::new(PythonBuilder::new()),
    )
}

/// Get information about all built-in plugins without loading them
#[allow(dead_code)]
pub fn get_builtin_plugin_info() -> Vec<PluginInfo> {
    vec![
        create_rust_plugin().info().clone(),
        create_go_plugin().info().clone(),
        create_c_plugin().info().clone(),
        create_assemblyscript_plugin().info().clone(),
        create_python_plugin().info().clone(),
    ]
}

/// Check if a plugin name is a built-in plugin
#[allow(dead_code)]
pub fn is_builtin_plugin(name: &str) -> bool {
    matches!(name, "rust" | "go" | "c" | "assemblyscript" | "python")
}

/// Get specific built-in plugin info by name
#[allow(dead_code)]
pub fn get_builtin_plugin_by_name(name: &str) -> Option<PluginInfo> {
    match name {
        "rust" => Some(create_rust_plugin().info().clone()),
        "go" => Some(create_go_plugin().info().clone()),
        "c" => Some(create_c_plugin().info().clone()),
        "assemblyscript" => Some(create_assemblyscript_plugin().info().clone()),
        "python" => Some(create_python_plugin().info().clone()),
        _ => None,
    }
}
