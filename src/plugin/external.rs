use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationResult, Result, WasmrunError};
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};
use crate::utils::{PluginUtils, SystemUtils};

#[cfg(not(target_os = "windows"))]
use crate::plugin::bridge::{symbols::*, PluginSymbols};
#[cfg(not(target_os = "windows"))]
use libloading::Library;

pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl ExternalPluginWrapper {
    pub fn new(_plugin_path: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let plugin_name = entry.info.name.clone();

        if !PluginUtils::is_plugin_available(&plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{plugin_name}' not available"
            )));
        }

        #[cfg(not(target_os = "windows"))]
        let library = Self::try_load_library(&plugin_name)?;

        Ok(Self {
            info: entry.info,
            plugin_name,
            #[cfg(not(target_os = "windows"))]
            library,
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn try_load_library(plugin_name: &str) -> Result<Option<Arc<Library>>> {
        let plugin_dir = PluginUtils::get_plugin_directory(plugin_name)?;

        let lib_extensions = ["so", "dylib"];

        for ext in &lib_extensions {
            let path = plugin_dir.join(format!("lib{plugin_name}.{ext}"));
            if path.exists() {
                unsafe {
                    match Library::new(&path) {
                        Ok(library) => {
                            let symbols = PluginSymbols::get_symbol_names(plugin_name);
                            if library
                                .get::<CreateBuilderFn>(symbols.create_builder)
                                .is_ok()
                            {
                                return Ok(Some(Arc::new(library)));
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        Ok(None)
    }

    fn check_project_via_binary(&self, project_path: &str) -> bool {
        match &self.plugin_name as &str {
            "wasmrust" => {
                Path::new(project_path).join("Cargo.toml").exists()
            }
            "wasmgo" => {
                Path::new(project_path).join("go.mod").exists() || self.has_go_files(project_path)
            }
            _ => false,
        }
    }

    fn check_project_via_manifest(&self, project_path: &str) -> bool {
        let path = Path::new(project_path);

        for entry_file in &self.info.entry_files {
            if path.join(entry_file).exists() {
                return true;
            }
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if let Some(ext_str) = ext.to_str() {
                        if self.info.extensions.contains(&ext_str.to_string()) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    fn has_go_files(&self, project_path: &str) -> bool {
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "go" {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        if PluginUtils::is_plugin_available(&self.plugin_name) {
            return self.check_project_via_binary(project_path);
        }
        self.check_project_via_manifest(project_path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder {
            plugin_name: self.plugin_name.clone(),
            #[cfg(not(target_os = "windows"))]
            library: self.library.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ExternalWasmBuilder {
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    library: Option<Arc<Library>>,
}

impl WasmBuilder for ExternalWasmBuilder {
    fn can_handle_project(&self, project_path: &str) -> bool {
        PluginUtils::is_plugin_available(&self.plugin_name)
            && match &self.plugin_name as &str {
                "wasmrust" => Path::new(project_path).join("Cargo.toml").exists(),
                "wasmgo" => {
                    Path::new(project_path).join("go.mod").exists()
                        || self.has_go_files_in_project(project_path)
                }
                _ => false,
            }
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        match &self.plugin_name as &str {
            "wasmrust" => self.build_rust_project(config),
            "wasmgo" => self.build_go_project(config),
            _ => Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!("Unsupported external plugin: {}", self.plugin_name),
            }),
        }
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        match &self.plugin_name as &str {
            "wasmrust" => {
                let output = Command::new("cargo")
                    .args(["clean"])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| {
                        WasmrunError::from(format!("Failed to clean Rust project: {e}"))
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Clean failed: {stderr}")));
                }
            }
            "wasmgo" => {
                let output = Command::new("go")
                    .args(["clean"])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| WasmrunError::from(format!("Failed to clean Go project: {e}")))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WasmrunError::from(format!("Clean failed: {stderr}")));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalWasmBuilder {
            plugin_name: self.plugin_name.clone(),
            #[cfg(not(target_os = "windows"))]
            library: self.library.clone(),
        })
    }

    fn language_name(&self) -> &str {
        &self.plugin_name
    }

    fn entry_file_candidates(&self) -> &[&str] {
        match &self.plugin_name as &str {
            "wasmrust" => &["Cargo.toml", "src/main.rs", "src/lib.rs"],
            "wasmgo" => &["go.mod", "main.go"],
            _ => &[],
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        match &self.plugin_name as &str {
            "wasmrust" => &["rs", "toml"],
            "wasmgo" => &["go", "mod"],
            _ => &[],
        }
    }

    fn check_dependencies(&self) -> Vec<String> {
        PluginUtils::check_plugin_dependencies(&self.plugin_name)
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        if !self.can_handle_project(project_path) {
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!(
                    "Project at '{}' cannot be handled by {} plugin",
                    project_path, self.plugin_name
                ),
            });
        }
        Ok(())
    }
}

impl ExternalWasmBuilder {
    fn build_rust_project(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let project_path = Path::new(&config.project_path);
        let output_dir = Path::new(&config.output_dir);

        std::fs::create_dir_all(output_dir).map_err(|e| {
            crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: format!("Failed to create output directory: {e}"),
            }
        })?;

        // Check if project has wasm-bindgen dependency
        if self.has_wasm_bindgen_dependency(project_path) && SystemUtils::is_tool_available("wasm-pack") {
            self.build_rust_with_wasm_pack(project_path, output_dir)
        } else {
            self.build_rust_with_cargo(project_path, output_dir)
        }
    }

    fn has_wasm_bindgen_dependency(&self, project_path: &Path) -> bool {
        let cargo_toml_path = project_path.join("Cargo.toml");
        SystemUtils::has_wasm_bindgen_dependency(&cargo_toml_path)
    }

    fn build_rust_with_wasm_pack(
        &self,
        project_path: &Path,
        output_dir: &Path,
    ) -> CompilationResult<BuildResult> {
        let output = Command::new("wasm-pack")
            .args([
                "build",
                "--target", "web",
                "--out-dir",
            ])
            .arg(output_dir)
            .current_dir(project_path)
            .output()
            .map_err(|e| crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: format!("Failed to execute wasm-pack: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: format!("wasm-pack build failed: {stderr}"),
            });
        }

        // Find the generated .wasm file - wasm-pack generates files with different naming patterns
        let mut wasm_file = None;
        let mut js_file = None;

        if let Ok(entries) = std::fs::read_dir(output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "wasm" {
                        wasm_file = Some(path);
                    } else if ext == "js" && path.file_name() != Some(std::ffi::OsStr::new("package.js")) {
                        // Skip package.js, look for the main JS file
                        js_file = Some(path);
                    }
                }
            }
        }

        if let Some(wasm_path) = wasm_file {
            Ok(BuildResult {
                wasm_path: wasm_path.to_string_lossy().to_string(),
                js_path: js_file.map(|p| p.to_string_lossy().to_string()),
                additional_files: vec![],
                is_wasm_bindgen: true,
            })
        } else {
            // List all files in output directory for debugging
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(output_dir) {
                for entry in entries.flatten() {
                    files.push(entry.path().to_string_lossy().to_string());
                }
            }
            
            Err(crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: format!(
                    "wasm-pack succeeded but no .wasm file was found. Generated files: {:?}",
                    files
                ),
            })
        }
    }

    fn build_rust_with_cargo(
        &self,
        project_path: &Path,
        output_dir: &Path,
    ) -> CompilationResult<BuildResult> {
        let target_dir = output_dir.join("target");

        let output = Command::new("cargo")
            .args([
                "build",
                "--target", "wasm32-unknown-unknown",
                "--release",
                "--target-dir",
            ])
            .arg(&target_dir)
            .current_dir(project_path)
            .output()
            .map_err(|e| crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: format!("Failed to execute cargo: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: format!("Cargo build failed: {stderr}"),
            });
        }

        let wasm_dir = target_dir.join("wasm32-unknown-unknown").join("release");
        let mut wasm_file = None;

        if let Ok(entries) = std::fs::read_dir(&wasm_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "wasm" {
                        wasm_file = Some(entry.path());
                        break;
                    }
                }
            }
        }

        if let Some(wasm_path) = wasm_file {
            let final_wasm_path = output_dir.join("main.wasm");
            std::fs::copy(&wasm_path, &final_wasm_path).map_err(|e| {
                crate::error::CompilationError::BuildFailed {
                    language: "rust".to_string(),
                    reason: format!("Failed to copy wasm file: {e}"),
                }
            })?;

            Ok(BuildResult {
                wasm_path: final_wasm_path.to_string_lossy().to_string(),
                js_path: None,
                additional_files: vec![],
                is_wasm_bindgen: false,
            })
        } else {
            Err(crate::error::CompilationError::BuildFailed {
                language: "rust".to_string(),
                reason: "Cargo build succeeded but no .wasm file was found".to_string(),
            })
        }
    }

    fn build_go_project(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let project_path = Path::new(&config.project_path);
        let output_dir = Path::new(&config.output_dir);

        std::fs::create_dir_all(output_dir).map_err(|e| {
            crate::error::CompilationError::BuildFailed {
                language: "go".to_string(),
                reason: format!("Failed to create output directory: {e}"),
            }
        })?;

        let wasm_file = output_dir.join("main.wasm");

        let output = Command::new("tinygo")
            .args([
                "build",
                "-target", "wasm",
                "-o",
            ])
            .arg(&wasm_file)
            .arg(".")
            .current_dir(project_path)
            .output()
            .map_err(|e| crate::error::CompilationError::BuildFailed {
                language: "go".to_string(),
                reason: format!("Failed to execute tinygo: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::CompilationError::BuildFailed {
                language: "go".to_string(),
                reason: format!("TinyGo build failed: {stderr}"),
            });
        }

        if !wasm_file.exists() {
            return Err(crate::error::CompilationError::BuildFailed {
                language: "go".to_string(),
                reason: "TinyGo build succeeded but no .wasm file was generated".to_string(),
            });
        }

        Ok(BuildResult {
            wasm_path: wasm_file.to_string_lossy().to_string(),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    fn has_go_files_in_project(&self, project_path: &str) -> bool {
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "go" {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        let wrapper = ExternalPluginWrapper::new(PathBuf::new(), entry.clone())?;
        Ok(Box::new(wrapper))
    }

    pub fn create_wasmrust_entry() -> ExternalPluginEntry {
        let version = detect_wasmrust_version().unwrap_or_else(|| "0.2.1".to_string());

        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmrust".to_string(),
                version: version.clone(),
                description: "Rust to WebAssembly compiler with wasm-bindgen support".to_string(),
                author: "Kumar Anirudha".to_string(),
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: true,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec![
                        "wasm32-unknown-unknown".to_string(),
                        "wasm32-wasi".to_string(),
                    ],
                },
                extensions: vec!["rs".to_string(), "toml".to_string()],
                entry_files: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmrust".to_string(),
                    version: version.clone(),
                }),
                dependencies: vec!["cargo".to_string(), "rustc".to_string()],
            },
            source: PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: version.clone(),
            },
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            install_path: String::new(),
            executable_path: None,
            enabled: true,
        }
    }

    pub fn create_wasmgo_entry() -> ExternalPluginEntry {
        let version = detect_wasmgo_version().unwrap_or_else(|| "0.1.0".to_string());

        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmgo".to_string(),
                version: version.clone(),
                description: "Go to WebAssembly compiler using TinyGo".to_string(),
                author: "Kumar Anirudha".to_string(),
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: false,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec!["wasm".to_string()],
                },
                extensions: vec!["go".to_string(), "mod".to_string()],
                entry_files: vec!["go.mod".to_string(), "main.go".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmgo".to_string(),
                    version: version.clone(),
                }),
                dependencies: vec!["tinygo".to_string()],
            },
            source: PluginSource::CratesIo {
                name: "wasmgo".to_string(),
                version: version.clone(),
            },
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            install_path: String::new(),
            executable_path: None,
            enabled: true,
        }
    }
}

fn detect_wasmrust_version() -> Option<String> {
    if let Ok(plugin_dir) = PluginUtils::get_plugin_directory("wasmrust") {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
            if let Some(version) = SystemUtils::detect_version_from_cargo_toml(&content) {
                return Some(version);
            }
        }
    }

    SystemUtils::get_latest_crates_version("wasmrust")
}

fn detect_wasmgo_version() -> Option<String> {
    if let Ok(plugin_dir) = PluginUtils::get_plugin_directory("wasmgo") {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
            if let Some(version) = SystemUtils::detect_version_from_cargo_toml(&content) {
                return Some(version);
            }
        }
    }

    SystemUtils::get_latest_crates_version("wasmgo")
}
