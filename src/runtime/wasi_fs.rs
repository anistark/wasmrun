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

/// WASI file descriptor type
pub type WasiFd = u32;

/// WASI filesystem manager for OS mode
pub struct WasiFilesystem {
    /// Mounted directories (virtual path -> host path)
    mounts: Arc<RwLock<HashMap<String, PathBuf>>>,
    /// File descriptor table (fd -> open file info)
    fd_table: Arc<RwLock<HashMap<WasiFd, OpenFile>>>,
    /// Next available file descriptor
    #[allow(dead_code)]
    next_fd: Arc<RwLock<WasiFd>>,
    /// Configuration
    config: WasiConfig,
}

/// WASI filesystem configuration
#[derive(Debug, Clone)]
pub struct WasiConfig {
    /// Allow access to host filesystem
    #[allow(dead_code)]
    pub allow_host_access: bool,
    /// Read-only mode
    pub read_only: bool,
    /// Maximum file size for operations (in bytes)
    pub max_file_size: usize,
}

impl Default for WasiConfig {
    fn default() -> Self {
        Self {
            allow_host_access: true,
            read_only: false,
            max_file_size: 100 * 1024 * 1024, // 100 MB
        }
    }
}

/// Information about an open file
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OpenFile {
    /// Resolved host path
    path: PathBuf,
    /// File open flags
    flags: OpenFlags,
    /// Current offset in the file
    offset: usize,
    /// Virtual path (as seen by WASI)
    virtual_path: String,
}

/// File open flags (WASI-compatible)
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub append: bool,
    pub create: bool,
    pub truncate: bool,
}

impl Default for OpenFlags {
    fn default() -> Self {
        Self {
            read: true,
            write: false,
            append: false,
            create: false,
            truncate: false,
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
            fd_table: Arc::new(RwLock::new(HashMap::new())),
            next_fd: Arc::new(RwLock::new(3)), // 0, 1, 2 reserved for stdin, stdout, stderr
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
            anyhow::bail!(
                "Cannot mount non-directory path: {}",
                host_path.display()
            );
        }

        let canonical = host_path.canonicalize()?;
        let mut mounts = self.mounts.write().unwrap();
        mounts.insert(guest_path.to_string(), canonical);

        Ok(())
    }

    /// Unmount a virtual path
    #[allow(dead_code)]
    pub fn unmount(&self, guest_path: &str) -> Option<PathBuf> {
        let mut mounts = self.mounts.write().unwrap();
        mounts.remove(guest_path)
    }

    /// List all current mounts
    #[allow(dead_code)]
    pub fn list_mounts(&self) -> Vec<(String, PathBuf)> {
        let mounts = self.mounts.read().unwrap();
        mounts
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// WASI path_open - Open or create a file
    #[allow(dead_code)]
    pub fn path_open(
        &self,
        virtual_path: &str,
        flags: OpenFlags,
    ) -> Result<WasiFd> {
        if self.config.read_only && (flags.write || flags.create) {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        let host_path = self.resolve_path(virtual_path)?;

        // Create the file if needed
        if flags.create && !host_path.exists() {
            if let Some(parent) = host_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::File::create(&host_path)?;
        }

        // Verify the file exists
        if !host_path.exists() {
            anyhow::bail!("File not found: {}", virtual_path);
        }

        // Truncate if requested
        if flags.truncate && host_path.is_file() {
            fs::File::create(&host_path)?;
        }

        // Allocate a file descriptor
        let fd = {
            let mut next_fd = self.next_fd.write().unwrap();
            let fd = *next_fd;
            *next_fd += 1;
            fd
        };

        // Store the open file information
        let mut fd_table = self.fd_table.write().unwrap();
        fd_table.insert(
            fd,
            OpenFile {
                path: host_path,
                flags,
                offset: if flags.append { 0 } else { 0 }, // Will be set to end in first write
                virtual_path: virtual_path.to_string(),
            },
        );

        Ok(fd)
    }

    /// WASI fd_read - Read from a file descriptor
    #[allow(dead_code)]
    pub fn fd_read(&self, fd: WasiFd, count: usize) -> Result<Vec<u8>> {
        let mut fd_table = self.fd_table.write().unwrap();
        let open_file = fd_table
            .get_mut(&fd)
            .ok_or_else(|| anyhow::anyhow!("Invalid file descriptor: {}", fd))?;

        if !open_file.flags.read {
            anyhow::bail!("File not open for reading");
        }

        let data = fs::read(&open_file.path)?;
        let start = open_file.offset.min(data.len());
        let end = (start + count).min(data.len());
        let result = data[start..end].to_vec();

        open_file.offset = end;
        Ok(result)
    }

    /// WASI fd_write - Write to a file descriptor
    #[allow(dead_code)]
    pub fn fd_write(&self, fd: WasiFd, data: &[u8]) -> Result<usize> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        if data.len() > self.config.max_file_size {
            anyhow::bail!("File size exceeds maximum allowed size");
        }

        let mut fd_table = self.fd_table.write().unwrap();
        let open_file = fd_table
            .get_mut(&fd)
            .ok_or_else(|| anyhow::anyhow!("Invalid file descriptor: {}", fd))?;

        if !open_file.flags.write {
            anyhow::bail!("File not open for writing");
        }

        if open_file.flags.append {
            // Append mode: write to end of file
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&open_file.path)?;
            std::io::Write::write_all(&mut file, data)?;
            open_file.offset += data.len();
        } else {
            // Write mode: write at current offset
            let mut content = if open_file.path.exists() {
                fs::read(&open_file.path)?
            } else {
                Vec::new()
            };

            // Expand content if needed
            if open_file.offset + data.len() > content.len() {
                content.resize(open_file.offset + data.len(), 0);
            }

            // Write data at offset
            content[open_file.offset..open_file.offset + data.len()].copy_from_slice(data);
            fs::write(&open_file.path, &content)?;
            open_file.offset += data.len();
        }

        Ok(data.len())
    }

