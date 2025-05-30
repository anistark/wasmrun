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
    pub fn validate_file_exists(path: &str) -> Result<(), String> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(format!("File not found: {}", path));
        }

        if !path_obj.is_file() {
            return Err(format!("Path is not a file: {}", path));
        }

        Ok(())
    }

    /// Validate that path exists and is a directory
    pub fn validate_directory_exists(path: &str) -> Result<(), String> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(format!("Directory not found: {}", path));
        }

        if !path_obj.is_dir() {
            return Err(format!("Path is not a directory: {}", path));
        }

        Ok(())
    }

    /// Validate WASM file (exists, is file, has .wasm extension)
    pub fn validate_wasm_file(path: &str) -> Result<(), String> {
        Self::validate_file_exists(path)?;

        if !Self::has_extension(path, "wasm") {
            return Err(format!("Not a WASM file: {}", path));
        }

        Ok(())
    }

    /// Get absolute path as string
    #[allow(dead_code)]
    pub fn get_absolute_path(path: &str) -> Result<String, String> {
        fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| format!("Failed to get absolute path for {}: {}", path, e))
    }

    /// Get filename from path
    pub fn get_filename(path: &str) -> Result<String, String> {
        Path::new(path)
            .file_name()
            .ok_or_else(|| format!("Invalid path: {}", path))?
            .to_string_lossy()
            .to_string()
            .pipe(Ok)
    }

    /// Get file stem (filename without extension)
    #[allow(dead_code)]
    pub fn get_file_stem(path: &str) -> Result<String, String> {
        Path::new(path)
            .file_stem()
            .ok_or_else(|| format!("Invalid path: {}", path))?
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
    pub fn ensure_output_directory(output_dir: &str) -> Result<(), String> {
        let output_path = Path::new(output_dir);
        if !output_path.exists() {
            fs::create_dir_all(output_path)
                .map_err(|e| format!("Failed to create output directory {}: {}", output_dir, e))?;
        }
        Ok(())
    }

    /// Find files with specific extension in directory
    pub fn find_files_with_extension(
        dir_path: &str,
        extension: &str,
    ) -> Result<Vec<String>, String> {
        let mut files = Vec::new();
        let path = Path::new(dir_path);

        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", dir_path));
        }

        let entries = fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {}: {}", dir_path, e))?;

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
}
