//! Build system abstraction for different languages and compilation targets

use crate::error::{CompilationResult, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub project_path: String,
    pub output_dir: String,
    pub optimization_level: OptimizationLevel,
    pub verbose: bool,
    pub watch: bool,
    pub target_type: TargetType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationLevel {
    Debug,
    Release,
    Size,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TargetType {
    Standard,
    Web,
}

impl fmt::Display for OptimizationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptimizationLevel::Debug => write!(f, "debug"),
            OptimizationLevel::Release => write!(f, "release"),
            OptimizationLevel::Size => write!(f, "size"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub wasm_path: String,
    pub js_path: Option<String>,
    pub additional_files: Vec<String>,
    pub is_wasm_bindgen: bool,
}

pub trait WasmBuilder: Send + Sync {
    fn can_handle_project(&self, project_path: &str) -> bool;
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;

    // TODO: Implement clean functionality for build artifacts cleanup
    #[allow(dead_code)]
    fn clean(&self, project_path: &str) -> Result<()>;

    // TODO: Implement plugin cloning for dynamic loading
    #[allow(dead_code)]
    fn clone_box(&self) -> Box<dyn WasmBuilder>;

    fn language_name(&self) -> &str;
    fn entry_file_candidates(&self) -> &[&str];
    fn supported_extensions(&self) -> &[&str];
    fn check_dependencies(&self) -> Vec<String>;
    fn validate_project(&self, project_path: &str) -> CompilationResult<()>;

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!("Building {} project...", self.language_name());
        self.build(config)
    }
}

// Helper trait to enable cloning of WasmBuilder trait objects
pub trait CloneableWasmBuilder: WasmBuilder {
    // TODO: Implement cloneable builder for plugin management
    #[allow(dead_code)]
    fn clone_boxed(&self) -> Box<dyn WasmBuilder>;
}

impl<T> CloneableWasmBuilder for T
where
    T: WasmBuilder + Clone + 'static,
{
    fn clone_boxed(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }
}

// Each WasmBuilder implementation should implement the trait directly
impl BuildConfig {
    // TODO: Use in future build configuration UI
    #[allow(dead_code)]
    pub fn new(
        project_path: String,
        output_dir: String,
        optimization_level: OptimizationLevel,
        verbose: bool,
        watch: bool,
    ) -> Self {
        Self {
            project_path,
            output_dir,
            optimization_level,
            verbose,
            watch,
            target_type: TargetType::Standard,
        }
    }

    pub fn with_defaults(project_path: String, output_dir: String) -> Self {
        Self {
            project_path,
            output_dir,
            optimization_level: OptimizationLevel::Release,
            verbose: false,
            watch: false,
            target_type: TargetType::Standard,
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self::with_defaults(".".to_string(), "./dist".to_string())
    }
}

impl BuildResult {
    // TODO: Use these constructors in plugin implementations
    #[allow(dead_code)]
    pub fn new(wasm_path: String) -> Self {
        Self {
            wasm_path,
            js_path: None,
            additional_files: Vec::new(),
            is_wasm_bindgen: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_js(wasm_path: String, js_path: String) -> Self {
        Self {
            wasm_path,
            js_path: Some(js_path),
            additional_files: Vec::new(),
            is_wasm_bindgen: true,
        }
    }

    #[allow(dead_code)]
    pub fn web_app(app_dir: String, index_path: String) -> Self {
        Self {
            wasm_path: app_dir,
            js_path: Some(index_path),
            additional_files: Vec::new(),
            is_wasm_bindgen: false,
        }
    }

    #[allow(dead_code)]
    pub fn get_primary_file(&self) -> &str {
        self.js_path.as_ref().unwrap_or(&self.wasm_path)
    }

    #[allow(dead_code)]
    pub fn is_web_app(&self) -> bool {
        self.js_path.as_ref()
            .map(|js| js.ends_with("index.html"))
            .unwrap_or(false)
    }
}

/// Factory for creating builders
pub struct BuilderFactory;

impl BuilderFactory {
    pub fn create_builder(language: &crate::compiler::ProjectLanguage) -> Box<dyn WasmBuilder> {
        use crate::compiler::ProjectLanguage;
        use crate::plugin::languages::{
            asc_plugin::AscPlugin, c_plugin::CPlugin, python_plugin::PythonPlugin,
        };

        match language {
            ProjectLanguage::Rust => {
                if let Ok(plugin_manager) = crate::plugin::PluginManager::new() {
                    for plugin in plugin_manager.list_plugins() {
                        if plugin.name.contains("rust") || plugin.name == "wasmrust" {
                            if let Some(found_plugin) = plugin_manager.get_plugin_by_name(&plugin.name) {
                                return found_plugin.get_builder();
                            }
                        }
                    }
                }
                Box::new(UnknownBuilder)
            }
            ProjectLanguage::Go => {
                if let Ok(plugin_manager) = crate::plugin::PluginManager::new() {
                    for plugin in plugin_manager.list_plugins() {
                        if plugin.name.contains("go") || plugin.name == "wasmgo" {
                            if let Some(found_plugin) = plugin_manager.get_plugin_by_name(&plugin.name) {
                                return found_plugin.get_builder();
                            }
                        }
                    }
                }
                Box::new(UnknownBuilder)
            }
            ProjectLanguage::C => Box::new(CPlugin::new()),
            ProjectLanguage::Asc => Box::new(AscPlugin::new()),
            ProjectLanguage::Python => Box::new(PythonPlugin::new()),
            ProjectLanguage::Unknown => Box::new(UnknownBuilder),
        }
    }

    pub fn create_builder_from_plugin(project_path: &str) -> Option<Box<dyn WasmBuilder>> {
        if let Ok(plugin_manager) = crate::plugin::PluginManager::new() {
            if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
                return Some(plugin.get_builder());
            }
        }
        None
    }

    // TODO: Use in help command to show supported languages
    // or read from dynamic plugin list
    #[allow(dead_code)]
    pub fn supported_languages() -> Vec<String> {
        vec![
            "Rust".to_string(),
            "Go".to_string(),
            "C".to_string(),
            "Asc".to_string(),
            "Python".to_string(),
        ]
    }
}

/// Unknown language builder
#[derive(Clone)]
struct UnknownBuilder;

impl WasmBuilder for UnknownBuilder {
    fn can_handle_project(&self, _project_path: &str) -> bool {
        false
    }

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: "Unknown".to_string(),
        })
    }

    fn clean(&self, _project_path: &str) -> Result<()> {
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }

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

    fn validate_project(&self, _project_path: &str) -> CompilationResult<()> {
        Err(crate::error::CompilationError::UnsupportedLanguage {
            language: "Unknown".to_string(),
        })
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!("❌ Unknown language project");
        self.build(config)
    }
}

/// Build WASM project
pub fn build_wasm_project(
    project_path: &str,
    output_dir: &str,
    language: &crate::compiler::ProjectLanguage,
    verbose: bool,
) -> CompilationResult<BuildResult> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        verbose,
        optimization_level: OptimizationLevel::Release,
        watch: false,
        target_type: TargetType::Standard,
    };

    if let Some(builder) = BuilderFactory::create_builder_from_plugin(project_path) {
        if verbose {
            println!("🔌 Using plugin: {}", builder.language_name());
        }
        builder.validate_project(project_path)?;
        return if verbose {
            builder.build_verbose(&config)
        } else {
            builder.build(&config)
        };
    }

    let builder = BuilderFactory::create_builder(language);
    builder.validate_project(project_path)?;

    if verbose {
        builder.build_verbose(&config)
    } else {
        builder.build(&config)
    }
}
