use crate::error::{CompilationError, CompilationResult};
use crate::utils::PathResolver;
use std::path::Path;

/// Configuration for building WASM modules
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub project_path: String,
    pub output_dir: String,
    pub verbose: bool,
    pub optimization_level: OptimizationLevel,
    #[allow(dead_code)]
    pub target_type: TargetType,
}

#[derive(Debug, Clone)]
pub enum OptimizationLevel {
    Debug,
    Release,
    Size,
}

/// Type of WASM target
#[derive(Debug, Clone)]
pub enum TargetType {
    Standard,
    #[allow(dead_code)]
    WasmBindgen,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            project_path: "./".to_string(),
            output_dir: "./".to_string(),
            verbose: false,
            optimization_level: OptimizationLevel::Release,
            target_type: TargetType::Standard,
        }
    }
}

/// Result of a build operation
#[derive(Debug)]
pub struct BuildResult {
    pub wasm_path: String,
    pub js_path: Option<String>,
    pub additional_files: Vec<String>,
    #[allow(dead_code)]
    pub is_wasm_bindgen: bool,
}

/// Common interface for all WASM builders
pub trait WasmBuilder: Send + Sync {
    fn language_name(&self) -> &str;
    #[allow(dead_code)]
    fn entry_file_candidates(&self) -> &[&str];
    #[allow(dead_code)]
    fn supported_extensions(&self) -> &[&str];
    fn check_dependencies(&self) -> Vec<String>;
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!(
            "🔨 Building {} project at: {}",
            self.language_name(),
            config.project_path
        );
        self.build(config)
    }

    #[allow(dead_code)]
    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        let path = Path::new(project_path);
        if !path.exists() || !path.is_dir() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Project path not found: {}", project_path),
            });
        }
        Ok(())
    }

    fn validate_config(&self, config: &BuildConfig) -> CompilationResult<()> {
        if !PathResolver::is_safe_path(&config.project_path) {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Unsafe project path: {}", config.project_path),
            });
        }

        if !PathResolver::is_safe_path(&config.output_dir) {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Unsafe output path: {}", config.output_dir),
            });
        }

        Ok(())
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
                    for plugin in plugin_manager.get_plugins() {
                        if plugin.info().name.contains("rust") || plugin.info().name == "wasmrust" {
                            return plugin.get_builder();
                        }
                    }
                }
                Box::new(UnknownBuilder)
            }
            ProjectLanguage::Go => {
                if let Ok(plugin_manager) = crate::plugin::PluginManager::new() {
                    for plugin in plugin_manager.get_plugins() {
                        if plugin.info().name.contains("go") || plugin.info().name == "wasmgo" {
                            return plugin.get_builder();
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

    fn build(&self, _config: &BuildConfig) -> CompilationResult<BuildResult> {
        Err(CompilationError::UnsupportedLanguage {
            language: "Unknown".to_string(),
        })
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
        target_type: TargetType::Standard,
    };

    if let Some(builder) = BuilderFactory::create_builder_from_plugin(project_path) {
        if verbose {
            println!("🔌 Using plugin: {}", builder.language_name());
        }
        builder.validate_config(&config)?;
        return if verbose {
            builder.build_verbose(&config)
        } else {
            builder.build(&config)
        };
    }

    let builder = BuilderFactory::create_builder(language);
    builder.validate_config(&config)?;

    if verbose {
        builder.build_verbose(&config)
    } else {
        builder.build(&config)
    }
}
