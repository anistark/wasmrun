use anyhow::Result;
/// Common traits and utilities for language runtime implementations
use std::path::Path;

/// Helper functions for project detection
#[allow(dead_code)]
pub trait ProjectDetector {
    /// Check if a directory contains files indicating this language project
    fn has_entry_files(&self, project_path: &str) -> bool {
        let path = Path::new(project_path);
        if !path.exists() || !path.is_dir() {
            return false;
        }

        for entry_file in self.get_entry_files() {
            let entry_path = path.join(entry_file);
            if entry_path.exists() {
                return true;
            }
        }
        false
    }

    /// Get the list of entry files that indicate a project of this type
    fn get_entry_files(&self) -> &[&str];

    /// Check if a file has a supported extension
    fn has_supported_extension(&self, file_path: &str) -> bool {
        let path = Path::new(file_path);
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return self.get_supported_extensions().contains(&ext_str);
            }
        }
        false
    }

    /// Get the list of supported file extensions
    fn get_supported_extensions(&self) -> &[&str];
}

/// Helper functions for project bundling
pub trait ProjectBundler {
    /// Recursively read all project files
    fn read_project_files(
        &self,
        project_path: &str,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>> {
        use std::collections::HashMap;

        let mut files = HashMap::new();
        let path = Path::new(project_path);

        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Project path does not exist: {}",
                project_path
            ));
        }

        self.read_directory_recursive(path, &mut files, "")?;
        Ok(files)
    }

    /// Recursively read directory contents
    fn read_directory_recursive(
        &self,
        dir: &Path,
        files: &mut std::collections::HashMap<String, Vec<u8>>,
        prefix: &str,
    ) -> Result<()> {
        use std::fs;

        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => continue,
            };

            let relative_path = if prefix.is_empty() {
                file_name.to_string()
            } else {
                format!("{prefix}/{file_name}")
            };

            // Skip hidden files and directories
            if file_name.starts_with('.') {
                continue;
            }

            // Skip common build/cache directories
            if self.should_skip_directory(&file_name) {
                continue;
            }

            if path.is_file() {
                // Only include files with supported extensions or important files
                if self.should_include_file(&relative_path) {
                    let content = fs::read(&path)?;
                    files.insert(relative_path, content);
                }
            } else if path.is_dir() {
                self.read_directory_recursive(&path, files, &relative_path)?;
            }
        }

        Ok(())
    }

    /// Check if a directory should be skipped during bundling
    fn should_skip_directory(&self, dir_name: &str) -> bool {
        matches!(
            dir_name,
            "node_modules"
                | "target"
                | "build"
                | "dist"
                | "__pycache__"
                | ".git"
                | ".vscode"
                | ".idea"
                | "vendor"
                | "bin"
                | "obj"
        )
    }

    /// Check if a file should be included in the bundle
    fn should_include_file(&self, file_path: &str) -> bool;

    /// Get language-specific dependencies from the project
    fn extract_dependencies(&self, project_path: &str) -> Result<Vec<String>>;
}

/// Default implementations for common project operations
pub struct DefaultProjectOps;

impl ProjectDetector for DefaultProjectOps {
    fn get_entry_files(&self) -> &[&str] {
        &[]
    }

    fn get_supported_extensions(&self) -> &[&str] {
        &[]
    }
}

impl ProjectBundler for DefaultProjectOps {
    fn should_include_file(&self, _file_path: &str) -> bool {
        true
    }

    fn extract_dependencies(&self, _project_path: &str) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
