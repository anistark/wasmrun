use crate::error::{ChakraError, Result};
use std::fs;
use std::path::Path;

/// Utility for resolving and handling file paths consistently across the application
pub struct PathResolver;

impl PathResolver {
    /// Resolve input path from positional argument or flag, with fallback to current directory
    pub fn resolve_input_path(positional: Option<String>, flag: Option<String>) -> String {
        positional.unwrap_or_else(|| flag.unwrap_or_else(|| String::from("./")))
    }

    /// Resolve input path with custom default
    #[allow(dead_code)]
    pub fn resolve_input_path_with_default(
        positional: Option<String>,
        flag: Option<String>,
        default: &str,
    ) -> String {
        positional.unwrap_or_else(|| flag.unwrap_or_else(|| default.to_string()))
    }

    /// Check if file has the expected extension
    pub fn has_extension(path: &str, expected_ext: &str) -> bool {
        Path::new(path).extension().map_or(false, |ext| {
            ext.to_string_lossy().to_lowercase() == expected_ext.to_lowercase()
        })
    }

    /// Get file extension as lowercase string
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
            return Err(ChakraError::file_not_found(path));
        }

        if !path_obj.is_file() {
            return Err(ChakraError::path(format!("Path is not a file: {}", path)));
        }

        Ok(())
    }

    /// Validate that path exists and is a directory
    pub fn validate_directory_exists(path: &str) -> Result<()> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(ChakraError::directory_not_found(path));
        }

        if !path_obj.is_dir() {
            return Err(ChakraError::path(format!(
                "Path is not a directory: {}",
                path
            )));
        }

        Ok(())
    }

    /// Validate WASM file (exists, is file, has .wasm extension)
    pub fn validate_wasm_file(path: &str) -> Result<()> {
        Self::validate_file_exists(path)?;

        if !Self::has_extension(path, "wasm") {
            return Err(ChakraError::invalid_file_format(
                path,
                "File does not have .wasm extension",
            ));
        }

        Ok(())
    }

    /// Get absolute path as string
    #[allow(dead_code)]
    pub fn get_absolute_path(path: &str) -> Result<String> {
        fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| ChakraError::add_context(format!("Getting absolute path for {}", path), e))
    }

    /// Get filename from path
    pub fn get_filename(path: &str) -> Result<String> {
        Path::new(path)
            .file_name()
            .ok_or_else(|| ChakraError::path(format!("Invalid path: {}", path)))?
            .to_string_lossy()
            .to_string()
            .pipe(Ok)
    }

    /// Get file stem (filename without extension)
    #[allow(dead_code)]
    pub fn get_file_stem(path: &str) -> Result<String> {
        Path::new(path)
            .file_stem()
            .ok_or_else(|| ChakraError::path(format!("Invalid path: {}", path)))?
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

    /// Create output directory if it doesn't exist
    pub fn ensure_output_directory(output_dir: &str) -> Result<()> {
        let output_path = Path::new(output_dir);
        if !output_path.exists() {
            fs::create_dir_all(output_path).map_err(|e| {
                ChakraError::add_context(format!("Creating output directory {}", output_dir), e)
            })?;
        }
        Ok(())
    }

    /// Find files with specific extension in directory
    pub fn find_files_with_extension(dir_path: &str, extension: &str) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let path = Path::new(dir_path);

        if !path.is_dir() {
            return Err(ChakraError::path(format!(
                "Path is not a directory: {}",
                dir_path
            )));
        }

        let entries = fs::read_dir(path)
            .map_err(|e| ChakraError::add_context(format!("Reading directory {}", dir_path), e))?;

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
    pub fn find_entry_file(project_path: &str, candidates: &[&str]) -> Option<String> {
        for candidate in candidates {
            let entry_path = Self::join_paths(project_path, candidate);
            if Path::new(&entry_path).exists() {
                return Some(entry_path);
            }
        }
        None
    }

    /// Check if a path is safe to use (no directory traversal)
    pub fn is_safe_path(path: &str) -> bool {
        let path = Path::new(path);

        // Check for directory traversal attempts
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => return false,
                std::path::Component::Normal(name) => {
                    let name_str = name.to_string_lossy();
                    // Check for potentially dangerous names
                    if name_str.starts_with('.') && name_str.len() > 1 {
                        continue; // Allow normal hidden files
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
            ChakraError::add_context(format!("Getting file metadata for {}", path), e)
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

    /// Remove file with error handling
    pub fn remove_file(path: &str) -> Result<()> {
        fs::remove_file(path)
            .map_err(|e| ChakraError::add_context(format!("Removing file {}", path), e))?;
        Ok(())
    }

    /// Remove directory recursively with error handling
    pub fn remove_dir_all(path: &str) -> Result<()> {
        fs::remove_dir_all(path)
            .map_err(|e| ChakraError::add_context(format!("Removing directory {}", path), e))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_input_path() {
        // Test with positional argument
        assert_eq!(
            PathResolver::resolve_input_path(Some("test.wasm".to_string()), None),
            "test.wasm"
        );

        // Test with flag argument
        assert_eq!(
            PathResolver::resolve_input_path(None, Some("test.wasm".to_string())),
            "test.wasm"
        );

        // Test with default
        assert_eq!(PathResolver::resolve_input_path(None, None), "./");

        // Test positional takes precedence
        assert_eq!(
            PathResolver::resolve_input_path(
                Some("positional.wasm".to_string()),
                Some("flag.wasm".to_string())
            ),
            "positional.wasm"
        );
    }

    #[test]
    fn test_has_extension() {
        assert!(PathResolver::has_extension("test.wasm", "wasm"));
        assert!(PathResolver::has_extension("test.WASM", "wasm")); // Case insensitive
        assert!(!PathResolver::has_extension("test.js", "wasm"));
        assert!(!PathResolver::has_extension("test", "wasm"));
    }

    #[test]
    fn test_get_filename() {
        assert_eq!(
            PathResolver::get_filename("/path/to/test.wasm").unwrap(),
            "test.wasm"
        );
        assert_eq!(
            PathResolver::get_filename("test.wasm").unwrap(),
            "test.wasm"
        );
    }

    #[test]
    fn test_get_file_stem() {
        assert_eq!(
            PathResolver::get_file_stem("/path/to/test.wasm").unwrap(),
            "test"
        );
        assert_eq!(PathResolver::get_file_stem("test.wasm").unwrap(), "test");
    }

    #[test]
    fn test_find_entry_file() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create test files
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("lib.rs"), "// lib").unwrap();

        let candidates = &["main.rs", "src/main.rs", "lib.rs"];
        let found = PathResolver::find_entry_file(project_path, candidates);

        assert!(found.is_some());
        assert!(found.unwrap().ends_with("main.rs"));
    }

    #[test]
    fn test_ensure_output_directory() {
        let temp_dir = tempdir().unwrap();
        let new_dir = temp_dir.path().join("output");
        let new_dir_str = new_dir.to_str().unwrap();

        assert!(!new_dir.exists());

        PathResolver::ensure_output_directory(new_dir_str).unwrap();

        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
    }

    #[test]
    fn test_validate_file_exists() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // File doesn't exist
        let result = PathResolver::validate_file_exists(file_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ChakraError::FileNotFound { .. }
        ));

        // Create file and test again
        fs::write(&file_path, "test").unwrap();
        let result = PathResolver::validate_file_exists(file_path.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_wasm_file() {
        let temp_dir = tempdir().unwrap();
        let wasm_file = temp_dir.path().join("test.wasm");
        let js_file = temp_dir.path().join("test.js");

        // Create files
        fs::write(&wasm_file, b"fake wasm content").unwrap();
        fs::write(&js_file, "console.log('test')").unwrap();

        // Test valid WASM file
        let result = PathResolver::validate_wasm_file(wasm_file.to_str().unwrap());
        assert!(result.is_ok());

        // Test invalid extension
        let result = PathResolver::validate_wasm_file(js_file.to_str().unwrap());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ChakraError::InvalidFileFormat { .. }
        ));
    }

    #[test]
    fn test_is_safe_path() {
        assert!(PathResolver::is_safe_path("normal/path.txt"));
        assert!(PathResolver::is_safe_path("./current/dir.txt"));
        assert!(!PathResolver::is_safe_path("../parent/dir.txt"));
        assert!(!PathResolver::is_safe_path("../../dangerous.txt"));
        assert!(PathResolver::is_safe_path(".hidden/file.txt"));
    }

    #[test]
    fn test_file_operations() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        // Test file size
        fs::write(&file_path, b"Hello, world!").unwrap();
        let size = PathResolver::get_file_size_human(file_path_str).unwrap();
        assert!(size.contains("bytes"));

        // Test remove
        PathResolver::remove_file(file_path_str).unwrap();
        assert!(!file_path.exists());
    }
}
