use crate::error::{Result, WasmrunError};
use std::fs;
use std::path::Path;

/// Utility for resolving and handling file paths
pub struct PathResolver;

impl PathResolver {
    pub fn resolve_input_path(positional: Option<String>, flag: Option<String>) -> String {
        positional.unwrap_or_else(|| flag.unwrap_or_else(|| String::from("./")))
    }

    pub fn has_extension(path: &str, expected_ext: &str) -> bool {
        Path::new(path).extension().map_or(false, |ext| {
            ext.to_string_lossy().to_lowercase() == expected_ext.to_lowercase()
        })
    }

    #[allow(dead_code)]
    pub fn get_extension(path: &str) -> Option<String> {
        Path::new(path)
            .extension()
            .map(|ext| ext.to_string_lossy().to_lowercase().to_string())
    }

    /// Validate that path exists and is a file
    pub fn validate_file_exists(path: &str) -> Result<()> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(WasmrunError::file_not_found(path));
        }

        if !path_obj.is_file() {
            return Err(WasmrunError::path(format!("Path is not a file: {}", path)));
        }

        Ok(())
    }

    /// Validate that path exists and is a directory
    pub fn validate_directory_exists(path: &str) -> Result<()> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(WasmrunError::directory_not_found(path));
        }

        if !path_obj.is_dir() {
            return Err(WasmrunError::path(format!(
                "Path is not a directory: {}",
                path
            )));
        }

        Ok(())
    }

    /// Validate WASM file
    pub fn validate_wasm_file(path: &str) -> Result<()> {
        Self::validate_file_exists(path)?;

        if !Self::has_extension(path, "wasm") {
            return Err(WasmrunError::invalid_file_format(
                path,
                "File does not have .wasm extension",
            ));
        }

        Ok(())
    }

    /// Get absolute path
    #[allow(dead_code)]
    pub fn get_absolute_path(path: &str) -> Result<String> {
        fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| {
                WasmrunError::add_context(format!("Getting absolute path for {}", path), e)
            })
    }

    /// Get filename from path
    pub fn get_filename(path: &str) -> Result<String> {
        Path::new(path)
            .file_name()
            .ok_or_else(|| WasmrunError::path(format!("Invalid path: {}", path)))?
            .to_string_lossy()
            .to_string()
            .pipe(Ok)
    }

    /// Get file stem
    #[allow(dead_code)]
    pub fn get_file_stem(path: &str) -> Result<String> {
        Path::new(path)
            .file_stem()
            .ok_or_else(|| WasmrunError::path(format!("Invalid path: {}", path)))?
            .to_string_lossy()
            .to_string()
            .pipe(Ok)
    }

    /// Join paths safely
    pub fn join_paths(base: &str, additional: &str) -> String {
        Path::new(base)
            .join(additional)
            .to_string_lossy()
            .to_string()
    }

    /// Create output directory
    pub fn ensure_output_directory(output_dir: &str) -> Result<()> {
        let output_path = Path::new(output_dir);
        if !output_path.exists() {
            fs::create_dir_all(output_path).map_err(|e| {
                WasmrunError::add_context(format!("Creating output directory {}", output_dir), e)
            })?;
        }
        Ok(())
    }

    pub fn find_files_with_extension(dir_path: &str, extension: &str) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let path = Path::new(dir_path);

        if !path.is_dir() {
            return Err(WasmrunError::path(format!(
                "Path is not a directory: {}",
                dir_path
            )));
        }

        let entries = fs::read_dir(path)
            .map_err(|e| WasmrunError::add_context(format!("Reading directory {}", dir_path), e))?;

        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && Self::has_extension(&entry_path.to_string_lossy(), extension)
            {
                files.push(entry_path.to_string_lossy().to_string());
            }
        }

        Ok(files)
    }

    /// Find common entry files for different languages
    #[allow(dead_code)]
    pub fn find_entry_file(project_path: &str, candidates: &[&str]) -> Option<String> {
        for candidate in candidates {
            let entry_path = Self::join_paths(project_path, candidate);
            if Path::new(&entry_path).exists() {
                return Some(entry_path);
            }
        }
        None
    }

    /// Check if a path is safe to use
    #[allow(dead_code)]
    pub fn is_safe_path(path: &str) -> bool {
        let path = Path::new(path);

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => return false,
                std::path::Component::Normal(name) => {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') && name_str.len() > 1 {
                        continue;
                    }
                }
                _ => {}
            }
        }

        true
    }

    /// Get file size in a human-readable format
    pub fn get_file_size_human(path: &str) -> Result<String> {
        let metadata = fs::metadata(path).map_err(|e| {
            WasmrunError::add_context(format!("Getting file metadata for {}", path), e)
        })?;

        let bytes = metadata.len();

        if bytes < 1024 {
            Ok(format!("{} bytes", bytes))
        } else if bytes < 1024 * 1024 {
            Ok(format!("{:.2} KB", bytes as f64 / 1024.0))
        } else if bytes < 1024 * 1024 * 1024 {
            Ok(format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0)))
        } else {
            Ok(format!(
                "{:.2} GB",
                bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            ))
        }
    }

    /// Remove file
    pub fn remove_file(path: &str) -> Result<()> {
        fs::remove_file(path)
            .map_err(|e| WasmrunError::add_context(format!("Removing file {}", path), e))?;
        Ok(())
    }

    /// Remove directory recursively
    pub fn remove_dir_all(path: &str) -> Result<()> {
        fs::remove_dir_all(path)
            .map_err(|e| WasmrunError::add_context(format!("Removing directory {}", path), e))?;
        Ok(())
    }
}

// Helper trait for method chaining
trait Pipe<T> {
    fn pipe<U, F>(self, f: F) -> U
    where
        F: FnOnce(T) -> U;
}

impl<T> Pipe<T> for T {
    fn pipe<U, F>(self, f: F) -> U
    where
        F: FnOnce(T) -> U,
    {
        f(self)
    }
}
