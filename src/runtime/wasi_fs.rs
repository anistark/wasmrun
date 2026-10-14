//! Custom WASI-compatible filesystem implementation for OS mode
//!
//! This module provides a WASI filesystem that integrates with the wasmrun OS mode kernel.
//! It's a standalone implementation that doesn't depend on external WASM runtimes.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// WASI filesystem manager for OS mode
pub struct WasiFilesystem {
    mounts: Arc<RwLock<HashMap<String, PathBuf>>>,
    config: WasiConfig,
}

/// WASI filesystem configuration
#[derive(Debug, Clone)]
pub struct WasiConfig {
    pub read_only: bool,
    pub max_file_size: usize,
}

impl Default for WasiConfig {
    fn default() -> Self {
        Self {
            read_only: false,
            max_file_size: 100 * 1024 * 1024, // 100 MB
        }
    }
}

impl Default for WasiFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiFilesystem {
    /// Create a new WASI filesystem manager
    pub fn new() -> Self {
        Self::with_config(WasiConfig::default())
    }

    /// Create a WASI filesystem with custom configuration
    pub fn with_config(config: WasiConfig) -> Self {
        Self {
            mounts: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Mount a host directory to a virtual path (WASI preopen)
    ///
    /// # Arguments
    /// * `guest_path` - Virtual path in the WASI environment (e.g., "/project")
    /// * `host_path` - Real filesystem path on the host
    ///
    /// # Example
    /// ```ignore
    /// fs.mount("/project", "/Users/user/my-project")?;
    /// ```
    pub fn mount(&self, guest_path: &str, host_path: impl AsRef<Path>) -> Result<()> {
        let host_path = host_path.as_ref();

        if !host_path.exists() {
            anyhow::bail!(
                "Cannot mount non-existent directory: {}",
                host_path.display()
            );
        }

        if !host_path.is_dir() {
            anyhow::bail!("Cannot mount non-directory path: {}", host_path.display());
        }

        let canonical = host_path.canonicalize()?;
        let mut mounts = self.mounts.write().unwrap();
        mounts.insert(guest_path.to_string(), canonical);

        Ok(())
    }

    /// WASI path_create_directory - Create a directory
    pub fn path_create_directory(&self, virtual_path: &str) -> Result<()> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        let host_path = self.resolve_path(virtual_path)?;
        fs::create_dir_all(&host_path)
            .with_context(|| format!("Failed to create directory: {virtual_path}"))?;
        Ok(())
    }

    /// WASI path_unlink_file - Delete a file
    pub fn path_unlink_file(&self, virtual_path: &str) -> Result<()> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        let host_path = self.resolve_path(virtual_path)?;
        fs::remove_file(&host_path)
            .with_context(|| format!("Failed to unlink file: {virtual_path}"))?;
        Ok(())
    }

    /// WASI path_readdir - Read directory entries
    pub fn path_readdir(&self, virtual_path: &str) -> Result<Vec<DirEntry>> {
        let host_path = self.resolve_path(virtual_path)?;

        let entries: Result<Vec<DirEntry>> = fs::read_dir(&host_path)?
            .map(|entry| {
                let entry = entry?;
                let metadata = entry.metadata()?;
                Ok(DirEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    is_dir: metadata.is_dir(),
                    is_file: metadata.is_file(),
                    size: metadata.len(),
                })
            })
            .collect();

