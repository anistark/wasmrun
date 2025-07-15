//! Built-in plugin implementations

use crate::compiler::builder::WasmBuilder;
use crate::error::Result;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};

/// Wrapper for built-in plugins
pub struct BuiltinPlugin {
    info: PluginInfo,
    #[allow(dead_code)]
    builder: Box<dyn WasmBuilder>,
}

impl BuiltinPlugin {
    /// Create a new built-in plugin. make separate helper?
    pub fn new(
        name: String,
        version: String,
        description: String,
        extensions: Vec<String>,
        entry_files: Vec<String>,
        capabilities: PluginCapabilities,
        builder: Box<dyn WasmBuilder>,
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
        match self.info.name.as_str() {
            "c" => {
                use crate::plugin::languages::c_plugin::CPlugin;
                Box::new(CPlugin::new())
            }
            "asc" => {
                use crate::plugin::languages::asc_plugin::AscPlugin;
                Box::new(AscPlugin::new())
            }
            "python" => {
                use crate::plugin::languages::python_plugin::PythonPlugin;
                Box::new(PythonPlugin::new())
            }
            _ => Box::new(UnknownBuilder),
        }
    }
}

struct UnknownBuilder;

impl WasmBuilder for UnknownBuilder {
    fn language_name(&self) -> &str {
        "Unknown"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &[]
    }

    fn supported_extensions(&self) -> &[&str] {
        &[]
    }

    fn check_dependencies(&self) -> Vec<String> {
        vec!["Language not detected or supported".to_string()]
    }

    fn build(
        &self,
        _config: &crate::compiler::builder::BuildConfig,
    ) -> crate::error::CompilationResult<crate::compiler::builder::BuildResult> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: "Unknown".to_string(),
        })
    }

    fn validate_project(&self, _project_path: &str) -> crate::error::CompilationResult<()> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: "Unknown".to_string(),
        })
    }
}

pub fn get_builtin_plugins() -> Vec<Box<dyn Plugin>> {
    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();

    // C plugin
    let c_plugin = create_c_plugin();
    plugins.push(Box::new(c_plugin));

    // AssemblyScript plugin
    let asc_plugin = create_asc_plugin();
    plugins.push(Box::new(asc_plugin));

    // Python plugin
    let python_plugin = create_python_plugin();
    plugins.push(Box::new(python_plugin));

    plugins
}

/// Create the C built-in plugin
fn create_c_plugin() -> BuiltinPlugin {
    use crate::plugin::languages::c_plugin::CPlugin;

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
        vec![
            "main.c".to_string(),
            "src/main.c".to_string(),
            "app.c".to_string(),
            "index.c".to_string(),
            "main.cpp".to_string(),
            "src/main.cpp".to_string(),
            "app.cpp".to_string(),
            "index.cpp".to_string(),
        ],
        capabilities,
        Box::new(CPlugin::new()),
    )
}

/// Create the AssemblyScript built-in plugin
fn create_asc_plugin() -> BuiltinPlugin {
    use crate::plugin::languages::asc_plugin::AscPlugin;

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
        "AssemblyScript WebAssembly compiler".to_string(),
        vec!["ts".to_string(), "as".to_string()],
        vec![
            "assembly/index.ts".to_string(),
            "src/index.ts".to_string(),
            "index.ts".to_string(),
            "main.ts".to_string(),
        ],
        capabilities,
        Box::new(AscPlugin::new()),
    )
}

/// Create the Python built-in plugin
fn create_python_plugin() -> BuiltinPlugin {
    use crate::plugin::languages::python_plugin::PythonPlugin;

    let capabilities = PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: false,
        optimization: false,
        custom_targets: vec![],
    };

    BuiltinPlugin::new(
        "python".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        "Python WebAssembly runtime using Pyodide".to_string(),
        vec!["py".to_string()],
        vec![
            "main.py".to_string(),
            "app.py".to_string(),
            "index.py".to_string(),
            "src/main.py".to_string(),
        ],
        capabilities,
        Box::new(PythonPlugin::new()),
    )
}

#[allow(dead_code)]
pub fn load_all_builtin_plugins() -> Result<Vec<Box<dyn Plugin>>> {
    let plugins = get_builtin_plugins();
    println!("Loaded {} built-in plugins", plugins.len());
    Ok(plugins)
}
