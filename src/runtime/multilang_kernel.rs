use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::logging::LogTrailSystem;
use crate::runtime::dev_server::DevServerManager;
use crate::runtime::microkernel::{Pid, WasmInstance, WasmMicroKernel};
use crate::runtime::registry::{DevServerStatus, LanguageRuntimeRegistry};
use crate::runtime::syscalls::{SyscallArgs, SyscallHandler, SyscallResult};

/// Multi-language kernel that orchestrates different language runtimes
pub struct MultiLanguageKernel {
    base_kernel: WasmMicroKernel,
    language_registry: LanguageRuntimeRegistry,
    active_runtimes: Arc<Mutex<HashMap<String, WasmInstance>>>,
    dev_server_manager: Arc<DevServerManager>,
    syscall_handler: Arc<Mutex<SyscallHandler>>,
    process_languages: Arc<Mutex<HashMap<Pid, String>>>,
    log_system: Arc<LogTrailSystem>,
}

/// Configuration for running projects in OS mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRunConfig {
    pub project_path: String,
    pub language: Option<String>,
    pub dev_mode: bool,
    pub port: Option<u16>,
    pub hot_reload: bool,
    pub debugging: bool,
}

impl Default for MultiLanguageKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl MultiLanguageKernel {
    /// Create a new multi-language kernel
    pub fn new() -> Self {
        let base_kernel = WasmMicroKernel::new();
        let syscall_handler = SyscallHandler::new(base_kernel.clone());

        Self {
            base_kernel: base_kernel.clone(),
            language_registry: LanguageRuntimeRegistry::register_builtin_runtimes(),
            active_runtimes: Arc::new(Mutex::new(HashMap::new())),
            dev_server_manager: Arc::new(DevServerManager::new()),
            syscall_handler: Arc::new(Mutex::new(syscall_handler)),
            process_languages: Arc::new(Mutex::new(HashMap::new())),
            log_system: Arc::new(LogTrailSystem::new()),
        }
    }

    /// Start the multi-language kernel
    pub fn start(&self) -> Result<()> {
        self.base_kernel.start_scheduler()?;
        self.base_kernel.init_vfs()?;
        println!("✅ WASI filesystem initialized");
        Ok(())
    }

