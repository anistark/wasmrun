use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::runtime::languages::traits::{DefaultProjectOps, ProjectBundler, ProjectDetector};
use crate::runtime::microkernel::{Pid, SyscallInterface, WasmMicroKernel};
use crate::runtime::registry::{
    DevServer, DevServerStatus, LanguageRuntime, ProjectBundle, ProjectMetadata,
};
use crate::runtime::syscalls::{SyscallArgs, SyscallResult};

/// Node.js runtime implementation for OS mode
#[allow(dead_code)]
pub struct NodeJSRuntime {
    detector: DefaultProjectOps,
}

impl NodeJSRuntime {
    pub fn new() -> Self {
        Self {
            detector: DefaultProjectOps,
        }
    }

    /// Parse package.json to extract project metadata and dependencies
    fn parse_package_json(&self, project_path: &str) -> Result<PackageJson> {
        let package_json_path = Path::new(project_path).join("package.json");
        if !package_json_path.exists() {
            return Ok(PackageJson::default());
        }

        let content = std::fs::read_to_string(&package_json_path)?;
        let package_json: PackageJson = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse package.json: {}", e))?;

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

    fn get_supported_extensions(&self) -> &[&str] {
        &["js", "mjs", "json", "ts"]
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
    fn name(&self) -> &str {
        "nodejs"
    }

    fn extensions(&self) -> &[&str] {
        self.get_supported_extensions()
    }

    fn entry_files(&self) -> &[&str] {
        self.get_entry_files()
    }

    fn load_wasm_binary(&self) -> Result<Vec<u8>> {
        // TODO: In a real implementation, this would load a Node.js + V8 WASM binary
        // For now, return a placeholder WASM module
        Ok(create_placeholder_nodejs_wasm())
    }

    fn create_syscall_interface(&self) -> Box<dyn SyscallInterface> {
        Box::new(NodeJSSyscallInterface::new())
    }

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn supports_debugging(&self) -> bool {
        true
    }

    fn create_dev_server(&self) -> Option<Box<dyn DevServer>> {
        Some(Box::new(NodeJSDevServer::new()))
    }

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
        // Create a process for the Node.js project
        let pid = kernel.create_process(bundle.name.clone(), "nodejs".to_string(), None)?;

        // Load the Node.js runtime WASM
        let wasm_binary = self.load_wasm_binary()?;
        kernel.load_wasm_module(pid, &wasm_binary)?;

        // Write project files to the virtual filesystem
        for (path, content) in bundle.files {
            let vfs_path = format!("/projects/{pid}/{path}");
            kernel.write_file(&vfs_path, &content)?;
        }

        // Write package.json with metadata
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

        let package_path = format!("/projects/{pid}/package.json");
        kernel.write_file(&package_path, &package_json)?;

        Ok(pid)
    }

    fn handle_syscall(&self, _pid: Pid, _syscall_num: u32, _args: SyscallArgs) -> SyscallResult {
        // TODO: Implement Node.js-specific syscalls (e.g., require, setTimeout, process.env)
        SyscallResult::Error("Node.js-specific syscalls not yet implemented".to_string())
    }
}

/// Node.js-specific syscall interface
struct NodeJSSyscallInterface {
    // TODO: Add Node.js-specific syscall implementations
}

impl NodeJSSyscallInterface {
    fn new() -> Self {
        Self {}
    }
}

impl SyscallInterface for NodeJSSyscallInterface {
    fn read_file(&self, _path: &str) -> Result<Vec<u8>> {
        // TODO: Implement Node.js-specific file reading with require() support
        Err(anyhow::anyhow!("Node.js file reading not implemented"))
    }

    fn write_file(&self, _path: &str, _data: &[u8]) -> Result<()> {
        // TODO: Implement Node.js-specific file writing
        Err(anyhow::anyhow!("Node.js file writing not implemented"))
    }

    fn list_directory(&self, _path: &str) -> Result<Vec<crate::runtime::microkernel::VfsEntry>> {
        // TODO: Implement Node.js-specific directory listing
        Err(anyhow::anyhow!("Node.js directory listing not implemented"))
    }

    fn create_directory(&self, _path: &str) -> Result<()> {
        // TODO: Implement Node.js-specific directory creation
        Err(anyhow::anyhow!(
            "Node.js directory creation not implemented"
        ))
    }

    fn delete_file(&self, _path: &str) -> Result<()> {
        // TODO: Implement Node.js-specific file deletion
        Err(anyhow::anyhow!("Node.js file deletion not implemented"))
    }
}

/// Node.js development server
struct NodeJSDevServer {
    status: DevServerStatus,
}

impl NodeJSDevServer {
    fn new() -> Self {
        Self {
            status: DevServerStatus::Stopped,
        }
    }
}

impl DevServer for NodeJSDevServer {
    fn start(&self, port: u16) -> Result<()> {
        // TODO: Implement Node.js development server startup
        // This would involve setting up hot reload, watch mode, etc.
        println!("Starting Node.js dev server on port {port}");
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        // TODO: Implement Node.js development server shutdown
        println!("Stopping Node.js dev server");
        Ok(())
    }

    fn reload(&self) -> Result<()> {
        // TODO: Implement hot reload functionality
        println!("Reloading Node.js project");
        Ok(())
    }

    fn get_status(&self) -> DevServerStatus {
        self.status.clone()
    }
}

/// Create a placeholder WASM binary for Node.js runtime
/// In a real implementation, this would be a pre-compiled Node.js + V8 WASM binary
fn create_placeholder_nodejs_wasm() -> Vec<u8> {
    // WASM magic number + version
    let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

    // Minimal WASM module structure
    // Type section
    wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);

    // Function section
    wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);

    // Export section
    wasm.extend_from_slice(&[
        0x07, 0x09, 0x01, 0x05, b'_', b's', b't', b'a', b'r', b't', 0x00, 0x00,
    ]);

    // Code section
    wasm.extend_from_slice(&[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b]);

    wasm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nodejs_runtime_creation() {
        let runtime = NodeJSRuntime::new();
        assert_eq!(runtime.name(), "nodejs");
        assert!(runtime.supports_hot_reload());
        assert!(runtime.supports_debugging());
    }

    #[test]
    fn test_nodejs_extensions() {
        let runtime = NodeJSRuntime::new();
        let extensions = runtime.extensions();
        assert!(extensions.contains(&"js"));
        assert!(extensions.contains(&"json"));
        assert!(extensions.contains(&"ts"));
    }

    #[test]
    fn test_nodejs_entry_files() {
        let runtime = NodeJSRuntime::new();
        let entry_files = runtime.entry_files();
        assert!(entry_files.contains(&"package.json"));
        assert!(entry_files.contains(&"index.js"));
    }

    #[test]
    fn test_placeholder_wasm_generation() {
        let wasm = create_placeholder_nodejs_wasm();
        assert!(wasm.len() > 8);
        // Check WASM magic number
        assert_eq!(&wasm[0..4], &[0x00, 0x61, 0x73, 0x6d]);
        // Check WASM version
        assert_eq!(&wasm[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }
}
