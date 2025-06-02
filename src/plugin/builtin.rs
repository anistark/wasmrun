//! Built-in plugin implementations
//!
//! This module contains all the built-in plugins that ship with Chakra

use crate::compiler::builder::WasmBuilder;
use crate::error::Result;
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
        Arc::new(crate::compiler::language::rust::RustBuilder::new()),
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
        Arc::new(crate::compiler::language::go::GoBuilder::new()),
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
        Arc::new(crate::compiler::language::c::CBuilder::new()),
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
        Arc::new(crate::compiler::language::asc::AssemblyScriptBuilder::new()),
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
        Arc::new(crate::compiler::language::python::PythonBuilder::new()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_rust_plugin() {
        let plugin = create_rust_plugin();
        let info = plugin.info();

        assert_eq!(info.name, "rust");
        assert_eq!(info.plugin_type, PluginType::Builtin);
        assert!(info.capabilities.compile_wasm);
        assert!(info.capabilities.compile_webapp);
        assert!(info.extensions.contains(&"rs".to_string()));
        assert!(info.entry_files.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn test_create_go_plugin() {
        let plugin = create_go_plugin();
        let info = plugin.info();

        assert_eq!(info.name, "go");
        assert_eq!(info.plugin_type, PluginType::Builtin);
        assert!(info.capabilities.compile_wasm);
        assert!(!info.capabilities.compile_webapp);
        assert!(info.extensions.contains(&"go".to_string()));
        assert!(info.entry_files.contains(&"go.mod".to_string()));
    }

    #[test]
    fn test_plugin_can_handle_project() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create a Cargo.toml file
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

        let rust_plugin = create_rust_plugin();
        assert!(rust_plugin.can_handle_project(project_path));

        let go_plugin = create_go_plugin();
        assert!(!go_plugin.can_handle_project(project_path));
    }

    #[test]
    fn test_plugin_can_handle_by_extension() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create a Rust file
        std::fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();

        let rust_plugin = create_rust_plugin();
        assert!(rust_plugin.can_handle_project(project_path));
    }

    #[test]
    fn test_is_builtin_plugin() {
        assert!(is_builtin_plugin("rust"));
        assert!(is_builtin_plugin("go"));
        assert!(is_builtin_plugin("c"));
        assert!(is_builtin_plugin("assemblyscript"));
        assert!(is_builtin_plugin("python"));
        assert!(!is_builtin_plugin("unknown"));
    }

    #[test]
    fn test_get_builtin_plugin_by_name() {
        let rust_info = get_builtin_plugin_by_name("rust").unwrap();
        assert_eq!(rust_info.name, "rust");

        let unknown_info = get_builtin_plugin_by_name("unknown");
        assert!(unknown_info.is_none());
    }

    #[test]
    fn test_load_all_builtin_plugins() {
        let mut registry = PluginRegistry::new();

        let result = load_all_builtin_plugins(&mut registry);
        assert!(result.is_ok());

        // Check that all plugins were loaded
        assert!(registry.get_plugin("rust").is_some());
        assert!(registry.get_plugin("go").is_some());
        assert!(registry.get_plugin("c").is_some());
        assert!(registry.get_plugin("assemblyscript").is_some());
        assert!(registry.get_plugin("python").is_some());
    }
}
