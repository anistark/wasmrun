use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult, Result};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::Path;

/// Rust WebAssembly plugin (uses cargo + wasm-bindgen)
#[derive(Clone)]
pub struct RustPlugin {
    info: PluginInfo,
}

impl RustPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "rust".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Rust WebAssembly compiler using cargo and wasm-bindgen".to_string(),
            author: "Wasmrun Team".to_string(),
            extensions: vec!["rs".to_string(), "toml".to_string()],
            entry_files: vec!["Cargo.toml".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: false,
                optimization: true,
                custom_targets: vec!["wasm32-unknown-unknown".to_string()],
                supported_languages: Some(vec!["rust".to_string()]),
            },
        };

        Self { info }
    }

    fn is_rust_project(project_path: &str) -> bool {
        Path::new(project_path).join("Cargo.toml").exists()
    }

    fn read_package_name(project_path: &str) -> Option<String> {
        let cargo_toml = Path::new(project_path).join("Cargo.toml");
        let content = fs::read_to_string(cargo_toml).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("name") && line.contains('=') {
                if let Some(val) = line.splitn(2, '=').nth(1) {
                    let name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !name.is_empty() {
                        return Some(name.replace('-', "_"));
                    }
                }
            }
        }
        None
    }

    fn has_cdylib(project_path: &str) -> bool {
        let cargo_toml = Path::new(project_path).join("Cargo.toml");
        if let Ok(content) = fs::read_to_string(cargo_toml) {
            return content.contains("cdylib");
        }
        false
    }
}

impl Plugin for RustPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        Self::is_rust_project(project_path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(RustPlugin::new())
    }
}

impl WasmBuilder for RustPlugin {
    fn supported_extensions(&self) -> &[&str] {
        &["rs", "toml"]
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["Cargo.toml", "src/lib.rs", "src/main.rs"]
    }

    fn language_name(&self) -> &str {
        "Rust"
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if !CommandExecutor::is_tool_installed("cargo") {
            missing.push("cargo (install from https://rustup.rs)".to_string());
        }
        if !CommandExecutor::is_tool_installed("wasm-bindgen") {
            missing.push(
                "wasm-bindgen (install with: cargo install wasm-bindgen-cli)".to_string(),
            );
        }
        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        PathResolver::validate_directory_exists(project_path).map_err(|e| {
            CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("Project directory validation failed: {e}"),
            }
        })?;

        if !Self::is_rust_project(project_path) {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: "No Cargo.toml found".to_string(),
            });
        }

        Ok(())
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        Self::is_rust_project(project_path)
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        if !CommandExecutor::is_tool_installed("cargo") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "cargo".to_string(),
                language: self.language_name().to_string(),
            });
        }

        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        if config.verbose {
            println!("🔨 Building Rust project for wasm32-unknown-unknown...");
        }

        let cargo_args = [
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
        ];

        let build_output = CommandExecutor::execute_command(
            "cargo",
            &cargo_args,
            &config.project_path,
            config.verbose,
        )?;

        if !build_output.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "cargo build failed: {}",
                    String::from_utf8_lossy(&build_output.stderr)
                ),
            });
        }

        let pkg_name = Self::read_package_name(&config.project_path).unwrap_or_else(|| "output".to_string());
        let wasm_file = Path::new(&config.project_path)
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("{pkg_name}.wasm"));

        if !wasm_file.exists() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Expected wasm file not found: {}", wasm_file.display()),
            });
        }

        // Use wasm-bindgen if the project uses it (has cdylib crate-type)
        if Self::has_cdylib(&config.project_path)
            && CommandExecutor::is_tool_installed("wasm-bindgen")
        {
            if config.verbose {
                println!("🔗 Running wasm-bindgen...");
            }

            let bindgen_output = CommandExecutor::execute_command(
                "wasm-bindgen",
                &[
                    "--out-dir",
                    &config.output_dir,
                    "--target",
                    "web",
                    "--no-typescript",
                    wasm_file.to_str().unwrap_or_default(),
                ],
                &config.project_path,
                config.verbose,
            )?;

            if !bindgen_output.status.success() {
                return Err(CompilationError::BuildFailed {
                    language: self.language_name().to_string(),
                    reason: format!(
                        "wasm-bindgen failed: {}",
                        String::from_utf8_lossy(&bindgen_output.stderr)
                    ),
                });
            }

            // Find the generated _bg.wasm file
            let bg_wasm = Path::new(&config.output_dir).join(format!("{pkg_name}_bg.wasm"));
            let js_file = Path::new(&config.output_dir).join(format!("{pkg_name}.js"));

            if bg_wasm.exists() {
                return Ok(BuildResult {
                    wasm_path: bg_wasm.to_string_lossy().to_string(),
                    js_path: if js_file.exists() {
                        Some(js_file.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    additional_files: vec![],
                    is_wasm_bindgen: true,
                });
            }
        }

        // Copy plain wasm to output dir
        let output_wasm = Path::new(&config.output_dir).join(format!("{pkg_name}.wasm"));
        fs::copy(&wasm_file, &output_wasm).map_err(|e| CompilationError::BuildFailed {
            language: self.language_name().to_string(),
            reason: format!("Failed to copy wasm file: {e}"),
        })?;

        Ok(BuildResult {
            wasm_path: output_wasm.to_string_lossy().to_string(),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        let target_dir = Path::new(project_path).join("target");
        if target_dir.exists() {
            let _ = CommandExecutor::execute_command("cargo", &["clean"], project_path, false);
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }
}

impl Default for RustPlugin {
    fn default() -> Self {
        Self::new()
    }
}
