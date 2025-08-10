use crate::error::{Result, WasmrunError};
use crate::utils::{PluginUtils, SystemUtils};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PluginInstaller;

impl PluginInstaller {
    pub fn install_external_plugin(plugin_name: &str) -> Result<InstallationResult> {
        let mut result = InstallationResult::new(plugin_name);

        if !Self::is_supported_plugin(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Unsupported plugin: {plugin_name}. Supported: wasmrust, wasmgo"
            )));
        }

        if !SystemUtils::is_tool_available("cargo") {
            return Err(WasmrunError::from(
                "cargo is required for plugin installation but was not found",
            ));
        }

        let plugin_dir = PluginUtils::get_plugin_directory(plugin_name)?;

        if Self::is_plugin_library_installed(plugin_name) {
            result.binary_already_installed = true;
            let current_version = PluginUtils::detect_plugin_version_from_metadata(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());

            if let Some(latest_version) = SystemUtils::get_latest_crates_version(plugin_name) {
                if current_version != latest_version && current_version != "unknown" {
                    println!("📦 Installed version: {current_version}");
                    println!("🆕 Latest version available: {latest_version}");
                    println!("💡 Run 'wasmrun plugin update {plugin_name}' to upgrade");
                }
                result.version = latest_version;
            } else {
                result.version = current_version;
            }

            println!(
                "Plugin '{}' library files already exist (v{})",
                plugin_name, result.version
            );
        } else {
            Self::install_plugin_library(plugin_name, &plugin_dir)?;
            result.binary_installed = true;
            result.version = SystemUtils::get_latest_crates_version(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());

            println!(
                "Plugin '{}' library files installed successfully (v{})",
                plugin_name, result.version
            );
        }

        Ok(result)
    }

    pub fn update_plugin_metadata(plugin_name: &str, new_version: &str) -> Result<()> {
        if let Ok(plugin_dir) = PluginUtils::get_plugin_directory(plugin_name) {
            PluginUtils::create_metadata_file(plugin_name, &plugin_dir, new_version)?;
            println!("📝 Updated metadata file with version: {new_version}");
        }
        Ok(())
    }

    pub fn setup_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        let plugin_dir = PluginUtils::get_plugin_directory(plugin_name)?;

        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        Self::create_plugin_manifest(plugin_name, &plugin_dir)?;
        let version = SystemUtils::get_latest_crates_version(plugin_name)
            .unwrap_or_else(|| "unknown".to_string());
        PluginUtils::create_metadata_file(plugin_name, &plugin_dir, &version)?;

        Ok(plugin_dir)
    }

    pub fn remove_plugin_directory(plugin_name: &str) -> Result<()> {
        let plugin_dir = PluginUtils::get_plugin_directory(plugin_name)?;
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir).map_err(|e| {
                WasmrunError::from(format!("Failed to remove plugin directory: {e}"))
            })?;
        }
        Ok(())
    }

    fn is_supported_plugin(plugin_name: &str) -> bool {
        matches!(plugin_name, "wasmrust" | "wasmgo")
    }

    fn is_plugin_library_installed(plugin_name: &str) -> bool {
        if let Ok(plugin_dir) = PluginUtils::get_plugin_directory(plugin_name) {
            let cargo_toml = plugin_dir.join("Cargo.toml");
            let src_lib = plugin_dir.join("src").join("lib.rs");

            if cargo_toml.exists() && src_lib.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[wasmrun.plugin]")
                        || content.contains("wasmrun")
                        || content.contains("wasm-bindgen")
                    {
                        return true;
                    }
                }
                return true;
            }

            let manifest_path = plugin_dir.join("wasmrun.toml");
            if manifest_path.exists() {
                return true;
            }

            let metadata_path = plugin_dir.join(".wasmrun_metadata");
            if metadata_path.exists() {
                return true;
            }

            let lib_extensions = ["so", "dylib", "dll"];
            for ext in &lib_extensions {
                let lib_path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
                if lib_path.exists() {
                    return true;
                }
            }
        }

        false
    }

    fn install_plugin_library(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        println!("Setting up {plugin_name} plugin library files...");

        std::fs::create_dir_all(plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        Self::setup_plugin_source(plugin_name, plugin_dir)?;
        println!("Plugin '{plugin_name}' library files setup completed");

        Ok(())
    }

    fn setup_plugin_source(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        match plugin_name {
            "wasmrust" => Self::setup_wasmrust_plugin(plugin_dir),
            "wasmgo" => Self::setup_wasmgo_plugin(plugin_dir),
            _ => Err(WasmrunError::from(format!(
                "Unsupported plugin: {plugin_name}"
            ))),
        }
    }

    fn setup_wasmrust_plugin(plugin_dir: &Path) -> Result<()> {
        let version = SystemUtils::get_latest_crates_version("wasmrust")
            .unwrap_or_else(|| "0.3.0".to_string());

        let cargo_toml_content = format!(
            r#"[package]
name = "wasmrust"
version = "{version}"
edition = "2021"
description = "Rust to WebAssembly compiler plugin for wasmrun"
authors = ["Kumar Anirudha"]

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"
web-sys = "0.3"
js-sys = "0.3"
serde = {{ version = "1.0", features = ["derive"] }}
toml = "0.8"

[wasmrun.plugin]
name = "wasmrust"
version = "{version}"
capabilities = ["compile_wasm", "compile_webapp", "live_reload", "optimization"]
extensions = ["rs", "toml"]
entry_files = ["Cargo.toml", "src/main.rs"]
dependencies = ["cargo", "rustc"]
"#
        );

        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {e}")))?;

        let src_dir = plugin_dir.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create src directory: {e}")))?;

        let lib_rs_content = Self::get_wasmrust_lib_content();
        let lib_rs_path = src_dir.join("lib.rs");
        std::fs::write(&lib_rs_path, lib_rs_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create lib.rs: {e}")))?;

        println!("📦 Created Cargo.toml with version: {version}");
        println!("📁 Created src/lib.rs with plugin implementation");
        Ok(())
    }

    fn setup_wasmgo_plugin(plugin_dir: &Path) -> Result<()> {
        let version = SystemUtils::get_latest_crates_version("wasmgo")
            .unwrap_or_else(|| "0.3.0".to_string());

        let cargo_toml_content = format!(
            r#"[package]
name = "wasmgo"
version = "{version}"
edition = "2021"
description = "Go WebAssembly plugin for Wasmrun"

[lib]
name = "wasmgo"
crate-type = ["cdylib", "rlib"]

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
toml = "0.8"

[wasmrun.plugin]
name = "wasmgo"
version = "{version}"
capabilities = ["compile_wasm", "live_reload", "optimization"]
extensions = ["go", "mod"]
entry_files = ["go.mod", "main.go"]
dependencies = ["tinygo"]
"#
        );

        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {e}")))?;

        let src_dir = plugin_dir.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create src directory: {e}")))?;

        let lib_rs_content = Self::get_wasmgo_lib_content();
        let lib_rs_path = src_dir.join("lib.rs");
        std::fs::write(&lib_rs_path, lib_rs_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create lib.rs: {e}")))?;

        println!("📦 Created Cargo.toml with version: {version}");
        println!("📁 Created src/lib.rs with Go plugin implementation");
        Ok(())
    }

    fn get_wasmrust_lib_content() -> &'static str {
        r#"use std::path::Path;
use std::process::Command;

pub struct WasmRustBuilder;

impl WasmRustBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, project_path: &Path, output_path: &Path) -> Result<(), String> {
        let cargo_toml = project_path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err("No Cargo.toml found in project directory".to_string());
        }

        if Self::is_wasm_pack_available() {
            self.build_with_wasm_pack(project_path, output_path)
        } else {
            self.build_with_cargo(project_path, output_path)
        }
    }

    fn build_with_wasm_pack(&self, project_path: &Path, output_path: &Path) -> Result<(), String> {
        let output = Command::new("wasm-pack")
            .args(&["build", "--target", "web", "--out-dir"])
            .arg(output_path)
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to execute wasm-pack: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("wasm-pack build failed: {}", stderr));
        }

        Ok(())
    }

    fn build_with_cargo(&self, project_path: &Path, output_path: &Path) -> Result<(), String> {
        let output = Command::new("cargo")
            .args(&[
                "build",
                "--target", "wasm32-unknown-unknown",
                "--release",
                "--target-dir"
            ])
            .arg(output_path)
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to execute cargo: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Cargo build failed: {}", stderr));
        }

        Ok(())
    }

    fn is_wasm_pack_available() -> bool {
        Command::new("wasm-pack")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn can_handle_project(&self, path: &Path) -> bool {
        path.join("Cargo.toml").exists()
    }

    pub fn get_supported_extensions(&self) -> Vec<&'static str> {
        vec!["rs", "toml"]
    }

    pub fn get_entry_files(&self) -> Vec<&'static str> {
        vec!["Cargo.toml", "src/main.rs", "src/lib.rs"]
    }

    pub fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();
        
        if !Self::is_command_available("cargo") {
            missing.push("cargo".to_string());
        }
        
        if !Self::is_command_available("rustc") {
            missing.push("rustc".to_string());
        }

        missing
    }

    fn is_command_available(cmd: &str) -> bool {
        Command::new(cmd)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
"#
    }

    fn get_wasmgo_lib_content() -> &'static str {
        r#"use std::path::Path;
