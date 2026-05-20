use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult, Result};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::{Path, PathBuf};

/// AssemblyScript WebAssembly plugin
#[derive(Clone)]
pub struct AscPlugin {
    info: PluginInfo,
}

impl AscPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "asc".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "AssemblyScript WebAssembly compiler".to_string(),
            author: "Wasmrun Team".to_string(),
            extensions: vec!["ts".to_string(), "json".to_string()],
            entry_files: vec!["asconfig.json".to_string(), "package.json".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: true,
                live_reload: false,
                optimization: true,
                custom_targets: vec!["wasm".to_string(), "web".to_string()],
                supported_languages: Some(vec![
                    "assemblyscript".to_string(),
                    "asc".to_string(),
                ]),
            },
        };

        Self { info }
    }

    fn is_asc_project(project_path: &str) -> bool {
        let path = Path::new(project_path);

        if path.join("asconfig.json").exists() {
            return true;
        }

        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            if content.contains("assemblyscript") {
                return true;
            }
        }

        false
    }

    fn find_output_wasm(project_path: &str) -> Option<PathBuf> {
        let build_dir = Path::new(project_path).join("build");
        if !build_dir.exists() {
            return None;
        }

        let candidates = [
            "optimized.wasm",
            "release.wasm",
            "output.wasm",
            "main.wasm",
        ];
        for candidate in &candidates {
            let p = build_dir.join(candidate);
            if p.exists() {
                return Some(p);
            }
        }

        // Fall back to any .wasm file in build/
        if let Ok(entries) = fs::read_dir(&build_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("wasm") {
                    return Some(p);
                }
            }
        }

        None
    }
}

impl Plugin for AscPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        Self::is_asc_project(project_path)
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(AscPlugin::new())
    }
}

impl WasmBuilder for AscPlugin {
    fn supported_extensions(&self) -> &[&str] {
        &["ts"]
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["asconfig.json", "package.json"]
    }

    fn language_name(&self) -> &str {
        "AssemblyScript"
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if !CommandExecutor::is_tool_installed("asc") && !CommandExecutor::is_tool_installed("npx")
        {
            missing.push("asc (AssemblyScript compiler — install with: npm install -g assemblyscript)".to_string());
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

        if !Self::is_asc_project(project_path) {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: "No asconfig.json or AssemblyScript package.json found".to_string(),
            });
        }

        let assembly_dir = Path::new(project_path).join("assembly");
        if !assembly_dir.exists() {
            return Err(CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: "No assembly/ directory found".to_string(),
            });
        }

        Ok(())
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        Self::is_asc_project(project_path)
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        if config.verbose {
            println!("🔨 Building AssemblyScript project...");
        }

        // Prefer npm run build; fall back to asc directly
        let npm_result = CommandExecutor::execute_command(
            "npm",
            &["run", "build"],
            &config.project_path,
            config.verbose,
        );

        let build_succeeded = match npm_result {
            Ok(output) if output.status.success() => true,
            _ => {
                // Try asc directly
                if config.verbose {
                    println!("⚠️  npm run build failed, trying asc directly...");
                }
                let asc_cmd = if CommandExecutor::is_tool_installed("asc") {
                    "asc"
                } else {
                    "npx"
                };
                let asc_args: Vec<&str> = if asc_cmd == "npx" {
                    vec!["asc", "assembly/index.ts", "--target", "release"]
                } else {
                    vec!["assembly/index.ts", "--target", "release"]
                };
                match CommandExecutor::execute_command(
                    asc_cmd,
                    &asc_args,
                    &config.project_path,
                    config.verbose,
                ) {
                    Ok(output) => output.status.success(),
                    Err(_) => false,
                }
            }
        };

        if !build_succeeded {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "AssemblyScript build failed".to_string(),
            });
        }

        let wasm_path = Self::find_output_wasm(&config.project_path).ok_or_else(|| {
            CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: "No .wasm file found after build in build/ directory".to_string(),
            }
        })?;

        let output_wasm = CommandExecutor::copy_to_output(
            wasm_path.to_str().unwrap_or_default(),
            &config.output_dir,
            "AssemblyScript",
        )?;

        Ok(BuildResult {
            wasm_path: output_wasm,
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        let build_dir = Path::new(project_path).join("build");
        if build_dir.exists() {
            let _ = fs::remove_dir_all(build_dir);
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }
}

impl Default for AscPlugin {
    fn default() -> Self {
        Self::new()
    }
}
