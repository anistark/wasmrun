//! [OS Mode] Node.js projects: detection and staging. The program itself runs
//! in the browser VM on wasmhub's nodejs runtime.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::runtime::languages::traits::{ProjectBundler, ProjectDetector};
use crate::runtime::microkernel::{Pid, WasmMicroKernel};
use crate::runtime::registry::{LanguageRuntime, ProjectBundle, ProjectMetadata};

/// Node.js runtime implementation for OS mode
pub struct NodeJSRuntime;

impl NodeJSRuntime {
    pub fn new() -> Self {
        Self
    }

    /// Parse package.json to extract project metadata and dependencies
    fn parse_package_json(&self, project_path: &str) -> Result<PackageJson> {
        let package_json_path = Path::new(project_path).join("package.json");
        if !package_json_path.exists() {
            return Ok(PackageJson::default());
        }

        let content = std::fs::read_to_string(&package_json_path)?;
        let package_json: PackageJson = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse package.json: {e}"))?;

        Ok(package_json)
    }

    /// Get the main entry point from package.json or default to index.js
    fn get_entry_point(&self, package_json: &PackageJson) -> String {
        package_json
            .main
            .clone()
            .unwrap_or_else(|| "index.js".to_string())
    }
}

impl Default for NodeJSRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Simplified package.json structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackageJson {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    main: Option<String>,
    scripts: Option<HashMap<String, String>>,
    dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<HashMap<String, String>>,
    author: Option<String>,
    license: Option<String>,
}

impl Default for PackageJson {
    fn default() -> Self {
        Self {
            name: None,
            version: Some("1.0.0".to_string()),
            description: None,
            main: Some("index.js".to_string()),
            scripts: None,
            dependencies: None,
            dev_dependencies: None,
            author: None,
            license: None,
        }
    }
}

impl ProjectDetector for NodeJSRuntime {
    fn get_entry_files(&self) -> &[&str] {
        &["package.json", "index.js", "app.js", "main.js", "server.js"]
    }
}

impl ProjectBundler for NodeJSRuntime {
    fn should_include_file(&self, file_path: &str) -> bool {
        // Include JavaScript files, JSON files, and important config files
        let path = Path::new(file_path);
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return matches!(ext_str, "js" | "mjs" | "json" | "ts" | "md");
            }
        }

        // Include important files without extensions or with specific names
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        matches!(
            file_name,
            "package.json" | "package-lock.json" | "tsconfig.json" | "README.md" | ".env"
        )
    }

    fn extract_dependencies(&self, project_path: &str) -> Result<Vec<String>> {
        let package_json = self.parse_package_json(project_path)?;
        let mut deps = vec![];

        if let Some(dependencies) = package_json.dependencies {
            deps.extend(dependencies.keys().cloned());
        }

        if let Some(dev_dependencies) = package_json.dev_dependencies {
            deps.extend(dev_dependencies.keys().cloned());
        }

        Ok(deps)
    }
}

impl LanguageRuntime for NodeJSRuntime {
    fn detect_project(&self, project_path: &str) -> bool {
        self.has_entry_files(project_path)
    }

    fn prepare_project(&self, project_path: &str) -> Result<ProjectBundle> {
        let package_json = self.parse_package_json(project_path)?;
        let files = self.read_project_files(project_path)?;
        let dependencies = self.extract_dependencies(project_path)?;
        let entry_point = self.get_entry_point(&package_json);

        let metadata = ProjectMetadata {
            version: package_json.version.unwrap_or_else(|| "1.0.0".to_string()),
            description: package_json.description,
            author: package_json.author,
            license: package_json.license,
        };

        Ok(ProjectBundle {
            name: package_json
                .name
                .unwrap_or_else(|| "nodejs-project".to_string()),
            language: "nodejs".to_string(),
            entry_point,
            files,
            dependencies,
            metadata,
        })
    }

    fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid> {
        let pid = kernel.create_process(bundle.name.clone(), "nodejs".to_string(), None)?;
        let staged: usize = bundle.files.values().map(Vec::len).sum();
        kernel.mark_running(pid, staged);

        let workspace = kernel.ensure_process_workspace(pid)?;

        for (path, content) in &bundle.files {
            let vfs_path = format!("{workspace}/{path}");
            kernel.wasi_filesystem().write_file(&vfs_path, content)?;
        }

        let deps_obj: HashMap<String, String> = bundle
            .dependencies
            .iter()
            .map(|dep| (dep.clone(), "*".to_string()))
            .collect();

        let package_json = serde_json::to_vec_pretty(&serde_json::json!({
            "name": bundle.name,
            "version": bundle.metadata.version,
            "description": bundle.metadata.description,
            "main": bundle.entry_point,
            "dependencies": deps_obj,
            "author": bundle.metadata.author,
            "license": bundle.metadata.license
        }))?;

        let package_path = format!("{workspace}/package.json");
        kernel
            .wasi_filesystem()
            .write_file(&package_path, &package_json)?;

        Ok(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nodejs_entry_files() {
        let runtime = NodeJSRuntime::new();
        let entry_files = runtime.get_entry_files();
        assert!(entry_files.contains(&"package.json"));
        assert!(entry_files.contains(&"index.js"));
    }

    #[test]
    fn test_a_project_with_an_entry_file_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = NodeJSRuntime::new();
        assert!(!runtime.detect_project(&dir.path().to_string_lossy()));
        std::fs::write(dir.path().join("server.js"), "").unwrap();
        assert!(runtime.detect_project(&dir.path().to_string_lossy()));
    }
}
