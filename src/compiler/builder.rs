//! Build system abstraction for different languages and compilation targets

use crate::error::{CompilationResult, Result};
use crate::plugin::manager::PluginManager;
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

/// What every builder implements, built-in and external alike. Detection,
/// cleaning and cloning go through the plugin rather than the builder today,
/// so those methods are only exercised by the plugin tests.
pub trait WasmBuilder: Send + Sync {
    #[allow(dead_code)]
    fn can_handle_project(&self, project_path: &str) -> bool;
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;
    #[allow(dead_code)]
    fn clean(&self, project_path: &str) -> Result<()>;
    #[allow(dead_code)]
    fn clone_box(&self) -> Box<dyn WasmBuilder>;
    fn language_name(&self) -> &str;
    #[allow(dead_code)]
    fn entry_file_candidates(&self) -> &[&str];
    #[allow(dead_code)]
    fn supported_extensions(&self) -> &[&str];
    fn check_dependencies(&self) -> Vec<String>;
    fn validate_project(&self, project_path: &str) -> CompilationResult<()>;

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!("Building {} project...", self.language_name());
        self.build(config)
    }
}

impl BuildConfig {
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

/// Factory for creating builders from plugins
pub struct BuilderFactory;

impl BuilderFactory {
    pub fn create_builder_from_plugin(project_path: &str) -> Option<Box<dyn WasmBuilder>> {
        if let Ok(plugin_manager) = PluginManager::new() {
            plugin_manager.get_builder_for_project(project_path)
        } else {
            None
        }
    }

    pub fn create_builder(language: &crate::compiler::ProjectLanguage) -> Box<dyn WasmBuilder> {
        use crate::compiler::ProjectLanguage;

        match language {
            ProjectLanguage::Rust => Box::new(UnknownBuilder),
            ProjectLanguage::C => Box::new(crate::plugin::languages::c_plugin::CPlugin::new()),
            ProjectLanguage::Asc => Box::new(UnknownBuilder),
            ProjectLanguage::Go => Box::new(UnknownBuilder),
            ProjectLanguage::Python => Box::new(UnknownBuilder),
            ProjectLanguage::Unknown => Box::new(UnknownBuilder),
        }
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

/// Build WASM project using plugin system
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

    // Try plugin-based building first
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

    // Fall back to legacy language detection
    let builder = BuilderFactory::create_builder(language);
    builder.validate_project(project_path)?;

    if verbose {
        builder.build_verbose(&config)
    } else {
        builder.build(&config)
    }
}
