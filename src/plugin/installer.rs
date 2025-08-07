use crate::error::{Result, WasmrunError};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PluginInstaller;

impl PluginInstaller {
    pub fn install_external_plugin(plugin_name: &str) -> Result<InstallationResult> {
        let mut result = InstallationResult::new(plugin_name);

        // Validate plugin name
        if !Self::is_supported_plugin(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Unsupported plugin: {}. Supported: wasmrust, wasmgo",
                plugin_name
            )));
        }

        // Check if cargo is available
        if !Self::is_cargo_available() {
            return Err(WasmrunError::from(
                "cargo is required for plugin installation but was not found"
            ));
        }

        // Get plugin directory
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;

        // Check if already installed by looking for the plugin files in the directory
        if Self::is_plugin_library_installed(plugin_name) {
            result.binary_already_installed = true;
            result.version = Self::detect_plugin_version_from_metadata(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());
            println!("Plugin '{}' library files already exist", plugin_name);
        } else {
            // Install the plugin library files
            Self::install_plugin_library(plugin_name, &plugin_dir)?;
            result.binary_installed = true;
            result.version = Self::detect_plugin_version_from_metadata(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());
            println!("Plugin '{}' library files installed successfully", plugin_name);
        }

        Ok(result)
    }

    pub fn setup_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;

        // Create plugin directory
        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {}", e)))?;

        // Create manifest file
        Self::create_plugin_manifest(plugin_name, &plugin_dir)?;

        // Create metadata file
        Self::create_metadata_file(plugin_name, &plugin_dir)?;

        Ok(plugin_dir)
    }

    pub fn remove_plugin_directory(plugin_name: &str) -> Result<()> {
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir)
                .map_err(|e| WasmrunError::from(format!("Failed to remove plugin directory: {}", e)))?;
        }
        Ok(())
    }

    fn is_supported_plugin(plugin_name: &str) -> bool {
        matches!(plugin_name, "wasmrust" | "wasmgo")
    }

    fn is_cargo_available() -> bool {
        Command::new("cargo")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn is_plugin_library_installed(plugin_name: &str) -> bool {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            // Check for Cargo.toml in the plugin directory
            let cargo_toml = plugin_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                return true;
            }

            // Check for manifest file
            let manifest_path = plugin_dir.join("wasmrun.toml");
            if manifest_path.exists() {
                return true;
            }

            // Check for shared library files
            let lib_extensions = ["so", "dylib", "dll"];
            for ext in &lib_extensions {
                let lib_path = plugin_dir.join(format!("lib{}.{}", plugin_name, ext));
                if lib_path.exists() {
                    return true;
                }
            }
        }

        false
    }

    fn install_plugin_library(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        println!("Setting up {} plugin library files...", plugin_name);

        // Create plugin directory if it doesn't exist
        std::fs::create_dir_all(plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {}", e)))?;

        // For library-based plugins, we need to:
        // 1. Set up the plugin source code and metadata files
        // 2. Copy the necessary files to the plugin directory  
        // 3. Optionally build the shared library for dynamic loading

        // Setup the plugin source and metadata
        Self::setup_plugin_source(plugin_name, plugin_dir)?;

        // Optionally build the plugin as a shared library for dynamic loading
        if Self::should_build_shared_library(plugin_name) {
            Self::build_plugin_library(plugin_name, plugin_dir)?;
        }

        println!("Plugin '{}' library files set up successfully", plugin_name);
        Ok(())
    }

    fn should_build_shared_library(_plugin_name: &str) -> bool {
        // For now, we'll skip building shared libraries and rely on
        // the function call interface or subprocess approach
        false
    }

    fn build_plugin_library(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        println!("Building {} as shared library...", plugin_name);

        let output = Command::new("cargo")
            .args(&["build", "--release", "--lib"])
            .current_dir(plugin_dir)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to build plugin library: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to build plugin library: {}",
                stderr
            )));
        }

        // Copy the built library to the plugin directory
        let target_dir = plugin_dir.join("target").join("release");
        let lib_extensions = if cfg!(target_os = "windows") {
            vec!["dll"]
        } else if cfg!(target_os = "macos") {
            vec!["dylib"]
        } else {
            vec!["so"]
        };

        for ext in lib_extensions {
            let lib_name = format!("lib{}.{}", plugin_name, ext);
            let lib_path = target_dir.join(&lib_name);
            if lib_path.exists() {
                let dest_path = plugin_dir.join(&lib_name);
                std::fs::copy(&lib_path, &dest_path)
                    .map_err(|e| WasmrunError::from(format!("Failed to copy library: {}", e)))?;
                break;
            }
        }

        Ok(())
    }

    fn setup_plugin_source(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        // Create a temporary directory for downloading the source
        let temp_dir = std::env::temp_dir().join(format!("wasmrun_{}", plugin_name));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create temp directory: {}", e)))?;

        // Use cargo download to get the source (if available) or create basic structure
        match plugin_name {
            "wasmrust" => {
                // Try to download source or create from known structure
                if let Err(_) = Self::download_crate_source(plugin_name, &temp_dir) {
                    Self::create_wasmrust_structure(plugin_dir)?;
                } else {
                    Self::copy_plugin_files(&temp_dir, plugin_dir)?;
                }
            }
            "wasmgo" => {
                Self::create_wasmgo_structure(plugin_dir)?;
            }
            _ => return Err(WasmrunError::from(format!("Unknown plugin: {}", plugin_name))),
        }

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    fn download_crate_source(plugin_name: &str, temp_dir: &Path) -> Result<()> {
        // Try to use cargo to download the crate source
        let output = Command::new("cargo")
            .args(&["download", plugin_name])
            .current_dir(temp_dir)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to download crate: {}", e)))?;

        if !output.status.success() {
            return Err(WasmrunError::from("Failed to download crate source".to_string()));
        }

        Ok(())
    }

    fn copy_plugin_files(source_dir: &Path, plugin_dir: &Path) -> Result<()> {
        // Copy Cargo.toml and other necessary files
        let cargo_toml_src = source_dir.join("Cargo.toml");
        let cargo_toml_dst = plugin_dir.join("Cargo.toml");
        
        if cargo_toml_src.exists() {
            std::fs::copy(&cargo_toml_src, &cargo_toml_dst)
                .map_err(|e| WasmrunError::from(format!("Failed to copy Cargo.toml: {}", e)))?;
        }

        // Copy src directory if it exists
        let src_dir = source_dir.join("src");
        if src_dir.exists() {
            let dst_src_dir = plugin_dir.join("src");
            copy_dir_recursive(&src_dir, &dst_src_dir)?;
        }

        Ok(())
    }

    fn create_wasmrust_structure(plugin_dir: &Path) -> Result<()> {
        // Create a basic Cargo.toml for wasmrust based on the known structure
        let cargo_toml_content = r#"[package]
name = "wasmrust"
version = "0.2.1"
edition = "2021"
authors = ["Kumar Anirudha <wasm@anirudha.dev>"]
description = "Rust WebAssembly plugin for Wasmrun"

[lib]
name = "wasmrust"
crate-type = ["cdylib", "rlib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
thiserror = "1.0"

[package.metadata.wasm-plugin]
name = "rust"
extensions = ["rs"]
entry_files = ["Cargo.toml"]

[package.metadata.wasm-plugin.capabilities]
compile_wasm = true
compile_webapp = true
live_reload = true
optimization = true
"#;

        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {}", e)))?;

        Ok(())
    }

    fn create_wasmgo_structure(plugin_dir: &Path) -> Result<()> {
        // Create a basic structure for wasmgo
        let cargo_toml_content = r#"[package]
name = "wasmgo"
version = "0.1.0"
edition = "2021"
description = "Go WebAssembly plugin for Wasmrun"

[lib]
name = "wasmgo"
crate-type = ["cdylib", "rlib"]

[package.metadata.wasm-plugin]
name = "go"
extensions = ["go"]
entry_files = ["go.mod"]

[package.metadata.wasm-plugin.capabilities]
compile_wasm = true
compile_webapp = false
live_reload = true
optimization = true
"#;

        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {}", e)))?;

        Ok(())
    }

    fn detect_plugin_version_from_metadata(plugin_name: &str) -> Option<String> {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            // Try to read version from Cargo.toml in plugin directory
            let cargo_toml_path = plugin_dir.join("Cargo.toml");
            if cargo_toml_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
                    // Parse TOML to get version
                    if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                        if let Some(package) = parsed.get("package") {
                            if let Some(version) = package.get("version") {
                                if let Some(version_str) = version.as_str() {
                                    return Some(version_str.to_string());
                                }
                            }
                        }
                    }
                }
            }

            // Try to read from metadata file
            let metadata_path = plugin_dir.join(".wasmrun_metadata");
            if metadata_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                    if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                        if let Some(metadata) = parsed.get("metadata") {
                            if let Some(version) = metadata.get("version") {
                                if let Some(version_str) = version.as_str() {
                                    return Some(version_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not determine home directory"))?;
        Ok(home_dir.join(".wasmrun").join("plugins").join(plugin_name))
    }

    fn create_plugin_manifest(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        let manifest_content = match plugin_name {
            "wasmrust" => {
                r#"[plugin]
name = "wasmrust"
description = "Rust WebAssembly plugin for Wasmrun"
extensions = ["rs", "toml"]
entry_files = ["Cargo.toml"]

[capabilities]
compile_wasm = true
compile_webapp = true
live_reload = true
optimization = true
custom_targets = ["wasm32-unknown-unknown", "web"]

[dependencies]
tools = ["cargo", "rustc", "wasm-pack"]
"#
            }
            "wasmgo" => {
                r#"[plugin]
name = "wasmgo"
description = "Go WebAssembly plugin for Wasmrun"
extensions = ["go", "mod"]
entry_files = ["go.mod", "main.go"]

[capabilities]
compile_wasm = true
compile_webapp = false
live_reload = true
optimization = true
custom_targets = ["wasm"]

[dependencies]
tools = ["tinygo"]
"#
            }
            _ => return Err(WasmrunError::from(format!("Unknown plugin: {}", plugin_name))),
        };

        let manifest_path = plugin_dir.join("wasmrun.toml");
        std::fs::write(&manifest_path, manifest_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin manifest: {}", e)))?;

        Ok(())
    }

    fn create_metadata_file(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        let version = Self::detect_plugin_version_from_metadata(plugin_name)
            .unwrap_or_else(|| "unknown".to_string());
        let metadata_content = format!(
            "[metadata]\ninstalled_at = \"{}\"\nversion = \"{}\"\nbinary_path = \"{}\"\n",
            chrono::Utc::now().to_rfc3339(),
            version,
            plugin_name
        );

        let metadata_path = plugin_dir.join(".wasmrun_metadata");
        std::fs::write(&metadata_path, metadata_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create metadata file: {}", e)))?;

        Ok(())
    }

    pub fn uninstall_plugin_library(plugin_name: &str) -> Result<()> {
        // For library-based plugins, we mainly need to remove the directory
        // The cargo uninstall is optional since the plugin functions as a library
        println!("Removing plugin library files...");
        
        let output = Command::new("cargo")
            .args(&["uninstall", plugin_name])
            .output();

        match output {
            Ok(result) => {
                if !result.status.success() {
                    println!("Warning: cargo uninstall failed, but continuing with directory removal");
                }
            }
            Err(_) => {
                println!("Warning: Could not run cargo uninstall, but continuing with directory removal");
            }
        }

        Ok(())
    }

    pub fn verify_plugin_installation(plugin_name: &str) -> Result<PluginVerificationResult> {
        let mut result = PluginVerificationResult::new(plugin_name);

        // Check if plugin library files are available
        result.binary_available = Self::is_plugin_library_installed(plugin_name);

        // Check directory structure
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            result.directory_exists = plugin_dir.exists();
            
            if result.directory_exists {
                result.manifest_exists = plugin_dir.join("wasmrun.toml").exists();
                result.metadata_exists = plugin_dir.join(".wasmrun_metadata").exists();
                
                // Check for Cargo.toml which contains plugin metadata
                let cargo_toml_exists = plugin_dir.join("Cargo.toml").exists();
                result.manifest_exists = result.manifest_exists || cargo_toml_exists;

                println!("Plugin directory verification:");
                println!("  Directory exists: {}", result.directory_exists);
                println!("  Manifest exists: {}", result.manifest_exists);
                println!("  Metadata exists: {}", result.metadata_exists);
                println!("  Cargo.toml exists: {}", cargo_toml_exists);
            }
        }

        // Check dependencies
        result.dependencies_available = Self::check_plugin_dependencies(plugin_name);

        result.is_functional = result.binary_available && 
                              result.directory_exists && 
                              result.manifest_exists;

        println!("Plugin '{}' functional status: {}", plugin_name, result.is_functional);

        Ok(result)
    }

    fn check_plugin_dependencies(plugin_name: &str) -> bool {
        match plugin_name {
            "wasmrust" => {
                Self::is_tool_available("cargo") && 
                Self::is_tool_available("rustc") &&
                Self::is_wasm_target_installed()
            }
            "wasmgo" => {
                Self::is_tool_available("tinygo")
            }
            _ => false,
        }
    }

    fn is_tool_available(tool: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        Command::new(which_cmd)
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn is_wasm_target_installed() -> bool {
        Command::new("rustup")
            .args(&["target", "list", "--installed"])
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("wasm32-unknown-unknown")
            })
            .unwrap_or(false)
    }
}

// Helper function for recursive directory copying
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(to)
        .map_err(|e| WasmrunError::from(format!("Failed to create directory: {}", e)))?;

    for entry in std::fs::read_dir(from)
        .map_err(|e| WasmrunError::from(format!("Failed to read directory: {}", e)))?
    {
        let entry = entry
            .map_err(|e| WasmrunError::from(format!("Failed to read directory entry: {}", e)))?;
        let file_type = entry.file_type()
            .map_err(|e| WasmrunError::from(format!("Failed to get file type: {}", e)))?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            std::fs::copy(&from_path, &to_path)
                .map_err(|e| WasmrunError::from(format!("Failed to copy file: {}", e)))?;
        }
    }

    Ok(())
}

#[derive(Debug)]
pub struct InstallationResult {
    pub plugin_name: String,
    pub binary_installed: bool,
    pub binary_already_installed: bool,
    pub directory_created: bool,
    pub version: String,
}

impl InstallationResult {
    fn new(plugin_name: &str) -> Self {
        Self {
            plugin_name: plugin_name.to_string(),
            binary_installed: false,
            binary_already_installed: false,
            directory_created: false,
            version: "unknown".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct PluginVerificationResult {
    pub plugin_name: String,
    pub binary_available: bool,
    pub directory_exists: bool,
    pub manifest_exists: bool,
    pub metadata_exists: bool,
    pub dependencies_available: bool,
    pub is_functional: bool,
}

impl PluginVerificationResult {
    fn new(plugin_name: &str) -> Self {
        Self {
            plugin_name: plugin_name.to_string(),
            binary_available: false,
            directory_exists: false,
            manifest_exists: false,
            metadata_exists: false,
            dependencies_available: false,
            is_functional: false,
        }
    }
}
