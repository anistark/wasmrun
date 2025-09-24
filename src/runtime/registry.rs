use crate::runtime::microkernel::{Pid, SyscallInterface, WasmMicroKernel};
use crate::runtime::syscalls::{SyscallArgs, SyscallResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Project bundle containing all necessary files and metadata
#[allow(dead_code)]
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

/// Development server interface for language runtimes
#[allow(dead_code)]
pub trait DevServer: Send + Sync {
    fn start(&self, port: u16) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn reload(&self) -> Result<()>;
    fn get_status(&self) -> DevServerStatus;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DevServerStatus {
    Stopped,
    Starting,
    Running(u16),
    Error(String),
}

/// Language runtime trait that defines the interface for all language implementations
#[allow(dead_code)]
pub trait LanguageRuntime: Send + Sync {
    /// Get the name of the language runtime
    fn name(&self) -> &str;

    /// Get supported file extensions for this language
    fn extensions(&self) -> &[&str];

    /// Get entry files that indicate a project of this language (e.g., package.json, requirements.txt)
    fn entry_files(&self) -> &[&str];

    /// Load the WASM binary for this language runtime
    fn load_wasm_binary(&self) -> Result<Vec<u8>>;

    /// Create a syscall interface specific to this language
    fn create_syscall_interface(&self) -> Box<dyn SyscallInterface>;

    /// Check if this runtime supports hot reload
    fn supports_hot_reload(&self) -> bool;

    /// Check if this runtime supports debugging
    fn supports_debugging(&self) -> bool;

    /// Create a development server instance
    fn create_dev_server(&self) -> Option<Box<dyn DevServer>>;

    /// Detect if a project path contains a project for this language
    fn detect_project(&self, project_path: &str) -> bool;

    /// Prepare a project bundle from a project path
    fn prepare_project(&self, project_path: &str) -> Result<ProjectBundle>;

    /// Run a project bundle in the micro-kernel
    fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid>;

    /// Handle language-specific syscalls
    fn handle_syscall(&self, pid: Pid, syscall_num: u32, args: SyscallArgs) -> SyscallResult;
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

#[allow(dead_code)]
impl LanguageRuntimeRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            runtimes: HashMap::new(),
        }
    }

    /// Register all built-in language runtimes
    pub fn register_builtin_runtimes() -> Self {
        let mut registry = Self::new();

        // Register built-in runtimes
        {
            use crate::runtime::languages::nodejs::NodeJSRuntime;

            // Register implemented runtimes
            registry.register("nodejs", Box::new(NodeJSRuntime::new()));

            // TODO: Uncomment when other language runtimes are fully implemented
            // registry.register("python", Box::new(PythonRuntime::new()));
            // registry.register("go", Box::new(GoRuntime::new()));
        }

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

    /// Get a mutable reference to a language runtime by name
    pub fn get_runtime_mut(&mut self, name: &str) -> Option<&mut dyn LanguageRuntime> {
        if let Some(runtime) = self.runtimes.get_mut(name) {
            Some(runtime.as_mut())
        } else {
            None
        }
    }

    /// List all registered runtime names
    pub fn list_runtimes(&self) -> Vec<String> {
        self.runtimes.keys().cloned().collect()
    }

    /// Auto-detect the language of a project
    pub fn detect_project_language(&self, project_path: &str) -> Option<&str> {
        for (name, runtime) in &self.runtimes {
            if runtime.detect_project(project_path) {
                return Some(name);
            }
        }
        None
    }

    /// Get runtime by file extension
    pub fn get_runtime_by_extension(&self, extension: &str) -> Option<&str> {
        for (name, runtime) in &self.runtimes {
            if runtime.extensions().contains(&extension) {
                return Some(name);
            }
        }
        None
    }

    /// Remove a runtime from the registry
    pub fn unregister(&mut self, name: &str) -> Option<Box<dyn LanguageRuntime>> {
        self.runtimes.remove(name)
    }

    /// Check if a runtime is registered
    pub fn has_runtime(&self, name: &str) -> bool {
        self.runtimes.contains_key(name)
    }

    /// Get runtime information
    pub fn get_runtime_info(&self, name: &str) -> Option<RuntimeInfo> {
        self.runtimes.get(name).map(|runtime| RuntimeInfo {
            name: runtime.name().to_string(),
            extensions: runtime.extensions().iter().map(|s| s.to_string()).collect(),
            entry_files: runtime
                .entry_files()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            supports_hot_reload: runtime.supports_hot_reload(),
            supports_debugging: runtime.supports_debugging(),
        })
    }

    /// Get all runtime information
    pub fn get_all_runtime_info(&self) -> Vec<RuntimeInfo> {
        self.runtimes
            .keys()
            .filter_map(|name| self.get_runtime_info(name))
            .collect()
    }
}

/// Information about a language runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub name: String,
    pub extensions: Vec<String>,
    pub entry_files: Vec<String>,
    pub supports_hot_reload: bool,
    pub supports_debugging: bool,
}