        entries
    }

    /// Read entire file contents (helper function)
    pub fn read_file(&self, virtual_path: &str) -> Result<Vec<u8>> {
        let host_path = self.resolve_path(virtual_path)?;
        let data = fs::read(&host_path)?;

        if data.len() > self.config.max_file_size {
            anyhow::bail!("File size exceeds maximum allowed size");
        }

        Ok(data)
    }

    /// Write entire file contents (helper function)
    pub fn write_file(&self, virtual_path: &str, data: &[u8]) -> Result<()> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        if data.len() > self.config.max_file_size {
            anyhow::bail!("File size exceeds maximum allowed size");
        }

        let host_path = self.resolve_path(virtual_path)?;

        if let Some(parent) = host_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&host_path, data)?;
        Ok(())
    }

    /// Get WASI filesystem statistics
    pub fn get_stats(&self) -> WasiFilesystemStats {
        let mounts = self.mounts.read().unwrap();
        let total_mounts = mounts.len();

        let total_size: u64 = mounts
            .values()
            .filter_map(|path| Self::calculate_dir_size(path).ok())
            .sum();

        WasiFilesystemStats {
            total_mounts,
            total_size,
            mounts: mounts
                .iter()
                .map(|(k, v)| MountInfo {
                    guest_path: k.clone(),
                    host_path: v.clone(),
                    size: Self::calculate_dir_size(v).unwrap_or(0),
                })
                .collect(),
        }
    }

    /// Resolve a virtual WASI path to a real host path.
    /// Uses longest-prefix matching so specific mounts shadow broader ones.
    fn resolve_path(&self, virtual_path: &str) -> Result<PathBuf> {
        let mounts = self.mounts.read().unwrap();

        let best_match = mounts
            .iter()
            .filter(|(guest_path, _)| {
                virtual_path == guest_path.as_str()
                    || virtual_path.starts_with(&format!("{}/", guest_path.trim_end_matches('/')))
                    || guest_path.as_str() == "/"
            })
            .max_by_key(|(guest_path, _)| guest_path.len());

        let (guest_path, host_path) =
            best_match.ok_or_else(|| anyhow::anyhow!("Path not mounted: {virtual_path}"))?;

        let relative = virtual_path
            .strip_prefix(guest_path)
            .unwrap_or(virtual_path)
            .trim_start_matches('/');

        let resolved = host_path.join(relative);

        let canonical_mount = host_path.canonicalize()?;
        let canonical_resolved = if let Ok(canon) = resolved.canonicalize() {
            canon
        } else if let Some(parent) = resolved.parent() {
            parent.canonicalize()?
        } else {
            anyhow::bail!("Invalid path: {virtual_path}");
        };

        if !canonical_resolved.starts_with(&canonical_mount) {
            anyhow::bail!("Path escapes mount point: {virtual_path}");
        }

        Ok(resolved)
    }

    /// Calculate the size of a directory recursively
    fn calculate_dir_size(path: &Path) -> Result<u64> {
        let mut total = 0u64;

        if path.is_file() {
            return Ok(path.metadata()?.len());
        }

        if !path.is_dir() {
            return Ok(0);
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.is_file() {
                total += metadata.len();
            } else if metadata.is_dir() {
                total += Self::calculate_dir_size(&entry.path())?;
            }
        }

        Ok(total)
    }
}

/// Directory entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
}

/// Statistics about the WASI filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasiFilesystemStats {
    pub total_mounts: usize,
    pub total_size: u64,
    pub mounts: Vec<MountInfo>,
}

/// Information about a single mount point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub guest_path: String,
    pub host_path: PathBuf,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wasi_filesystem_creation() {
        let fs = WasiFilesystem::new();
        assert_eq!(fs.get_stats().total_mounts, 0);
    }

    #[test]
    fn test_mount_directory() {
        let fs = WasiFilesystem::new();
        let temp = tempdir().unwrap();

        fs.mount("/test", temp.path()).unwrap();
        let mounts = fs.get_stats().mounts;
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].guest_path, "/test");
    }

    #[test]
    fn test_mount_nonexistent_directory() {
        let fs = WasiFilesystem::new();
        let result = fs.mount("/test", "/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_write_file() {
        let fs = WasiFilesystem::new();
        let temp = tempdir().unwrap();
        fs.mount("/test", temp.path()).unwrap();

        // Write a file
        fs.write_file("/test/file.txt", b"Hello, WASI!").unwrap();

        // Read it back
        let content = fs.read_file("/test/file.txt").unwrap();
        assert_eq!(content, b"Hello, WASI!");
    }

    #[test]
    fn test_path_operations() {
        let fs = WasiFilesystem::new();
        let temp = tempdir().unwrap();
        fs.mount("/test", temp.path()).unwrap();

        // Create directory
        fs.path_create_directory("/test/subdir").unwrap();
        assert!(temp.path().join("subdir").is_dir());

        // Write file in subdirectory
        fs.write_file("/test/subdir/file.txt", b"content").unwrap();
        assert_eq!(fs.read_file("/test/subdir/file.txt").unwrap(), b"content");

        // List directory
        let entries = fs.path_readdir("/test").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "subdir");
        assert!(entries[0].is_dir);
    }

    #[test]
    fn test_readonly_mode() {
        let config = WasiConfig {
            read_only: true,
            ..Default::default()
        };
        let fs = WasiFilesystem::with_config(config);
        let temp = tempdir().unwrap();
        fs.mount("/test", temp.path()).unwrap();

        // Writing should fail
        let result = fs.write_file("/test/file.txt", b"data");
        assert!(result.is_err());
    }
}
