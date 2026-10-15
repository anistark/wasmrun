//! [OS Mode] The host side of the OS kernel: a process table and the virtual
//! filesystem the project is staged into.
//!
//! The program itself runs in the browser VM (`ui/src/os/vm.worker.ts`), so
//! nothing here schedules or executes code. What the host keeps is the record
//! the Kernel Status panel shows, and the workspace `/api/fs` reads and writes.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::runtime::wasi_fs::WasiFilesystem;

/// Process ID type for OS mode
pub type Pid = u32;

/// Process state in the micro-kernel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessState {
    Ready,
    Running,
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
    /// Bytes staged into the workspace for it.
    pub memory_usage: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Core micro-kernel for OS mode
#[derive(Clone)]
pub struct WasmMicroKernel {
    processes: Arc<RwLock<HashMap<Pid, Process>>>,
    wasi_fs: Arc<WasiFilesystem>,
    workspace_root: Arc<PathBuf>,
    next_pid: Arc<Mutex<Pid>>,
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
            wasi_fs,
            workspace_root: Arc::new(workspace_root),
            next_pid: Arc::new(Mutex::new(1)),
        }
    }

    /// Get reference to the WASI filesystem
    pub fn wasi_filesystem(&self) -> &WasiFilesystem {
        &self.wasi_fs
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
        };

        let mut processes = self.processes.write().unwrap();
        processes.insert(pid, process);

        Ok(pid)
    }

    /// Mark a process as running, with the bytes staged for it.
    pub fn mark_running(&self, pid: Pid, staged_bytes: usize) {
        let mut processes = self.processes.write().unwrap();
        if let Some(process) = processes.get_mut(&pid) {
            process.state = ProcessState::Running;
            process.memory_usage = staged_bytes;
        }
    }

    #[cfg(test)]
    pub fn get_process(&self, pid: Pid) -> Option<Process> {
        let processes = self.processes.read().unwrap();
        processes.get(&pid).cloned()
    }

    pub fn kill_process(&self, pid: Pid) -> Result<()> {
        let mut processes = self.processes.write().unwrap();
        if let Some(process) = processes.get_mut(&pid) {
            process.state = ProcessState::Terminated;
        }
        Ok(())
    }

    /// Get memory usage statistics
    pub fn get_memory_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        let processes = self.processes.read().unwrap();

        let total_memory: usize = processes.values().map(|p| p.memory_usage).sum();
        let process_count = processes
            .values()
            .filter(|p| p.state != ProcessState::Terminated)
            .count();

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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_a_killed_process_stops_counting() {
        let kernel = WasmMicroKernel::new();
        let pid = kernel
            .create_process("app".into(), "nodejs".into(), None)
            .unwrap();
        kernel.mark_running(pid, 1234);
        assert_eq!(kernel.get_memory_stats()["process_count"], 1);
        assert_eq!(kernel.get_memory_stats()["total_memory"], 1234);
        kernel.kill_process(pid).unwrap();
        assert_eq!(kernel.get_memory_stats()["process_count"], 0);
        assert_eq!(
            kernel.get_process(pid).unwrap().state,
            ProcessState::Terminated
        );
    }

    #[test]
    fn test_the_vfs_refuses_a_path_out_of_the_workspace() {
        let kernel = WasmMicroKernel::new();
        kernel.init_vfs().unwrap();
        let fs = kernel.wasi_filesystem();
        assert!(fs.read_file("/tmp/../../../../etc/passwd").is_err());
        assert!(fs.write_file("/tmp/../../escape.txt", b"x").is_err());
    }
}
