//! [OS Mode] The OS kernel: which language a project is, its staged files, and
//! the process record the UI shows. The program runs in the browser VM.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::logging::LogTrailSystem;
use crate::runtime::microkernel::{Pid, WasmMicroKernel};
use crate::runtime::registry::LanguageRuntimeRegistry;

/// Multi-language kernel that orchestrates different language runtimes
pub struct MultiLanguageKernel {
    base_kernel: WasmMicroKernel,
    language_registry: LanguageRuntimeRegistry,
    process_languages: Arc<Mutex<HashMap<Pid, String>>>,
    log_system: Arc<LogTrailSystem>,
}

/// Configuration for running projects in OS mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRunConfig {
    pub project_path: String,
    pub language: Option<String>,
    pub port: Option<u16>,
    /// `--watch`
    pub hot_reload: bool,
    pub expose: bool,
    pub tunnel_server: Option<String>,
    pub tunnel_secret: Option<String>,
    pub allow_cors: bool,
}

impl Default for MultiLanguageKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiLanguageKernel {
    pub fn new() -> Self {
        Self {
            base_kernel: WasmMicroKernel::new(),
            language_registry: LanguageRuntimeRegistry::register_builtin_runtimes(),
            process_languages: Arc::new(Mutex::new(HashMap::new())),
            log_system: Arc::new(LogTrailSystem::new()),
        }
    }

    /// Start the multi-language kernel
    pub fn start(&self) -> Result<()> {
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

    /// Detect the project's language (or take the one given) and stage it
    pub fn auto_detect_and_run(&mut self, config: OsRunConfig) -> Result<Pid> {
        let language = match config.language.clone() {
            Some(lang) => lang,
            None => match self
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
            },
        };

        self.run_project_with_language(config, &language)
    }

    pub fn run_project_with_language(
        &mut self,
        config: OsRunConfig,
        language: &str,
    ) -> Result<Pid> {
        let runtime = self
            .language_registry
            .get_runtime(language)
            .ok_or_else(|| anyhow::anyhow!("Runtime not found: {language}"))?;

        let bundle = runtime.prepare_project(&config.project_path)?;
        let pid = runtime.run_project(bundle, &mut self.base_kernel)?;

        self.process_languages
            .lock()
            .unwrap()
            .insert(pid, language.to_string());

        Ok(pid)
    }

    /// Stop a process and clean up associated resources
    pub fn kill_process(&mut self, pid: Pid) -> Result<()> {
        self.process_languages.lock().unwrap().remove(&pid);
        self.base_kernel.kill_process(pid)
    }

    /// Get kernel statistics
    pub fn get_statistics(&self) -> KernelStatistics {
        let memory_stats = self.base_kernel.get_memory_stats();
        // The languages of the processes still alive, which is what the UI
        // labels "active runtimes".
        let mut active_runtimes: Vec<String> = {
            let languages = self.process_languages.lock().unwrap();
            languages.values().cloned().collect()
        };
        active_runtimes.sort();
        active_runtimes.dedup();

        let wasi_capabilities = vec![
            "wasi_snapshot_preview1".to_string(),
            "filesystem".to_string(),
            "networking".to_string(),
            "process".to_string(),
        ];

        KernelStatistics {
            total_memory_usage: memory_stats.get("total_memory").copied().unwrap_or(0),
            active_processes: memory_stats.get("process_count").copied().unwrap_or(0),
            active_runtimes,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            kernel_version: env!("CARGO_PKG_VERSION").to_string(),
            wasi_capabilities,
            filesystem_mounts: self.base_kernel.wasi_filesystem().get_stats().total_mounts,
            supported_languages: self.language_registry.list_runtimes(),
        }
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
    fn test_kernel_start() {
        let kernel = MultiLanguageKernel::new();
        assert!(kernel.start().is_ok());
    }

    #[test]
    fn test_a_nodejs_project_is_staged_and_killed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.js"), "console.log(1)").unwrap();
        let mut kernel = MultiLanguageKernel::new();
        kernel.start().unwrap();
        let config = OsRunConfig {
            project_path: dir.path().to_string_lossy().to_string(),
            language: None,
            port: None,
            hot_reload: false,
            expose: false,
            tunnel_server: None,
            tunnel_secret: None,
            allow_cors: false,
        };
        let pid = kernel.auto_detect_and_run(config).unwrap();
        let stats = kernel.get_statistics();
        assert_eq!(stats.active_processes, 1);
        assert_eq!(stats.active_runtimes, vec!["nodejs"]);
        assert!(kernel
            .wasi_filesystem()
            .read_file(&format!("/projects/{pid}/index.js"))
            .is_ok());
        kernel.kill_process(pid).unwrap();
        let stats = kernel.get_statistics();
        assert_eq!(stats.active_processes, 0);
        assert!(stats.active_runtimes.is_empty());
    }
}
