use crate::error::{Result, WasmrunError};
use crate::plugin::registry::PluginRegistry;
use crate::utils::{PluginUtils, SystemUtils};
use std::path::{Path, PathBuf};

pub struct PluginInstaller;

#[derive(Debug, Clone)]
pub struct InstallationResult {
    #[allow(dead_code)]
    pub plugin_name: String,
    pub version: String,
    pub binary_installed: bool,
    pub binary_already_installed: bool,
}

impl InstallationResult {
    pub fn new(plugin_name: &str) -> Self {
        Self {
            plugin_name: plugin_name.to_string(),
            version: String::new(),
            binary_installed: false,
            binary_already_installed: false,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginVerificationResult {
    pub is_functional: bool,
    pub version: String,
    pub missing_dependencies: Vec<String>,
    pub install_path: String,
}

impl PluginInstaller {
    pub fn install_external_plugin(plugin_name: &str) -> Result<InstallationResult> {
        let mut result = InstallationResult::new(plugin_name);

        if !Self::is_supported_plugin(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' not found or not a valid WebAssembly plugin"
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
            let install_result = Self::install_generic_plugin(plugin_name, &plugin_dir)?;

            result.binary_installed = install_result.binary_installed;
            result.version = install_result.version.clone();

            if install_result.binary_installed {
                println!(
                    "Plugin '{}' binary and library files installed successfully (v{})",
                    plugin_name, install_result.version
                );
            } else {
                println!(
                    "Plugin '{}' template created successfully (v{})",
                    plugin_name, install_result.version
                );
            }
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn verify_plugin_installation(plugin_name: &str) -> Result<PluginVerificationResult> {
        let validation = PluginUtils::validate_plugin_installation(plugin_name)?;

        Ok(PluginVerificationResult {
            is_functional: validation.is_functional,
            version: validation.version.unwrap_or_else(|| "unknown".to_string()),
            missing_dependencies: validation.missing_dependencies,
            install_path: validation.install_path.unwrap_or_default(),
        })
    }

    #[allow(dead_code)]
    pub fn update_generic_plugin(plugin_name: &str) -> Result<()> {
        println!("🔄 Updating {plugin_name}...");

        let plugin_dir = PluginUtils::get_plugin_directory(plugin_name)?;

        let output = std::process::Command::new("cargo")
            .args([
                "install",
                plugin_name,
                "--force",
                "--root",
                &plugin_dir.to_string_lossy(),
                "--features",
                "cli",
            ])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to update plugin: {e}")))?;

        if output.status.success() {
            println!("✅ Plugin {plugin_name} updated successfully");

            if let Some(latest_version) = SystemUtils::get_latest_crates_version(plugin_name) {
                Self::update_plugin_metadata(plugin_name, &latest_version)?;
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Plugin update failed: {stderr}"
            )));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn cleanup_generic_plugin(plugin_name: &str) -> Result<()> {
        let plugin_dir = PluginUtils::get_plugin_directory(plugin_name)?;

        if plugin_dir.exists() {
            let target_dir = plugin_dir.join("target");
            if target_dir.exists() {
                std::fs::remove_dir_all(&target_dir).map_err(|e| {
                    WasmrunError::from(format!("Failed to clean target directory: {e}"))
                })?;
            }

            let pkg_dir = plugin_dir.join("pkg");
            if pkg_dir.exists() {
                std::fs::remove_dir_all(&pkg_dir).map_err(|e| {
                    WasmrunError::from(format!("Failed to clean pkg directory: {e}"))
                })?;
            }

            println!("✅ Cleaned {plugin_name} build artifacts");
        }

        Ok(())
    }

    fn is_supported_plugin(plugin_name: &str) -> bool {
        PluginRegistry::validate_plugin(plugin_name).unwrap_or(false)
    }

    fn is_plugin_library_installed(plugin_name: &str) -> bool {
        if let Ok(plugin_dir) = PluginUtils::get_plugin_directory(plugin_name) {
            let cargo_toml = plugin_dir.join("Cargo.toml");
            let src_lib = plugin_dir.join("src").join("lib.rs");

            if cargo_toml.exists() && src_lib.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[package.metadata.wasm_plugin]")
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

    /// Install a library-only plugin by downloading and building the dynamic library
    fn install_library_plugin(plugin_name: &str, plugin_dir: &Path) -> Result<InstallationResult> {
        println!("📚 Installing library-only plugin: {plugin_name}");

        let mut result = InstallationResult::new(plugin_name);

        // Download the crate source
        println!("📥 Downloading {plugin_name} from crates.io...");
        let _output = std::process::Command::new("cargo")
            .args([
                "install",
                plugin_name,
                "--root",
                &plugin_dir.to_string_lossy(),
                "--no-track",
            ])
            .output();

        // Even if cargo install fails for library, we can try building manually
        let temp_download = std::env::temp_dir().join(format!("{plugin_name}_download"));

        // Clean up if it exists from a previous run
        if temp_download.exists() {
            std::fs::remove_dir_all(&temp_download).ok();
        }

        std::fs::create_dir_all(&temp_download)
            .map_err(|e| WasmrunError::from(format!("Failed to create temp directory: {e}")))?;

        // Download crate metadata to get the latest version
        let version = SystemUtils::get_latest_crates_version(plugin_name)
            .unwrap_or_else(|| "latest".to_string());

        println!("📦 Fetching {plugin_name} v{version} source...");

        // Use cargo to download the source
        let download_output = std::process::Command::new("cargo")
            .current_dir(&temp_download)
            .args(["init", "--lib", "--name", plugin_name])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to init temp project: {e}")))?;

        if !download_output.status.success() {
            let stderr = String::from_utf8_lossy(&download_output.stderr);
            return Err(WasmrunError::from(format!(
                "Failed to initialize download directory: {stderr}"
            )));
        }

        // Add the plugin as a dependency with all features
        let cargo_toml_path = temp_download.join("Cargo.toml");
        let mut cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)
            .map_err(|e| WasmrunError::from(format!("Failed to read Cargo.toml: {e}")))?;

        // Check if [dependencies] section exists, if not add it
        if cargo_toml_content.contains("[dependencies]") {
            // Append to existing dependencies section
            cargo_toml_content.push_str(&format!(
                "{plugin_name} = {{ version = \"{version}\", features = [\"wasm-plugin\"] }}\n"
            ));
        } else {
            cargo_toml_content.push_str(&format!(
                "\n[dependencies]\n{plugin_name} = {{ version = \"{version}\", features = [\"wasm-plugin\"] }}\n"
            ));
        }

        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| WasmrunError::from(format!("Failed to write Cargo.toml: {e}")))?;

        // Download dependencies
        println!("⬇️  Downloading dependencies...");
        let fetch_output = std::process::Command::new("cargo")
            .current_dir(&temp_download)
            .args(["fetch"])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to fetch dependencies: {e}")))?;

        if !fetch_output.status.success() {
            let stderr = String::from_utf8_lossy(&fetch_output.stderr);
            return Err(WasmrunError::from(format!("Failed to fetch: {stderr}")));
        }

        // Now copy the downloaded source to plugin directory
        println!("🔨 Building {plugin_name} as dynamic library...");

        // Build the plugin as a cdylib in the plugin directory
        std::fs::create_dir_all(plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        // Create a build project in the plugin directory
        let build_manifest = format!(
            r#"[package]
name = "{plugin_name}"
version = "{version}"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
{plugin_name} = {{ version = "{version}", features = ["wasm-plugin"] }}
"#
        );

        let cargo_toml = plugin_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml, build_manifest)
            .map_err(|e| WasmrunError::from(format!("Failed to write Cargo.toml: {e}")))?;

        let src_dir = plugin_dir.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create src directory: {e}")))?;

        // Create a simple re-export lib.rs
        let lib_rs = src_dir.join("lib.rs");
        std::fs::write(&lib_rs, format!("pub use {plugin_name}::*;\n"))
            .map_err(|e| WasmrunError::from(format!("Failed to write lib.rs: {e}")))?;

        // Build the dynamic library
        let build_output = std::process::Command::new("cargo")
            .current_dir(plugin_dir)
            .args(["build", "--release", "--lib"])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to build plugin: {e}")))?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            return Err(WasmrunError::from(format!("Build failed: {stderr}")));
        }

        // Check if the .dylib/.so file was created
        let target_dir = plugin_dir.join("target").join("release");
        let lib_extensions = if cfg!(target_os = "macos") {
            vec!["dylib"]
        } else if cfg!(target_os = "windows") {
            vec!["dll"]
        } else {
            vec!["so"]
        };

        let mut lib_found = false;
        for ext in &lib_extensions {
            let lib_path = target_dir.join(format!("lib{plugin_name}.{ext}"));
            if lib_path.exists() {
                println!("✅ Dynamic library built: {}", lib_path.display());
                lib_found = true;
                break;
            }
        }

        if !lib_found {
            return Err(WasmrunError::from(format!(
                "Dynamic library not found after build. Expected lib{}.{{dylib,so,dll}} in {}",
                plugin_name,
                target_dir.display()
            )));
        }

        // Store metadata
        Self::fetch_and_store_plugin_metadata(plugin_name, plugin_dir)?;

        result.version = version;
        result.binary_installed = false;

        // Cleanup temp directory
        let _ = std::fs::remove_dir_all(&temp_download);

        println!(
            "✅ Library plugin {plugin_name} v{} installed successfully",
            result.version
        );

        Ok(result)
    }

    fn install_generic_plugin(plugin_name: &str, plugin_dir: &Path) -> Result<InstallationResult> {
        println!("Installing {plugin_name} plugin via cargo...");

        let mut result = InstallationResult::new(plugin_name);

        std::fs::create_dir_all(plugin_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin directory: {e}")))?;

        let wasmrun_root = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not find home directory"))?
            .join(".wasmrun");

        std::fs::create_dir_all(&wasmrun_root)
            .map_err(|e| WasmrunError::from(format!("Failed to create .wasmrun directory: {e}")))?;

        let output = std::process::Command::new("cargo")
            .args([
                "install",
                plugin_name,
                "--root",
                &wasmrun_root.to_string_lossy(),
                "--features",
                "cli",
            ])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to execute cargo install: {e}")))?;

        if output.status.success() {
            println!("✅ Plugin installed successfully via cargo to ~/.wasmrun/");

            let bin_path = wasmrun_root.join("bin").join(plugin_name);
            if bin_path.exists() {
                println!("📦 Binary found at: {}", bin_path.display());
                result.binary_installed = true;
            } else {
                println!(
                    "⚠️  Binary not found in expected location: {}",
                    bin_path.display()
                );
            }

            result.version = SystemUtils::get_latest_crates_version(plugin_name)
                .unwrap_or_else(|| "unknown".to_string());

            Self::fetch_and_store_plugin_metadata(plugin_name, plugin_dir)?;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check if the failure is because it's a library-only crate
            if stderr.contains("has no binaries") || stderr.contains("only for installing programs")
            {
                println!("📚 Detected library-only plugin, switching to library installation...");
                return Self::install_library_plugin(plugin_name, plugin_dir);
            }

            println!("Direct cargo install failed: {stderr}");
            println!("Setting up as development plugin template...");
            Self::setup_plugin_from_source(plugin_name, plugin_dir)?;

            result.version = "0.1.0".to_string();
            result.binary_installed = false;
        }

        Ok(result)
    }

    fn setup_plugin_from_source(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        println!("Setting up {plugin_name} plugin template...");

        let (extensions, entry_files, dependencies) =
            if let Ok(metadata) = PluginRegistry::get_plugin_metadata(plugin_name) {
                (
                    metadata.extensions,
                    metadata.entry_files,
                    metadata.dependencies.tools,
                )
            } else {
                Self::infer_plugin_details(plugin_name)
            };

        let cargo_toml_content = format!(
            r#"[package]
name = "{plugin_name}"
version = "0.1.0"
edition = "2021"
description = "WebAssembly plugin for wasmrun"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
toml = "0.8"

[package.metadata.wasm_plugin]
name = "{plugin_name}"
extensions = {extensions:?}
entry_files = {entry_files:?}

[package.metadata.wasm_plugin.capabilities]
compile_wasm = true
compile_webapp = false
live_reload = false
optimization = false
custom_targets = []

[package.metadata.wasm_plugin.dependencies]
tools = {dependencies:?}
"#
        );

        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, cargo_toml_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create Cargo.toml: {e}")))?;

        let src_dir = plugin_dir.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| WasmrunError::from(format!("Failed to create src directory: {e}")))?;

        let plugin_name_pascal = Self::to_pascal_case(plugin_name);
        let lib_rs_content = format!(
            r#"// {plugin_name} WebAssembly plugin for wasmrun
use std::path::Path;

pub struct {plugin_name_pascal}Builder;

impl {plugin_name_pascal}Builder {{
    pub fn new() -> Self {{
        Self
    }}

    pub fn build(&self, project_path: &Path, output_path: &Path) -> Result<(), String> {{
        // Basic implementation that delegates to system tools
        let project_path_str = project_path.to_str().ok_or("Invalid project path")?;
        let output_path_str = output_path.to_str().ok_or("Invalid output path")?;
        
        println!("Building {{}} project at: {{}}", "{plugin_name}", project_path_str);
        println!("Output will be written to: {{}}", output_path_str);
        
        // TODO: Implement actual build logic for {plugin_name}
        // This is a basic template - customize for specific language requirements
        
        Err("Build logic not yet implemented for this plugin".to_string())
    }}
}}

#[no_mangle]
pub extern "C" fn create_wasm_builder() -> *mut {plugin_name_pascal}Builder {{
    Box::into_raw(Box::new({plugin_name_pascal}Builder::new()))
}}

#[no_mangle]
pub extern "C" fn can_handle_project(path: *const std::ffi::c_char) -> bool {{
    false
}}
"#,
        );

        let lib_rs_path = src_dir.join("lib.rs");
        std::fs::write(&lib_rs_path, lib_rs_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create lib.rs: {e}")))?;

        println!("📦 Created plugin template");
        println!("⚠️  Note: This plugin template needs implementation to be functional");
        println!(
            "   Edit {}/src/lib.rs to add your compilation logic",
            plugin_dir.display()
        );

        Ok(())
    }

    fn infer_plugin_details(plugin_name: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
        match plugin_name {
            name if name.contains("rust") => (
                vec!["rs".to_string(), "toml".to_string()],
                vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                vec!["cargo".to_string(), "rustc".to_string()],
            ),
            name if name.contains("go") => (
                vec!["go".to_string(), "mod".to_string()],
                vec!["go.mod".to_string(), "main.go".to_string()],
                vec!["tinygo".to_string()],
            ),
            name if name.contains("zig") => (
                vec!["zig".to_string()],
                vec!["build.zig".to_string(), "src/main.zig".to_string()],
                vec!["zig".to_string()],
            ),
            name if name.contains("cpp") || name.contains("cxx") => (
                vec!["cpp".to_string(), "cxx".to_string(), "hpp".to_string()],
                vec!["CMakeLists.txt".to_string(), "Makefile".to_string()],
                vec!["emcc".to_string()],
            ),
            name if name.contains("py") || name.contains("python") => (
                vec!["py".to_string()],
                vec!["main.py".to_string(), "app.py".to_string()],
                vec!["python".to_string(), "py2wasm".to_string()],
            ),
            _ => (
                vec!["wasm".to_string()],
                vec!["main.wasm".to_string()],
                vec![],
            ),
        }
    }

    fn to_pascal_case(s: &str) -> String {
        s.split(['-', '_'])
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect()
    }

    /// Fetch plugin metadata from crates.io and store in plugin directory
    fn fetch_and_store_plugin_metadata(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        // Try to get plugin info from crates.io
        let metadata_result = Self::fetch_plugin_metadata_from_crates_io(plugin_name);

        match metadata_result {
            Ok(metadata) => {
                // Store the metadata in the plugin directory for future use
                let metadata_path = plugin_dir.join(".wasmrun_metadata");
                let metadata_content = format!(
                    r#"name = "{}"
version = "{}"
description = "{}"
author = "{}"
extensions = {:?}
entry_files = {:?}
dependencies = {:?}
"#,
                    metadata.name,
                    metadata.version,
                    metadata.description,
                    metadata.author,
                    metadata.extensions,
                    metadata.entry_files,
                    metadata.dependencies.tools
                );

                std::fs::write(&metadata_path, metadata_content)
                    .map_err(|e| WasmrunError::from(format!("Failed to write metadata: {e}")))?;

                println!("📝 Stored plugin metadata in {}", metadata_path.display());
            }
            Err(e) => {
                println!("⚠️  Could not fetch detailed metadata: {e}");
                // Create basic metadata file
                Self::create_basic_metadata_file(plugin_name, plugin_dir)?;
            }
        }

        Ok(())
    }

    /// Fetch plugin metadata by downloading and parsing Cargo.toml from crates.io
    fn fetch_plugin_metadata_from_crates_io(
        plugin_name: &str,
    ) -> Result<crate::plugin::metadata::PluginMetadata> {
        // For now, using cargo search to get basic info and infer the rest
        // TODO: download and parse the actual Cargo.toml from crates.io
        crate::plugin::metadata::PluginMetadata::from_crates_io(plugin_name)
    }

    /// Create basic metadata file when full metadata isn't available
    fn create_basic_metadata_file(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        let version = SystemUtils::get_latest_crates_version(plugin_name)
            .unwrap_or_else(|| "unknown".to_string());

        let metadata_path = plugin_dir.join(".wasmrun_metadata");
        let (extensions, entry_files, dependencies) = Self::infer_plugin_details(plugin_name);

        let metadata_content = format!(
            r#"name = "{plugin_name}"
version = "{version}"
description = "{plugin_name} WebAssembly plugin"
author = "Unknown"
extensions = {extensions:?}
entry_files = {entry_files:?}
dependencies = {dependencies:?}
"#
        );

        std::fs::write(&metadata_path, metadata_content)
            .map_err(|e| WasmrunError::from(format!("Failed to write basic metadata: {e}")))?;

        Ok(())
    }

    #[allow(dead_code)]
    fn create_plugin_manifest(plugin_name: &str, plugin_dir: &Path) -> Result<()> {
        let manifest_content = format!(
            r#"[plugin]
name = "{plugin_name}"
version = "0.1.0"
description = "WebAssembly plugin for wasmrun"
type = "external"

[build]
command = "cargo"
args = ["build", "--release"]

[install]
method = "cargo"
source = "crates.io"
"#
        );

        let manifest_path = plugin_dir.join("wasmrun.toml");
        std::fs::write(&manifest_path, manifest_content)
            .map_err(|e| WasmrunError::from(format!("Failed to create plugin manifest: {e}")))?;

        Ok(())
    }
}