use std::process::Command;

pub struct WasmGoBuilder;

impl WasmGoBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, project_path: &Path, output_path: &Path) -> Result<(), String> {
        let go_mod = project_path.join("go.mod");
        if !go_mod.exists() {
            return Err("No go.mod found in project directory".to_string());
        }

        self.build_with_tinygo(project_path, output_path)
    }

    fn build_with_tinygo(&self, project_path: &Path, output_path: &Path) -> Result<(), String> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }

        let output = Command::new("tinygo")
            .args(&[
                "build",
                "-target", "wasm",
                "-o"
            ])
            .arg(output_path.join("main.wasm"))
            .arg(".")
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to execute tinygo: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("TinyGo build failed: {}", stderr));
        }

        Ok(())
    }

    pub fn can_handle_project(&self, path: &Path) -> bool {
        path.join("go.mod").exists() || path.join("main.go").exists()
    }

    pub fn get_supported_extensions(&self) -> Vec<&'static str> {
        vec!["go", "mod"]
    }

    pub fn get_entry_files(&self) -> Vec<&'static str> {
        vec!["go.mod", "main.go"]
    }

    pub fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();
        
        if !Self::is_command_available("tinygo") {
            missing.push("tinygo".to_string());
        }

        missing
    }

    fn is_command_available(cmd: &str) -> bool {
        Command::new(cmd)
            .arg("version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
"#
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
            _ => return Err(WasmrunError::from(format!("Unknown plugin: {plugin_name}"))),
        };

        let manifest_path = plugin_dir.join("wasmrun.toml");
        std::fs::write(&manifest_path, manifest_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin manifest: {e}")))?;

        Ok(())
    }

    pub fn uninstall_plugin_library(plugin_name: &str) -> Result<()> {
        println!("Removing plugin library files...");

        let output = Command::new("cargo")
            .args(["uninstall", plugin_name])
            .output();

        match output {
            Ok(result) => {
                if !result.status.success() {
                    println!(
                        "Warning: cargo uninstall failed, but continuing with directory removal"
                    );
                }
            }
            Err(_) => {
                println!("Warning: cargo not available, skipping binary uninstall");
            }
        }

        Self::remove_plugin_directory(plugin_name)?;
        Ok(())
    }

    pub fn verify_plugin_installation(plugin_name: &str) -> Result<PluginVerificationResult> {
        let mut result = PluginVerificationResult::new(plugin_name);

        if let Ok(plugin_dir) = PluginUtils::get_plugin_directory(plugin_name) {
            result.directory_exists = plugin_dir.exists();

            if result.directory_exists {
                let cargo_toml_path = plugin_dir.join("Cargo.toml");
                result.binary_available = cargo_toml_path.exists();

                let manifest_path = plugin_dir.join("wasmrun.toml");
                result.manifest_exists = manifest_path.exists();

                let metadata_path = plugin_dir.join(".wasmrun_metadata");
                result.metadata_exists = metadata_path.exists();

                let src_lib_path = plugin_dir.join("src").join("lib.rs");
                let has_source_files = src_lib_path.exists();

                let missing_deps = PluginUtils::check_plugin_dependencies(plugin_name);
                result.dependencies_available = missing_deps.is_empty();

                result.is_functional = result.binary_available
                    && result.manifest_exists
                    && result.metadata_exists
                    && has_source_files
                    && result.dependencies_available;

                println!("Plugin directory verification:");
                println!("  Directory exists: {}", result.directory_exists);
                println!("  Manifest exists: {}", result.manifest_exists);
                println!("  Metadata exists: {}", result.metadata_exists);
                println!("  Cargo.toml exists: {}", result.binary_available);
                println!("  Source files exist: {has_source_files}");

                if !missing_deps.is_empty() {
                    println!("  Missing dependencies: {:?}", missing_deps);
                    println!("  ⚠️  Some dependencies are missing, but plugin files are installed");
                }

                if !has_source_files {
                    println!("  ❌ Source files missing in src/ directory");
                } else if result.is_functional {
                    println!("  ✅ All plugin files installed correctly");
                }
            }
        }

        println!(
            "Plugin '{}' functional status: {}",
            plugin_name, result.is_functional
        );
        Ok(result)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct InstallationResult {
    pub plugin_name: String,
    pub success: bool,
    pub version: String,
    pub binary_installed: bool,
    pub binary_already_installed: bool,
    pub directory_created: bool,
    pub message: String,
}

impl InstallationResult {
    fn new(plugin_name: &str) -> Self {
        Self {
            plugin_name: plugin_name.to_string(),
            success: false,
            version: "unknown".to_string(),
            binary_installed: false,
            binary_already_installed: false,
            directory_created: false,
            message: String::new(),
        }
    }
}

#[derive(Debug)]
pub struct PluginVerificationResult {
    #[allow(dead_code)]
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
