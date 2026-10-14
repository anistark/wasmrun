//! [OS Mode] The language runtimes the OS kernel can stage a project for.
//!
//! A runtime here does not execute anything: the program runs in the browser
//! VM, on a runtime fetched from wasmhub. What it does is the host's half,
//! recognising a project and staging its files in the kernel's workspace.

use crate::runtime::microkernel::{Pid, WasmMicroKernel};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Project bundle containing all necessary files and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBundle {
    pub name: String,
    pub language: String,
    pub entry_point: String,
    pub files: HashMap<String, Vec<u8>>,
    pub dependencies: Vec<String>,
    pub metadata: ProjectMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
}

/// One language the OS kernel can stage a project for.
pub trait LanguageRuntime: Send + Sync {
    /// Detect if a project path contains a project for this language
    fn detect_project(&self, project_path: &str) -> bool;

    /// Prepare a project bundle from a project path
    fn prepare_project(&self, project_path: &str) -> Result<ProjectBundle>;

    /// Record the project as a kernel process and stage its files
    fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid>;
}

/// Registry for managing language runtimes
pub struct LanguageRuntimeRegistry {
    runtimes: HashMap<String, Box<dyn LanguageRuntime>>,
}

impl Default for LanguageRuntimeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageRuntimeRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            runtimes: HashMap::new(),
        }
    }

    /// Every runtime OS mode can run. Node.js only: wasmhub's Python runtime
    /// does not exist yet, and its Go and Rust artifacts are prebuilt programs
    /// rather than toolchains that could run a project.
    pub fn register_builtin_runtimes() -> Self {
        use crate::runtime::languages::nodejs::NodeJSRuntime;

        let mut registry = Self::new();
        registry.register("nodejs", Box::new(NodeJSRuntime::new()));
        registry
    }

    /// Register a new language runtime
    pub fn register(&mut self, name: &str, runtime: Box<dyn LanguageRuntime>) {
        self.runtimes.insert(name.to_string(), runtime);
    }

    /// Get a language runtime by name
    pub fn get_runtime(&self, name: &str) -> Option<&dyn LanguageRuntime> {
        self.runtimes.get(name).map(|r| r.as_ref())
    }

    /// List all registered runtime names
    pub fn list_runtimes(&self) -> Vec<String> {
        self.runtimes.keys().cloned().collect()
    }

    /// Auto-detect the language of a project
    pub fn detect_project_language(&self, project_path: &str) -> Option<&str> {
        self.runtimes
            .iter()
            .find(|(_, runtime)| runtime.detect_project(project_path))
            .map(|(name, _)| name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRuntime;

    impl LanguageRuntime for MockRuntime {
        fn detect_project(&self, project_path: &str) -> bool {
            project_path.ends_with("mock-project")
        }

        fn prepare_project(&self, _project_path: &str) -> Result<ProjectBundle> {
            Ok(ProjectBundle {
                name: "mock".to_string(),
                language: "mock".to_string(),
                entry_point: "main.mock".to_string(),
                files: HashMap::new(),
                dependencies: vec![],
                metadata: ProjectMetadata {
                    version: "1.0.0".to_string(),
                    description: None,
                    author: None,
                    license: None,
                },
            })
        }

        fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid> {
            kernel.create_process(bundle.name, bundle.language, None)
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = LanguageRuntimeRegistry::new();
        assert_eq!(registry.list_runtimes().len(), 0);
    }

    #[test]
    fn test_runtime_registration() {
        let mut registry = LanguageRuntimeRegistry::new();
        registry.register("mock", Box::new(MockRuntime));
        assert_eq!(registry.list_runtimes(), vec!["mock"]);
        assert!(registry.get_runtime("mock").is_some());
    }

    #[test]
    fn test_detection_names_the_matching_runtime() {
        let mut registry = LanguageRuntimeRegistry::new();
        registry.register("mock", Box::new(MockRuntime));
        assert_eq!(
            registry.detect_project_language("/x/mock-project"),
            Some("mock")
        );
        assert_eq!(registry.detect_project_language("/x/other"), None);
    }

    #[test]
    fn test_builtin_registry_is_nodejs() {
        assert_eq!(
            LanguageRuntimeRegistry::register_builtin_runtimes().list_runtimes(),
            vec!["nodejs"]
        );
    }
}
