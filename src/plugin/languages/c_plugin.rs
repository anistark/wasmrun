use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use crate::error::{CompilationError, CompilationResult};
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginType};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct CPlugin {
    info: PluginInfo,
    #[allow(dead_code)]
    builder: Arc<CBuilder>,
}

impl CPlugin {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let builder = Arc::new(CBuilder::new());

        let info = PluginInfo {
            name: "c".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "C/C++ WebAssembly compiler using Emscripten".to_string(),
            author: "Chakra Team".to_string(),
            extensions: vec![
                "c".to_string(),
                "cpp".to_string(),
                "h".to_string(),
                "hpp".to_string(),
            ],
            entry_files: vec!["main.c".to_string(), "Makefile".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec![],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32".to_string()],
            },
        };

        Self { info, builder }
    }
}

impl Plugin for CPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Look for .c or .cpp files
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "c" || ext == "cpp" || ext == "h" || ext == "hpp" {
                        return true;
                    }
                }
            }
        }

        // Check for Makefile
        let makefile_path = PathResolver::join_paths(project_path, "Makefile");
        Path::new(&makefile_path).exists()
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(CBuilder::new())
    }
}

pub struct CBuilder;

impl CBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Find main.c or similar entry point
    fn find_entry_file(&self, project_path: &str) -> CompilationResult<PathBuf> {
        let common_entry_files = [
            "main.c",
            "index.c",
            "app.c",
            "main.cpp",
            "index.cpp",
            "app.cpp",
        ];

        for entry_name in common_entry_files.iter() {
            let entry_path = Path::new(project_path).join(entry_name);
            if entry_path.exists() {
                return Ok(entry_path);
            }
        }

        // If no common entry file found, look for any .c or .cpp file
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "c" || ext == "cpp" {
                        return Ok(entry.path());
                    }
                }
            }
        }

        Err(CompilationError::MissingEntryFile {
            language: self.language_name().to_string(),
            candidates: vec!["main.c".to_string(), "main.cpp".to_string()],
        })
    }
}

impl WasmBuilder for CBuilder {
    fn language_name(&self) -> &str {
        "C"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.c", "index.c", "app.c", "Makefile"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["c", "h"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("emcc") {
            missing.push("emcc (Emscripten - install from https://emscripten.org)".to_string());
        }

        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        PathResolver::validate_directory_exists(project_path).map_err(|e| {
            CompilationError::InvalidProjectStructure {
                language: self.language_name().to_string(),
                reason: format!("Project directory validation failed: {}", e),
            }
        })?;

        // Try to find a C entry file
        let _ = self.find_entry_file(project_path)?;

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        // Check if emcc is installed
        if !CommandExecutor::is_tool_installed("emcc") {
            return Err(CompilationError::BuildToolNotFound {
                tool: "emcc".to_string(),
                language: self.language_name().to_string(),
            });
        }

        // Find entry file
        let entry_path = self.find_entry_file(&config.project_path)?;

        // Create output directory if it doesn't exist
        PathResolver::ensure_output_directory(&config.output_dir).map_err(|_| {
            CompilationError::OutputDirectoryCreationFailed {
                path: config.output_dir.clone(),
            }
        })?;

        // Get the output filename
        let output_name = entry_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string()
            + ".wasm";
        let output_file = Path::new(&config.output_dir).join(&output_name);

        println!("🔨 Building with Emscripten...");

        // Build with emcc
        let optimization_flag = match config.optimization_level {
            crate::compiler::builder::OptimizationLevel::Debug => "-O0",
            crate::compiler::builder::OptimizationLevel::Release => "-O2",
            crate::compiler::builder::OptimizationLevel::Size => "-Oz",
        };

        let build_output = CommandExecutor::execute_command(
            "emcc",
            &[
                entry_path.to_str().unwrap(),
                "-o",
                output_file.to_str().unwrap(),
                optimization_flag,
                "-s",
                "WASM=1",
                "-s",
                "STANDALONE_WASM=1",
            ],
            &config.project_path,
            config.verbose,
        )?;

        if !build_output.status.success() {
            return Err(CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "Build failed: {}",
                    String::from_utf8_lossy(&build_output.stderr)
                ),
            });
        }

        Ok(BuildResult {
            wasm_path: output_file.to_string_lossy().to_string(),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }
}
