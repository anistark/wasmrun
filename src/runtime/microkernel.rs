use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::runtime::scheduler::ProcessScheduler;
use crate::runtime::wasi_fs::WasiFilesystem;

/// Process ID type for OS mode
pub type Pid = u32;

/// Process state in the micro-kernel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

/// Process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Process {
    pub pid: Pid,
    pub parent_pid: Option<Pid>,
    pub name: String,
    pub language: String,
    pub state: ProcessState,
    pub memory_usage: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip)]
    #[allow(dead_code)]
    pub has_network: bool,
}

/// Memory region for WASM processes (used when WASM execution is implemented)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: usize,
    pub size: usize,
    pub permissions: MemoryPermissions,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MemoryPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

/// Virtual file system entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsEntry {
    pub path: String,
    pub is_directory: bool,
    pub size: Option<usize>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
pub trait SyscallInterface: Send + Sync {
    fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    fn list_directory(&self, path: &str) -> Result<Vec<VfsEntry>>;
    fn create_directory(&self, path: &str) -> Result<()>;
    fn delete_file(&self, path: &str) -> Result<()>;
}

/// WASM instance wrapper (fields read when WASM execution is implemented)
#[allow(dead_code)]
pub struct WasmInstance {
    pub binary: Vec<u8>,
    pub exports: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
    pub memory_regions: Vec<MemoryRegion>,
}

/// Core micro-kernel for OS mode
#[derive(Clone)]
pub struct WasmMicroKernel {
    processes: Arc<RwLock<HashMap<Pid, Process>>>,
    wasm_instances: Arc<RwLock<HashMap<Pid, WasmInstance>>>,
    wasi_fs: Arc<WasiFilesystem>,
    workspace_root: Arc<PathBuf>,
    next_pid: Arc<Mutex<Pid>>,
    scheduler: Arc<ProcessScheduler>,
    scheduler_running: Arc<Mutex<bool>>,
}

impl Default for WasmMicroKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl WasmMicroKernel {
    pub fn new() -> Self {
        let workspace_root = std::env::temp_dir().join(format!("wasmrun-{}", std::process::id()));
        std::fs::create_dir_all(&workspace_root).expect("Failed to create workspace root");

        let wasi_fs = Arc::new(WasiFilesystem::new());
        wasi_fs
            .mount("/", &workspace_root)
            .expect("Failed to mount workspace root");

        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            wasm_instances: Arc::new(RwLock::new(HashMap::new())),
            wasi_fs,
            workspace_root: Arc::new(workspace_root),
            next_pid: Arc::new(Mutex::new(1)),
            scheduler: Arc::new(ProcessScheduler::new()),
            scheduler_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Get reference to the WASI filesystem
    pub fn wasi_filesystem(&self) -> &WasiFilesystem {
        &self.wasi_fs
    }

    /// Get a shared handle to the WASI filesystem
    pub fn wasi_filesystem_arc(&self) -> Arc<WasiFilesystem> {
        Arc::clone(&self.wasi_fs)
    }

    pub fn start_scheduler(&self) -> Result<()> {
        let mut running = self.scheduler_running.lock().unwrap();
        if *running {
            return Ok(());
        }
        *running = true;

        let processes = self.processes.read().unwrap();
        for (pid, process) in processes.iter() {
            if process.state == ProcessState::Ready {
                self.scheduler.add_process(*pid);
            }
        }

        Ok(())
    }

    /// Stop the kernel scheduler
    pub fn stop_scheduler(&self) -> Result<()> {
        let mut running = self.scheduler_running.lock().unwrap();
        *running = false;
        Ok(())
    }

    pub fn create_process(
        &self,
        name: String,
        language: String,
        parent_pid: Option<Pid>,
    ) -> Result<Pid> {
        let pid = {
            let mut next_pid = self.next_pid.lock().unwrap();
            let current_pid = *next_pid;
            *next_pid += 1;
            current_pid
        };

        let process = Process {
            pid,
            parent_pid,
            name,
            language,
            state: ProcessState::Ready,
            memory_usage: 0,
            created_at: chrono::Utc::now(),
            has_network: false,
        };

        let mut processes = self.processes.write().unwrap();
        processes.insert(pid, process);

        let scheduler_running = *self.scheduler_running.lock().unwrap();
        if scheduler_running {
            self.scheduler.add_process(pid);
        }

        Ok(pid)
    }

    /// Get process information
    pub fn get_process(&self, pid: Pid) -> Option<Process> {
        let processes = self.processes.read().unwrap();
        processes.get(&pid).cloned()
    }

    /// List all processes
    pub fn list_processes(&self) -> Vec<Process> {
        let processes = self.processes.read().unwrap();
        processes.values().cloned().collect()
    }

    pub fn kill_process(&self, pid: Pid) -> Result<()> {
        {
            let mut processes = self.processes.write().unwrap();
            if let Some(process) = processes.get_mut(&pid) {
                process.state = ProcessState::Terminated;
            }
        }

        self.scheduler.remove_process(pid);

        let mut instances = self.wasm_instances.write().unwrap();
        instances.remove(&pid);

        Ok(())
    }

    /// Load a WASM module for a process
    pub fn load_wasm_module(&self, pid: Pid, wasm_binary: &[u8]) -> Result<()> {
        let instance = WasmInstance {
            binary: wasm_binary.to_vec(),
            exports: HashMap::new(),
            memory_regions: vec![],
        };

        let mut instances = self.wasm_instances.write().unwrap();
        instances.insert(pid, instance);

        // Update process state
        {
            let mut processes = self.processes.write().unwrap();
            if let Some(process) = processes.get_mut(&pid) {
                process.state = ProcessState::Running;
                process.memory_usage = wasm_binary.len();
            }
        }

        Ok(())
    }

    /// Get memory usage statistics
    pub fn get_memory_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        let processes = self.processes.read().unwrap();

        let total_memory: usize = processes.values().map(|p| p.memory_usage).sum();
        let process_count = processes.len();

        stats.insert("total_memory".to_string(), total_memory);
        stats.insert("process_count".to_string(), process_count);

        stats
    }