    /// WASI fd_close - Close a file descriptor
    #[allow(dead_code)]
    pub fn fd_close(&self, fd: WasiFd) -> Result<()> {
        let mut fd_table = self.fd_table.write().unwrap();
        fd_table
            .remove(&fd)
            .ok_or_else(|| anyhow::anyhow!("Invalid file descriptor: {}", fd))?;
        Ok(())
    }

    /// WASI fd_seek - Seek to a position in a file
    #[allow(dead_code)]
    pub fn fd_seek(&self, fd: WasiFd, offset: i64, whence: SeekWhence) -> Result<usize> {
        let mut fd_table = self.fd_table.write().unwrap();
        let open_file = fd_table
            .get_mut(&fd)
            .ok_or_else(|| anyhow::anyhow!("Invalid file descriptor: {}", fd))?;

        let file_size = fs::metadata(&open_file.path)?.len() as usize;

        let new_offset = match whence {
            SeekWhence::Start => offset.max(0) as usize,
            SeekWhence::Current => {
                let current = open_file.offset as i64;
                (current + offset).max(0) as usize
            }
            SeekWhence::End => {
                let end = file_size as i64;
                (end + offset).max(0) as usize
            }
        };

        open_file.offset = new_offset;
        Ok(new_offset)
    }

    /// WASI path_create_directory - Create a directory
    pub fn path_create_directory(&self, virtual_path: &str) -> Result<()> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        let host_path = self.resolve_path(virtual_path)?;
        fs::create_dir_all(&host_path)
            .with_context(|| format!("Failed to create directory: {}", virtual_path))?;
        Ok(())
    }

    /// WASI path_remove_directory - Remove a directory
    #[allow(dead_code)]
    pub fn path_remove_directory(&self, virtual_path: &str) -> Result<()> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        let host_path = self.resolve_path(virtual_path)?;
        fs::remove_dir(&host_path)
            .with_context(|| format!("Failed to remove directory: {}", virtual_path))?;
        Ok(())
    }

    /// WASI path_unlink_file - Delete a file
    pub fn path_unlink_file(&self, virtual_path: &str) -> Result<()> {
        if self.config.read_only {
            anyhow::bail!("Filesystem is in read-only mode");
        }

        let host_path = self.resolve_path(virtual_path)?;
        fs::remove_file(&host_path)
            .with_context(|| format!("Failed to unlink file: {}", virtual_path))?;
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

    /// WASI path_filestat_get - Get file/directory metadata
    #[allow(dead_code)]
    pub fn path_filestat_get(&self, virtual_path: &str) -> Result<FileStats> {
        let host_path = self.resolve_path(virtual_path)?;
        let metadata = fs::metadata(&host_path)?;

        Ok(FileStats {
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            accessed: metadata.accessed().ok(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
        })
    }

    /// Check if a path exists
    #[allow(dead_code)]
    pub fn path_exists(&self, virtual_path: &str) -> bool {
        self.resolve_path(virtual_path)
            .map(|p| p.exists())
            .unwrap_or(false)
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

        let fd_table = self.fd_table.read().unwrap();
        let open_fds = fd_table.len();

        WasiFilesystemStats {
            total_mounts,
            total_size,
            open_fds,
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

    /// Resolve a virtual WASI path to a real host path
    fn resolve_path(&self, virtual_path: &str) -> Result<PathBuf> {
        let mounts = self.mounts.read().unwrap();

        // Find the matching mount point
        for (guest_path, host_path) in mounts.iter() {
            if virtual_path.starts_with(guest_path) {
                let relative = virtual_path
                    .strip_prefix(guest_path)
                    .unwrap_or(virtual_path)
                    .trim_start_matches('/');

                let resolved = host_path.join(relative);

                // Security check: ensure resolved path is within the mount
                let canonical_mount = host_path.canonicalize()?;
                let canonical_resolved = if let Ok(canon) = resolved.canonicalize() {
                    canon
                } else if let Some(parent) = resolved.parent() {
                    // If file doesn't exist yet, check parent
                    parent.canonicalize()?
                } else {
                    anyhow::bail!("Invalid path: {}", virtual_path);
                };

                if !canonical_resolved.starts_with(&canonical_mount) {
                    anyhow::bail!("Path escapes mount point: {}", virtual_path);
                }

                return Ok(resolved);
            }
        }

        anyhow::bail!("Path not mounted: {}", virtual_path)
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

/// Seek position reference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SeekWhence {
    Start,
    Current,
    End,
}

/// Directory entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
}

/// File statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub is_file: bool,
    pub is_dir: bool,
    pub size: u64,
    pub accessed: Option<std::time::SystemTime>,
    pub modified: Option<std::time::SystemTime>,
    pub created: Option<std::time::SystemTime>,
}

/// Statistics about the WASI filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasiFilesystemStats {
    pub total_mounts: usize,
    pub total_size: u64,
    pub open_fds: usize,
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
        assert_eq!(fs.list_mounts().len(), 0);
    }

    #[test]
    fn test_mount_directory() {
        let fs = WasiFilesystem::new();
        let temp = tempdir().unwrap();

        fs.mount("/test", temp.path()).unwrap();
        let mounts = fs.list_mounts();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].0, "/test");
    }

    #[test]
    fn test_mount_nonexistent_directory() {
        let fs = WasiFilesystem::new();
        let result = fs.mount("/test", "/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_unmount() {
        let fs = WasiFilesystem::new();
        let temp = tempdir().unwrap();

        fs.mount("/test", temp.path()).unwrap();
        assert_eq!(fs.list_mounts().len(), 1);

        let unmounted = fs.unmount("/test");
        assert!(unmounted.is_some());
        assert_eq!(fs.list_mounts().len(), 0);
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
        assert!(fs.path_exists("/test/subdir"));

        // Write file in subdirectory
        fs.write_file("/test/subdir/file.txt", b"content")
            .unwrap();

        // Check file stats
        let stats = fs.path_filestat_get("/test/subdir/file.txt").unwrap();
        assert!(stats.is_file);
        assert_eq!(stats.size, 7);

        // List directory
        let entries = fs.path_readdir("/test").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "subdir");
        assert!(entries[0].is_dir);
    }

    #[test]
    fn test_fd_operations() {
        let fs = WasiFilesystem::new();
        let temp = tempdir().unwrap();
        fs.mount("/test", temp.path()).unwrap();

        // Create a file
        fs.write_file("/test/test.txt", b"Hello, World!").unwrap();

        // Open for reading
        let fd = fs
            .path_open("/test/test.txt", OpenFlags { read: true, ..Default::default() })
            .unwrap();

        // Read data
        let data = fs.fd_read(fd, 5).unwrap();
        assert_eq!(data, b"Hello");

        // Read more
        let data = fs.fd_read(fd, 7).unwrap();
        assert_eq!(data, b", World");

        // Close
        fs.fd_close(fd).unwrap();
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