/// Base implementation for language runtimes
#[allow(dead_code)]
pub struct BaseRuntime {
    pub name: String,
    pub extensions: Vec<String>,
    pub entry_files: Vec<String>,
    pub wasm_binary_path: Option<String>,
    pub supports_hot_reload: bool,
    pub supports_debugging: bool,
}

#[allow(dead_code)]
impl BaseRuntime {
    pub fn new(name: String) -> Self {
        Self {
            name,
            extensions: vec![],
            entry_files: vec![],
            wasm_binary_path: None,
            supports_hot_reload: false,
            supports_debugging: false,
        }
    }

    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.extensions = extensions;
        self
    }

    pub fn with_entry_files(mut self, entry_files: Vec<String>) -> Self {
        self.entry_files = entry_files;
        self
    }

    pub fn with_wasm_binary(mut self, path: String) -> Self {
        self.wasm_binary_path = Some(path);
        self
    }

    pub fn with_hot_reload(mut self, enabled: bool) -> Self {
        self.supports_hot_reload = enabled;
        self
    }

    pub fn with_debugging(mut self, enabled: bool) -> Self {
        self.supports_debugging = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRuntime {
        base: BaseRuntime,
    }

    impl MockRuntime {
        fn new() -> Self {
            Self {
                base: BaseRuntime::new("mock".to_string())
                    .with_extensions(vec!["mock".to_string()])
                    .with_entry_files(vec!["mock.toml".to_string()]),
            }
        }
    }

    impl LanguageRuntime for MockRuntime {
        fn name(&self) -> &str {
            &self.base.name
        }

        fn extensions(&self) -> &[&str] {
            // Convert Vec<String> to &[&str] for testing
            &[]
        }

        fn entry_files(&self) -> &[&str] {
            &[]
        }

        fn load_wasm_binary(&self) -> Result<Vec<u8>> {
            Ok(vec![0x00, 0x61, 0x73, 0x6d]) // WASM magic number
        }

        fn create_syscall_interface(&self) -> Box<dyn SyscallInterface> {
            unimplemented!("Mock runtime doesn't implement syscall interface")
        }

        fn supports_hot_reload(&self) -> bool {
            self.base.supports_hot_reload
        }

        fn supports_debugging(&self) -> bool {
            self.base.supports_debugging
        }

        fn create_dev_server(&self) -> Option<Box<dyn DevServer>> {
            None
        }

        fn detect_project(&self, _project_path: &str) -> bool {
            false
        }

        fn prepare_project(&self, _project_path: &str) -> Result<ProjectBundle> {
            Ok(ProjectBundle {
                name: "mock-project".to_string(),
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

        fn run_project(
            &self,
            _bundle: ProjectBundle,
            _kernel: &mut WasmMicroKernel,
        ) -> Result<Pid> {
            Ok(1)
        }

        fn handle_syscall(
            &self,
            _pid: Pid,
            _syscall_num: u32,
            _args: SyscallArgs,
        ) -> SyscallResult {
            SyscallResult::Error("Mock runtime doesn't handle syscalls".to_string())
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
        let mock_runtime = MockRuntime::new();
        registry.register("mock", Box::new(mock_runtime));

        assert!(registry.has_runtime("mock"));
        assert_eq!(registry.list_runtimes(), vec!["mock"]);
    }

    #[test]
    fn test_runtime_retrieval() {
        let mut registry = LanguageRuntimeRegistry::new();
        let mock_runtime = MockRuntime::new();
        registry.register("mock", Box::new(mock_runtime));

        let runtime = registry.get_runtime("mock");
        assert!(runtime.is_some());
        assert_eq!(runtime.unwrap().name(), "mock");
    }

    #[test]
    fn test_runtime_unregistration() {
        let mut registry = LanguageRuntimeRegistry::new();
        let mock_runtime = MockRuntime::new();
        registry.register("mock", Box::new(mock_runtime));

        assert!(registry.has_runtime("mock"));
        let removed = registry.unregister("mock");
        assert!(removed.is_some());
        assert!(!registry.has_runtime("mock"));
    }
}