    /// Initialize the virtual file system with standard directories
    pub fn init_vfs(&self) -> Result<()> {
        for dir in &["tmp", "home", "usr/bin", "projects"] {
            std::fs::create_dir_all(self.workspace_root.join(dir))?;
        }
        Ok(())
    }

    /// Ensure a process workspace directory exists and return its virtual path
    pub fn ensure_process_workspace(&self, pid: Pid) -> Result<String> {
        let dir = self.workspace_root.join(format!("projects/{pid}"));
        std::fs::create_dir_all(&dir)?;
        Ok(format!("/projects/{pid}"))
    }
}

fn validate_path(path: &str) -> Result<()> {
    if path.split('/').any(|seg| seg == "..") {
        anyhow::bail!("Path traversal not allowed: {path}");
    }
    if !path.starts_with('/') {
        anyhow::bail!("Paths must be absolute: {path}");
    }
    Ok(())
}

impl SyscallInterface for WasmMicroKernel {
    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        validate_path(path)?;
        self.wasi_fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<()> {
        validate_path(path)?;
        self.wasi_fs.write_file(path, data)
    }

    fn list_directory(&self, path: &str) -> Result<Vec<VfsEntry>> {
        validate_path(path)?;
        let entries = self.wasi_fs.path_readdir(path)?;
        let now = chrono::Utc::now();
        Ok(entries
            .into_iter()
            .map(|e| VfsEntry {
                path: format!("{}/{}", path.trim_end_matches('/'), e.name),
                is_directory: e.is_dir,
                size: if e.is_dir {
                    None
                } else {
                    Some(e.size as usize)
                },
                created_at: now,
                modified_at: now,
            })
            .collect())
    }

    fn create_directory(&self, path: &str) -> Result<()> {
        validate_path(path)?;
        self.wasi_fs.path_create_directory(path)
    }

    fn delete_file(&self, path: &str) -> Result<()> {
        validate_path(path)?;
        self.wasi_fs.path_unlink_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_rejects_traversal() {
        assert!(validate_path("/etc/../passwd").is_err());
        assert!(validate_path("/projects/1/../../etc/passwd").is_err());
        assert!(validate_path("/..").is_err());
    }

    #[test]
    fn test_validate_path_rejects_relative() {
        assert!(validate_path("relative/path").is_err());
        assert!(validate_path("../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_path_accepts_valid() {
        assert!(validate_path("/").is_ok());
        assert!(validate_path("/projects/1/file.txt").is_ok());
        assert!(validate_path("/tmp").is_ok());
    }

    #[test]
    fn test_syscall_interface_rejects_traversal() {
        let kernel = WasmMicroKernel::new();
        assert!(kernel.read_file("/tmp/../../etc/passwd").is_err());
        assert!(kernel
            .write_file("/tmp/../../../root/.ssh/id_rsa", b"x")
            .is_err());
        assert!(kernel.list_directory("/projects/../..").is_err());
        assert!(kernel.create_directory("/tmp/../../evil").is_err());
        assert!(kernel.delete_file("/tmp/../../etc/hosts").is_err());
    }

    #[test]
    fn test_syscall_interface_rejects_relative_paths() {
        let kernel = WasmMicroKernel::new();
        assert!(kernel.read_file("relative").is_err());
        assert!(kernel.write_file("no-slash", b"data").is_err());
    }

    #[test]
    fn test_create_process_with_parent() {
        let kernel = WasmMicroKernel::new();
        let parent = kernel
            .create_process("parent".into(), "rust".into(), None)
            .unwrap();
        let child = kernel
            .create_process("child".into(), "rust".into(), Some(parent))
            .unwrap();
        let proc = kernel.get_process(child).unwrap();
        assert_eq!(proc.parent_pid, Some(parent));
    }
}
