//! [Server Mode] `wasmrun run`: build a project, or take a `.wasm`, and serve it

use crate::compiler::builder::{BuildConfig, OptimizationLevel, TargetType, WasmBuilder};
use crate::compiler::compile_for_execution;
use crate::error::{Result, WasmrunError};
use crate::plugin::manager::PluginManager;
use crate::server::dev::{self, Artifacts, DevConfig, Rebuild};
use crate::utils::PathResolver;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct RunOptions {
    pub port: u16,
    pub app_port: Option<u16>,
    pub language: Option<String>,
    pub watch: bool,
    pub verbose: bool,
    pub serve: bool,
}

pub fn handle_run_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    options: RunOptions,
) -> Result<()> {
    let resolved_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());

    if options.verbose {
        println!("🔍 Analyzing path: {resolved_path}");
    }

    if is_wasm_file(&resolved_path) {
        return run_wasm_file(&resolved_path, &options);
    }

    if Path::new(&resolved_path).is_dir() {
        return run_project_directory(&resolved_path, &options);
    }

    Err(WasmrunError::from(format!(
        "Invalid path: {resolved_path}. Expected a .wasm file or project directory."
    )))
}

fn is_wasm_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase() == "wasm")
        .unwrap_or(false)
}

fn dev_config(project_path: Option<&str>, options: &RunOptions) -> DevConfig {
    DevConfig {
        project_path: project_path.map(str::to_string),
        ui_port: options.port,
        app_port: options.app_port,
        watch: options.watch,
        open_browser: options.serve,
    }
}

fn run_wasm_file(wasm_path: &str, options: &RunOptions) -> Result<()> {
    let path = Path::new(wasm_path);
    if !path.is_file() {
        return Err(WasmrunError::path(format!(
            "WASM file not found: {wasm_path}"
        )));
    }
    if options.watch {
        println!("ℹ️  --watch applies to project directories; serving {wasm_path} as it is");
    }
    dev::run(
        dev_config(None, options),
        Artifacts::from_wasm_file(path),
        None,
    )
}

fn run_project_directory(project_path: &str, options: &RunOptions) -> Result<()> {
    let output_dir = output_dir_for(project_path)?;

    match find_builder(project_path, options)? {
        Some(builder) => run_with_builder(project_path, output_dir, builder, options),
        None => {
            if options.verbose {
                println!("🔄 No plugin found, using legacy detection...");
            }
            run_legacy(project_path, output_dir, options)
        }
    }
}

/// `--language` wins over what the project looks like
fn find_builder(project_path: &str, options: &RunOptions) -> Result<Option<Box<dyn WasmBuilder>>> {
    let Ok(plugin_manager) = PluginManager::new() else {
        return Ok(None);
    };
    let plugin = match &options.language {
        Some(language) => plugin_manager.get_plugin_by_language(language),
        None => plugin_manager.find_plugin_for_project(project_path),
    };
    let Some(plugin) = plugin else {
        return Ok(None);
    };
    let name = plugin.info().name.clone();
    if options.verbose {
        println!("🔌 Using plugin: {name}");
    }

    let builder = plugin.get_builder();
    let missing_deps = builder.check_dependencies();
    if !missing_deps.is_empty() {
        return Err(WasmrunError::from(format!(
            "Missing dependencies for {name}: {}",
            missing_deps.join(", ")
        )));
    }
    Ok(Some(builder))
}

fn run_with_builder(
    project_path: &str,
    output_dir: PathBuf,
    builder: Box<dyn WasmBuilder>,
    options: &RunOptions,
) -> Result<()> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        optimization_level: OptimizationLevel::Release,
        verbose: options.verbose,
        watch: options.watch,
        target_type: TargetType::Standard,
    };

    println!("🔧 Building {project_path}...");
    let initial = builder.build(&config).map_err(WasmrunError::Compilation)?;

    let rebuild: Rebuild = Box::new(move || {
        builder
            .build(&config)
            .map(|result| Artifacts::from_build(&result))
            .map_err(|e| e.to_string())
    });

    dev::run(
        dev_config(Some(project_path), options),
        Artifacts::from_build(&initial),
        Some(rebuild),
    )
}

fn run_legacy(project_path: &str, output_dir: PathBuf, options: &RunOptions) -> Result<()> {
    let output_dir = output_dir.to_string_lossy().to_string();
    let initial = compile_for_execution(project_path, &output_dir)?;

    let project = project_path.to_string();
    let rebuild: Rebuild = Box::new(move || {
        compile_for_execution(&project, &output_dir)
            .map(|file| artifacts_from_primary(&file))
            .map_err(|e| e.to_string())
    });

    dev::run(
        dev_config(Some(project_path), options),
        artifacts_from_primary(&initial),
        Some(rebuild),
    )
}

/// The legacy compiler returns the JS glue when there is one, otherwise the `.wasm`
fn artifacts_from_primary(file: &str) -> Artifacts {
    let path = PathBuf::from(file);
    if path.extension().is_some_and(|e| e == "js") {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let bindgen = path.with_file_name(format!("{stem}_bg.wasm"));
        let wasm_path = if bindgen.is_file() {
            bindgen
        } else {
            path.with_extension("wasm")
        };
        Artifacts {
            wasm_path,
            js_path: Some(path),
        }
    } else {
        Artifacts {
            wasm_path: path,
            js_path: None,
        }
    }
}

/// Each project builds into its own directory, so one project's leftovers are never served for another
fn output_dir_for(project_path: &str) -> Result<PathBuf> {
    let canonical = Path::new(project_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(project_path));
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    let name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let dir = std::env::temp_dir()
        .join("wasmrun")
        .join(format!("{name}-{:08x}", hasher.finish() as u32));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_output_dir_is_per_project() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let dir_a = output_dir_for(a.path().to_str().unwrap()).unwrap();
        let dir_b = output_dir_for(b.path().to_str().unwrap()).unwrap();
        assert_ne!(dir_a, dir_b);
        assert_eq!(dir_a, output_dir_for(a.path().to_str().unwrap()).unwrap());
        assert!(dir_a.is_dir());
    }

    #[test]
    fn test_artifacts_from_primary() {
        let dir = tempdir().unwrap();
        let js = dir.path().join("app.js");
        std::fs::write(dir.path().join("app_bg.wasm"), b"\0asm").unwrap();

        let a = artifacts_from_primary(js.to_str().unwrap());
        assert_eq!(a.wasm_filename(), "app_bg.wasm");
        assert_eq!(a.js_filename().as_deref(), Some("app.js"));

        let plain = artifacts_from_primary(dir.path().join("x.wasm").to_str().unwrap());
        assert!(plain.js_path.is_none());
    }
}
