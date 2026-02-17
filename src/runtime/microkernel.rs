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

/// Memory region for WASM processes
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: usize,
    pub size: usize,
    pub permissions: MemoryPermissions,
}

/// Memory permissions
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

/// System call interface
#[allow(dead_code)]
pub trait SyscallInterface: Send + Sync {
    fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    fn list_directory(&self, path: &str) -> Result<Vec<VfsEntry>>;
    fn create_directory(&self, path: &str) -> Result<()>;
    fn delete_file(&self, path: &str) -> Result<()>;
}

/// WASM instance wrapper
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn get_process(&self, pid: Pid) -> Option<Process> {
        let processes = self.processes.read().unwrap();
        processes.get(&pid).cloned()
    }

    /// List all processes
    #[allow(dead_code)]
    pub fn list_processes(&self) -> Vec<Process> {
        let processes = self.processes.read().unwrap();
        processes.values().cloned().collect()
    }

    #[allow(dead_code)]
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

impl SyscallInterface for WasmMicroKernel {
    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.wasi_fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<()> {
        self.wasi_fs.write_file(path, data)
    }

    fn list_directory(&self, path: &str) -> Result<Vec<VfsEntry>> {
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
        self.wasi_fs.path_create_directory(path)
    }

    fn delete_file(&self, path: &str) -> Result<()> {
        self.wasi_fs.path_unlink_file(path)
    }
}