    /// Mount a project directory into the WASI filesystem
    pub fn mount_project(&self, project_path: &str) -> Result<()> {
        let wasi_fs = self.base_kernel.wasi_filesystem();

        // Extract project name from path
        let project_name = std::path::Path::new(project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");

        let mount_path = format!("/{project_name}");
        wasi_fs.mount(&mount_path, project_path)?;
        println!("✅ Project mounted at {mount_path} -> {project_path}");
        Ok(())
    }

    /// Get reference to the WASI filesystem
    pub fn wasi_filesystem(&self) -> &crate::runtime::wasi_fs::WasiFilesystem {
        self.base_kernel.wasi_filesystem()
    }

    /// Stop the multi-language kernel
    pub fn stop(&self) -> Result<()> {
        // Stop all dev servers
        for (pid, _, _) in self.dev_server_manager.list_servers() {
            let _ = self.dev_server_manager.stop_server(pid);
        }

        // Clean up active runtimes
        {
            let mut runtimes = self.active_runtimes.lock().unwrap();
            runtimes.clear();
        }

        self.base_kernel.stop_scheduler()?;
        Ok(())
    }

    /// Auto-detect language and run a project
    pub fn auto_detect_and_run(&mut self, config: OsRunConfig) -> Result<Pid> {
        let language = match config.language.clone() {
            Some(lang) => lang,
            None => {
                match self
                    .language_registry
                    .detect_project_language(&config.project_path)
                {
                    Some(lang) => lang.to_string(),
                    None => {
                        return Err(anyhow::anyhow!(
                        "Could not auto-detect language for project: {}. Please specify --language",
                        config.project_path
                    ))
                    }
                }
            }
        };

        self.run_project_with_language(config, &language)
    }

    /// Run a project with a specific language runtime
    pub fn run_project_with_language(
        &mut self,
        config: OsRunConfig,
        language: &str,
    ) -> Result<Pid> {
        // 1. Ensure runtime is loaded
        self.ensure_runtime_loaded(language)?;

        // 2. Get the runtime
        let runtime = self
            .language_registry
            .get_runtime(language)
            .ok_or_else(|| anyhow::anyhow!("Runtime not found: {language}"))?;

        // 3. Prepare project bundle
        let bundle = runtime.prepare_project(&config.project_path)?;

        // 4. Run the project
        let pid = runtime.run_project(bundle, &mut self.base_kernel)?;

        // 5. Track the process language
        {
            let mut process_languages = self.process_languages.lock().unwrap();
            process_languages.insert(pid, language.to_string());
        }

        // 6. Set up development features if enabled
        if config.dev_mode {
            self.setup_dev_environment(pid, language, &config)?;
        }

        Ok(pid)
    }

    /// Ensure a language runtime is loaded and available
    fn ensure_runtime_loaded(&mut self, runtime_name: &str) -> Result<()> {
        {
            let runtimes = self.active_runtimes.lock().unwrap();
            if runtimes.contains_key(runtime_name) {
                return Ok(()); // Already loaded
            }
        }

        let runtime = self
            .language_registry
            .get_runtime(runtime_name)
            .ok_or_else(|| anyhow::anyhow!("Runtime not found: {runtime_name}"))?;

        // Load the runtime WASM binary
        let wasm_binary = runtime.load_wasm_binary()?;

        // Create WASM instance
        let instance = WasmInstance {
            binary: wasm_binary,
            exports: HashMap::new(),
            memory_regions: vec![],
        };

        // Store the instance
        {
            let mut runtimes = self.active_runtimes.lock().unwrap();
            runtimes.insert(runtime_name.to_string(), instance);
        }

        Ok(())
    }

    /// Set up development environment for a process
    fn setup_dev_environment(&self, pid: Pid, language: &str, config: &OsRunConfig) -> Result<()> {
        let runtime = self
            .language_registry
            .get_runtime(language)
            .ok_or_else(|| anyhow::anyhow!("Runtime not found: {language}"))?;

        // Set up development server
        if runtime.create_dev_server().is_some() {
            let port = config.port.unwrap_or_else(|| 8000 + (pid as u16));
            let project_root = format!("/projects/{pid}");
            self.dev_server_manager
                .start_server(pid, port, project_root)?;
            println!("✅ Dev server started for PID {pid} on port {port}");
        }

        // TODO: Set up hot reload if enabled
        if config.hot_reload && runtime.supports_hot_reload() {
            self.setup_hot_reload(pid, language)?;
        }

        // TODO: Set up debugging if enabled
        if config.debugging && runtime.supports_debugging() {
            self.setup_debugging(pid, language)?;
        }

        Ok(())
    }

    fn setup_hot_reload(&self, _pid: Pid, _language: &str) -> Result<()> {
        // TODO: File watchers, reload mechanisms, process restarts
        Ok(())
    }

    fn setup_debugging(&self, _pid: Pid, _language: &str) -> Result<()> {
        // TODO: Debug protocols, breakpoint management, variable inspection
        Ok(())
    }

    /// Handle a system call from a process
    pub fn handle_syscall(
        &mut self,
        pid: Pid,
        syscall_num: u32,
        args: SyscallArgs,
    ) -> SyscallResult {
        // First try language-specific syscall handling
        if let Some(language) = self.get_process_language(pid) {
            if let Some(runtime) = self.language_registry.get_runtime(&language) {
                match runtime.handle_syscall(pid, syscall_num, args.clone()) {
                    SyscallResult::Success(result) => return SyscallResult::Success(result),
                    SyscallResult::Error(_) => {
                        // Fall through to generic syscall handling
                    }
                }
            }
        }

        // Fall back to generic syscall handling
        let mut handler = self.syscall_handler.lock().unwrap();
        handler.handle_syscall(pid, syscall_num, args)
    }

    /// Get the language for a process
    pub fn get_process_language(&self, pid: Pid) -> Option<String> {
        let process_languages = self.process_languages.lock().unwrap();
        process_languages.get(&pid).cloned()
    }

    /// List all active processes with their languages
    pub fn list_processes_with_languages(&self) -> Vec<(Pid, String, String)> {
        let processes = self.base_kernel.list_processes();
        let process_languages = self.process_languages.lock().unwrap();

        processes
            .into_iter()
            .map(|process| {
                let language = process_languages
                    .get(&process.pid)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                (process.pid, process.name, language)
            })
            .collect()
    }

    /// Get development server status for a process
    pub fn get_dev_server_status(&self, pid: Pid) -> Option<DevServerStatus> {
        self.dev_server_manager.get_status(pid)
    }

    /// Stop a process and clean up associated resources
    pub fn kill_process(&mut self, pid: Pid) -> Result<()> {
        // Stop dev server if running
        let _ = self.dev_server_manager.stop_server(pid);

        // Remove language tracking
        {
            let mut process_languages = self.process_languages.lock().unwrap();
            process_languages.remove(&pid);
        }

        // Kill the process in the base kernel
        self.base_kernel.kill_process(pid)?;

        Ok(())
    }

    /// Get kernel statistics
    pub fn get_statistics(&self) -> KernelStatistics {
        let memory_stats = self.base_kernel.get_memory_stats();
        let active_runtimes = {
            let runtimes = self.active_runtimes.lock().unwrap();
            runtimes.keys().cloned().collect()
        };
        let active_dev_servers = self.dev_server_manager.list_servers().len();

        // Get system information
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let kernel_version = env!("CARGO_PKG_VERSION").to_string();

        // Get WASI capabilities
        let wasi_capabilities = vec![
            "wasi_snapshot_preview1".to_string(),
            "filesystem".to_string(),
            "networking".to_string(),
            "process".to_string(),
        ];

        // Get filesystem mount count
        let wasi_fs = self.base_kernel.wasi_filesystem();
        let fs_stats = wasi_fs.get_stats();
        let filesystem_mounts = fs_stats.total_mounts;

        // Get supported languages from registry
        let supported_languages = self.language_registry.list_runtimes();

        KernelStatistics {
            total_memory_usage: memory_stats.get("total_memory").copied().unwrap_or(0),
            active_processes: memory_stats.get("process_count").copied().unwrap_or(0),
            active_runtimes,
            active_dev_servers,
            os,
            arch,
            kernel_version,
            wasi_capabilities,
            filesystem_mounts,
            supported_languages,
        }
    }

    /// Get reference to the base kernel
    pub fn base_kernel(&self) -> &WasmMicroKernel {
        &self.base_kernel
    }

    /// Get reference to the language registry
    pub fn registry(&self) -> &LanguageRuntimeRegistry {
        &self.language_registry
    }

    /// Get mutable reference to the language registry
    pub fn registry_mut(&mut self) -> &mut LanguageRuntimeRegistry {
        &mut self.language_registry
    }

    /// Get reference to the log system
    pub fn log_system(&self) -> Arc<LogTrailSystem> {
        Arc::clone(&self.log_system)
    }
}

/// Statistics about the multi-language kernel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelStatistics {
    pub total_memory_usage: usize,
    pub active_processes: usize,
    pub active_runtimes: Vec<String>,
    pub active_dev_servers: usize,
    // System information
    pub os: String,
    pub arch: String,
    pub kernel_version: String,
    // WASI capabilities
    pub wasi_capabilities: Vec<String>,
    pub filesystem_mounts: usize,
    pub supported_languages: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_creation() {
        let kernel = MultiLanguageKernel::new();
        let stats = kernel.get_statistics();
        assert_eq!(stats.active_processes, 0);
        assert_eq!(stats.active_runtimes.len(), 0);
    }

    #[test]
    fn test_kernel_start_stop() {
        let kernel = MultiLanguageKernel::new();
        assert!(kernel.start().is_ok());
        assert!(kernel.stop().is_ok());
    }
}
