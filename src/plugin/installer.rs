use crate::error::{Result, WasmrunError};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PluginInstaller;

impl PluginInstaller {
    pub fn install_external_plugin(plugin_name: &str) -> Result<InstallationResult> {
        let mut result = InstallationResult::new(plugin_name);

        // Validate plugin name
        // TODO: Move to either plugin registration or open plugin registry
        if !Self::is_supported_plugin(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Unsupported plugin: {plugin_name}. Supported: wasmrust, wasmgo"
            )));
        }

        // Check if cargo is available
        if !Self::is_cargo_available() {
            return Err(WasmrunError::from(
                "cargo is required for plugin installation but was not found",
            ));
        }

        // Get plugin directory
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;

        // Check if already installed by looking for the plugin files in the directory
        // TODO: Maintain plugin registry to check if plugin is already installed
        if Self::is_plugin_library_installed(plugin_name) {
            result.binary_already_installed = true;
            let current_version = Self::detect_plugin_version_from_metadata(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());

            // Check if there's a newer version available
            if let Some(latest_version) = Self::get_latest_crates_io_version(plugin_name) {
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
            // Install the plugin library files
            Self::install_plugin_library(plugin_name, &plugin_dir)?;
            result.binary_installed = true;
            result.version = Self::get_latest_crates_io_version(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());

            println!(
                "Plugin '{}' library files installed successfully (v{})",
                plugin_name, result.version
            );
        }

        Ok(result)
    }

    #[allow(dead_code)]
    pub fn update_plugin_metadata(plugin_name: &str, new_version: &str) -> Result<()> {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            let metadata_content = format!(
                "[metadata]
installed_at = \"{}\"
version = \"{}\"
plugin_name = \"{}\"
install_method = \"cargo\"
updated_at = \"{}\"
",
                chrono::Utc::now().to_rfc3339(),
                new_version,
                plugin_name,
                chrono::Utc::now().to_rfc3339()
            );

            let metadata_path = plugin_dir.join(".wasmrun_metadata");
            std::fs::write(&metadata_path, metadata_content)
                .map_err(|e| WasmrunError::from(format!("Failed to update metadata file: {e}")))?;

            println!("📝 Updated metadata file with version: {new_version}");
        }
        Ok(())
    }

    pub fn setup_plugin_directory(plugin_name: &str) -> Result<PathBuf> {
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;

        // Create plugin directory
        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        // Create manifest file
        Self::create_plugin_manifest(plugin_name, &plugin_dir)?;

        // Create metadata file
        Self::create_metadata_file(plugin_name, &plugin_dir)?;

        Ok(plugin_dir)
    }

    pub fn remove_plugin_directory(plugin_name: &str) -> Result<()> {
        let plugin_dir = Self::get_plugin_directory(plugin_name)?;
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

    fn is_cargo_available() -> bool {
        Command::new("cargo")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn is_plugin_library_installed(plugin_name: &str) -> bool {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
            let cargo_toml = plugin_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                // Verify it's a wasm plugin by checking content
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[wasm_plugin]")
                        || content.contains("wasmrun")
                        || content.contains("wasm-bindgen")
                    {
                        return true;
                    }
                }
                return true;
            }

            // Secondary check: manifest file
            let manifest_path = plugin_dir.join("wasmrun.toml");
            if manifest_path.exists() {
                return true;
            }

            // Tertiary check: metadata file
            let metadata_path = plugin_dir.join(".wasmrun_metadata");
            if metadata_path.exists() {
                return true;
            }

            // Quaternary check: shared library files (for dynamic loading scenarios)
            // TODO: Not implemented yet, but can be used for future plugin types
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

    /// Install plugin library files and setup directory structure
    fn install_plugin_library(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        println!("Setting up {plugin_name} plugin library files...");

        // Create plugin directory if it doesn't exist
        std::fs::create_dir_all(plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        // Setup the plugin source and metadata
        Self::setup_plugin_source(plugin_name, plugin_dir)?;
        println!("Plugin '{plugin_name}' library files setup completed");

        Ok(())
    }

    #[allow(dead_code)]
    fn should_build_shared_library(_plugin_name: &str) -> bool {
        // For now, we'll skip building shared libraries and rely on
        // the function call interface or subprocess approach
        false
    }

    #[allow(dead_code)]
    fn build_plugin_library(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        println!("Building {plugin_name} as shared library...");

        let output = Command::new("cargo")
            .args(["build", "--release", "--lib"])
            .current_dir(plugin_dir)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to build plugin library: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to build plugin library: {stderr}"
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
            let lib_name = format!("lib{plugin_name}.{ext}");
            let lib_path = target_dir.join(&lib_name);
            if lib_path.exists() {
                let dest_path = plugin_dir.join(&lib_name);
                std::fs::copy(&lib_path, &dest_path)
                    .map_err(|e| WasmrunError::from(format!("Failed to copy library: {e}")))?;
                break;
            }
        }

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

    /// Setup wasmrust plugin files
    fn setup_wasmrust_plugin(plugin_dir: &Path) -> Result<()> {
        let version =
            Self::get_latest_crates_io_version("wasmrust").unwrap_or_else(|| "0.3.0".to_string());
        // Create Cargo.toml for wasmrust plugin
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

        println!("📦 Created Cargo.toml with version: {version}");
        Ok(())
    }

    /// Setup wasmgo plugin files
    fn setup_wasmgo_plugin(plugin_dir: &Path) -> Result<()> {
        // Get the latest version from crates.io
        let version =
            Self::get_latest_crates_io_version("wasmgo").unwrap_or_else(|| "0.3.0".to_string());

        let cargo_toml_content = format!(
            r#"[package]
name = "wasmgo"
version = "{version}"
edition = "2021"
description = "Go WebAssembly plugin for Wasmrun"

[lib]
name = "wasmgo"
crate-type = ["cdylib", "rlib"]

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

        println!("📦 Created Cargo.toml with version: {version}");
        Ok(())
    }

    #[allow(dead_code)]
    fn download_crate_source(plugin_name: &str, temp_dir: &Path) -> Result<()> {
        // Try to use cargo to download the crate source
        let output = Command::new("cargo")
            .args(["download", plugin_name])
            .current_dir(temp_dir)
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to download crate: {e}")))?;

        if !output.status.success() {
            return Err(WasmrunError::from(
                "Failed to download crate source".to_string(),
            ));
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn copy_plugin_files(source_dir: &Path, plugin_dir: &Path) -> Result<()> {
        // Copy Cargo.toml and other necessary files
        let cargo_toml_src = source_dir.join("Cargo.toml");
        let cargo_toml_dst = plugin_dir.join("Cargo.toml");

        if cargo_toml_src.exists() {
            std::fs::copy(&cargo_toml_src, &cargo_toml_dst)
                .map_err(|e| WasmrunError::from(format!("Failed to copy Cargo.toml: {e}")))?;
        }

        // Copy src directory if it exists
        let src_dir = source_dir.join("src");
        if src_dir.exists() {
            let dst_src_dir = plugin_dir.join("src");
            copy_dir_recursive(&src_dir, &dst_src_dir)?;
        }

        Ok(())
    }

    #[allow(dead_code)]
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
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {e}")))?;

        Ok(())
    }

    #[allow(dead_code)]
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
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {e}")))?;

        Ok(())
    }

    fn detect_plugin_version_from_metadata(plugin_name: &str) -> Option<String> {
        if let Ok(plugin_dir) = Self::get_plugin_directory(plugin_name) {
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

            // Metadata file
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

        Self::get_latest_crates_io_version(plugin_name)
    }

    #[allow(dead_code)]
    fn extract_version_from_line(line: &str) -> Option<String> {
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                let version = &line[start + 1..start + 1 + end];
                // Validate that it looks like a version
                if version.chars().any(|c| c.is_ascii_digit()) {
                    return Some(version.to_string());
                }
            }
        }
        None
    }

    // Get the latest version from crates.io
    fn get_latest_crates_io_version(plugin_name: &str) -> Option<String> {
        if let Ok(output) = std::process::Command::new("cargo")
            .args(["search", plugin_name, "--limit", "1"])
            .output()
        {
            if output.status.success() {
                let search_output = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = search_output.lines().next() {
                    // Parse output like: wasmrust = "0.3.0"    # Description
                    if let Some(start) = line.find('"') {
                        if let Some(end) = line[start + 1..].find('"') {
                            let version = &line[start + 1..start + 1 + end];
                            return Some(version.to_string());
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
            _ => return Err(WasmrunError::from(format!("Unknown plugin: {plugin_name}"))),
        };

        let manifest_path = plugin_dir.join("wasmrun.toml");
        std::fs::write(&manifest_path, manifest_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin manifest: {e}")))?;

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
            .map_err(|e| WasmrunError::from(format!("Failed to create metadata file: {e}")))?;

        Ok(())
    }

    pub fn uninstall_plugin_library(plugin_name: &str) -> Result<()> {
        // For library-based plugins, we mainly need to remove the directory
        // The cargo uninstall is optional since the plugin functions as a library
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
                println!(
                    "Warning: Could not run cargo uninstall, but continuing with directory removal"
                );
            }
        }

        Ok(())
    }

    pub fn verify_plugin_installation(plugin_name: &str) -> Result<PluginVerificationResult> {
        let mut result = PluginVerificationResult::new(plugin_name);

        // Check if plugin library files are available (not binary in PATH)
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
                println!("  Cargo.toml exists: {cargo_toml_exists}");

                // Check for shared library files
                let lib_extensions = ["so", "dylib", "dll"];
                let mut lib_exists = false;
                for ext in &lib_extensions {
                    let lib_path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
                    if lib_path.exists() {
                        lib_exists = true;
                        println!("  Library file exists: {}", lib_path.display());
                        break;
                    }
                }

                if !lib_exists {
                    println!(
                        "  No shared library files found (this is normal for source-based plugins)"
                    );
                }
            }
        }

        // Check dependencies
        result.dependencies_available = Self::check_plugin_dependencies(plugin_name);

        // Plugin is functional if library files exist and directory structure is valid
        result.is_functional = result.binary_available
            && result.directory_exists
            && result.manifest_exists
            && result.dependencies_available;

        println!(
            "Plugin '{}' functional status: {}",
            plugin_name, result.is_functional
        );

        Ok(result)
    }

    fn check_plugin_dependencies(plugin_name: &str) -> bool {
        match plugin_name {
            "wasmrust" => {
                Self::is_tool_available("cargo")
                    && Self::is_tool_available("rustc")
                    && Self::is_wasm_target_installed()
            }
            "wasmgo" => Self::is_tool_available("tinygo"),
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
            .args(["target", "list", "--installed"])
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
        .map_err(|e| WasmrunError::from(format!("Failed to create directory: {e}")))?;

    for entry in std::fs::read_dir(from)
        .map_err(|e| WasmrunError::from(format!("Failed to read directory: {e}")))?
    {
        let entry = entry
            .map_err(|e| WasmrunError::from(format!("Failed to read directory entry: {e}")))?;
        let file_type = entry
            .file_type()
            .map_err(|e| WasmrunError::from(format!("Failed to get file type: {e}")))?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            std::fs::copy(&from_path, &to_path)
                .map_err(|e| WasmrunError::from(format!("Failed to copy file: {e}")))?;
        }
    }

    Ok(())
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
