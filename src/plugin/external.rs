//! External plugin loading and management

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compiler::builder::{BuildConfig, BuildResult, OptimizationLevel, WasmBuilder};
use crate::error::{CompilationResult, Result, WasmrunError};
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};

// External plugin wrapper
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
}

impl ExternalPluginWrapper {
    pub fn new(_plugin_dir: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        let info = entry.info.clone();
        let plugin_name = entry.info.name.clone();

        if !Self::is_plugin_available(&plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not available in PATH",
                plugin_name
            )));
        }

        Ok(Self { info, plugin_name })
    }

    fn is_plugin_available(plugin_name: &str) -> bool {
        Command::new(plugin_name)
            .arg("--version")
            .output()
            .or_else(|_| Command::new(plugin_name).arg("info").output())
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn can_handle_rust_project(&self, project_path: &str) -> bool {
        if self.plugin_name == "wasmrust" || self.info.name == "wasmrust" {
            let cargo_toml = Path::new(project_path).join("Cargo.toml");
            return cargo_toml.exists();
        }
        false
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // TODO: Remove rust-specific handling from here
        if self.can_handle_rust_project(project_path) {
            return true;
        }

        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if self.info.extensions.contains(&ext) {
                        return true;
                    }
                }
            }
        }

        for entry_file in &self.info.entry_files {
            let file_path = Path::new(project_path).join(entry_file);
            if file_path.exists() {
                return true;
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalPluginBuilder {
            plugin_name: self.plugin_name.clone(),
            info: self.info.clone(),
        })
    }
}

pub struct ExternalPluginBuilder {
    plugin_name: String,
    info: PluginInfo,
}

impl WasmBuilder for ExternalPluginBuilder {
    fn language_name(&self) -> &str {
        &self.info.name
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !ExternalPluginWrapper::is_plugin_available(&self.plugin_name) {
            missing.push(format!("{} executable not found in PATH", self.plugin_name));
            missing.push(format!(
                "Install with: wasmrun plugin install {}",
                self.plugin_name
            ));
            return missing;
        }

        missing
    }

    fn entry_file_candidates(&self) -> &[&str] {
        static RUST_ENTRIES: &[&str] = &["main.rs", "lib.rs"];
        static EMPTY: &[&str] = &[];

        if self.plugin_name == "wasmrust" {
            RUST_ENTRIES
        } else {
            EMPTY
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        static RUST_EXTENSIONS: &[&str] = &["rs"];
        static EMPTY: &[&str] = &[];

        if self.plugin_name == "wasmrust" {
            RUST_EXTENSIONS
        } else {
            EMPTY
        }
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        let path = Path::new(project_path);
        if !path.exists() || !path.is_dir() {
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!("Project path not found: {}", project_path),
            });
        }

        if self.plugin_name == "wasmrust" {
            let cargo_toml = path.join("Cargo.toml");
            if !cargo_toml.exists() {
                return Err(crate::error::CompilationError::BuildFailed {
                    language: self.plugin_name.clone(),
                    reason: "Cargo.toml not found".to_string(),
                });
            }
        }

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        if self.plugin_name == "wasmrust" {
            self.build_with_wasmrust(config)
        } else {
            Err(crate::error::CompilationError::UnsupportedLanguage {
                language: format!("External plugin '{}' not implemented", self.plugin_name),
            })
        }
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!(
            "🔌 Building with external plugin: {} v{}",
            self.plugin_name, self.info.version
        );
        self.build(config)
    }
}

impl ExternalPluginBuilder {
    fn build_with_wasmrust(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let optimization = match config.optimization_level {
            OptimizationLevel::Debug => "debug",
            OptimizationLevel::Release => "release",
            OptimizationLevel::Size => "size",
        };

        let mut cmd = Command::new("wasmrust");
        cmd.args(&[
            "run",
            "--project",
            &config.project_path,
            "--output",
            &config.output_dir,
            "--optimization",
            optimization,
        ]);

        if config.verbose {
            cmd.arg("--verbose");
        }

        let output = cmd
            .output()
            .map_err(|e| crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!("Failed to execute wasmrust: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!("wasmrust compilation failed: {}", stderr),
            });
        }

        let entry_point = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if entry_point.is_empty() {
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: "wasmrust returned empty output".to_string(),
            });
        }

        if entry_point.ends_with(".js") {
            // For wasm-bindgen project find the corresponding WASM file
            let wasm_path = if entry_point.contains("_bg.") {
                entry_point.replace(".js", "_bg.wasm")
            } else {
                entry_point.replace(".js", ".wasm")
            };

            Ok(BuildResult {
                wasm_path,
                js_path: Some(entry_point),
                additional_files: Vec::new(),
                is_wasm_bindgen: true,
            })
        } else if entry_point.ends_with(".wasm") {
            Ok(BuildResult {
                wasm_path: entry_point,
                js_path: None,
                additional_files: Vec::new(),
                is_wasm_bindgen: false,
            })
        } else {
            // For directory (web app) - check for index.html
            let entry_path = Path::new(&entry_point);
            if entry_path.is_dir() {
                let index_html = entry_path.join("index.html");
                if index_html.exists() {
                    Ok(BuildResult {
                        wasm_path: entry_point.clone(),
                        js_path: Some(index_html.to_string_lossy().to_string()),
                        additional_files: Vec::new(),
                        is_wasm_bindgen: false,
                    })
                } else {
                    Err(crate::error::CompilationError::BuildFailed {
                        language: self.plugin_name.clone(),
                        reason: "Web app directory missing index.html".to_string(),
                    })
                }
            } else {
                Err(crate::error::CompilationError::BuildFailed {
                    language: self.plugin_name.clone(),
                    reason: format!("Unexpected output from wasmrust: {}", entry_point),
                })
            }
        }
    }
}

// Plugin loader
pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        if !entry.enabled {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is disabled",
                entry.info.name
            )));
        }

        let wrapper = ExternalPluginWrapper::new(PathBuf::new(), entry.clone())?;
        Ok(Box::new(wrapper))
    }

    /// Create a wasmrust plugin entry for registration
    #[allow(dead_code)]
    pub fn create_wasmrust_entry() -> ExternalPluginEntry {
        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmrust".to_string(),
                version: Self::get_wasmrust_version(),
                description: "Rust WebAssembly plugin for Wasmrun".to_string(),
                author: "Kumar Anirudha".to_string(),
                extensions: vec!["rs".to_string()],
                entry_files: vec!["Cargo.toml".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmrust".to_string(),
                    version: "latest".to_string(),
                }),
                dependencies: vec!["cargo".to_string(), "rustc".to_string()],
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: true,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec!["wasm32-unknown-unknown".to_string(), "web".to_string()],
                },
            },
            enabled: true,
            install_path: "wasmrust".to_string(),
            source: PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: "latest".to_string(),
            },
            executable_path: Some("wasmrust".to_string()),
            installed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn get_wasmrust_version() -> String {
        Command::new("wasmrust")
            .arg("--version")
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    version_output
                        .split_whitespace()
                        .nth(1)
                        .map(|v| v.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}
