//! Run command implementation

use crate::compiler::builder::{BuildConfig, OptimizationLevel, TargetType};
use crate::compiler::{compile_for_execution, detect_project_language};
use crate::error::{Result, WasmrunError};
use crate::plugin::manager::PluginManager;
use crate::utils::PathResolver;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub fn handle_run_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    port: u16,
    language: &Option<String>,
    watch: bool,
) -> Result<()> {
    let resolved_path =
        crate::utils::PathResolver::resolve_input_path(positional_path.clone(), path.clone());

    run_project(resolved_path, Some(port), watch, language.clone(), false)
}

pub fn run_project(
    path: String,
    port: Option<u16>,
    watch: bool,
    language: Option<String>,
    verbose: bool,
) -> Result<()> {
    let resolved_path = PathResolver::resolve_input_path(Some(path.clone()), None);

    if verbose {
        println!("🔍 Analyzing path: {resolved_path}");
    }

    if is_wasm_file(&resolved_path) {
        return run_wasm_file(&resolved_path, port, verbose);
    }

    if Path::new(&resolved_path).is_dir() {
        return run_project_directory(&resolved_path, port, watch, language, verbose);
    }

    Err(WasmrunError::from(format!(
        "Invalid path: {path}. Expected a .wasm file or project directory."
    )))
}

fn is_wasm_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase() == "wasm")
        .unwrap_or(false)
}

fn run_wasm_file(wasm_path: &str, port: Option<u16>, verbose: bool) -> Result<()> {
    if verbose {
        println!("🎯 Running WASM file: {wasm_path}");
    }

    // Create a simple server config
    let server_port = port.unwrap_or(8420);

    if verbose {
        println!("🚀 Starting server on port {server_port}");
    }

    // For now, just indicate success - actual server implementation would go here
    println!("✅ Server would start on port {server_port} for file: {wasm_path}");
    Ok(())
}

fn run_project_directory(
    project_path: &str,
    port: Option<u16>,
    watch: bool,
    language: Option<String>,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("🔍 Detecting project type in: {project_path}");
    }

    // Try plugin-based compilation first
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
            return run_with_plugin(
                &plugin_manager,
                plugin.info().name.clone(),
                project_path,
                port,
                watch,
                verbose,
            );
        }
    }

    // Fall back to legacy language detection
    if verbose {
        println!("🔄 No plugin found, using legacy detection...");
    }

    let detected_language = detect_project_language(project_path);

    if let Some(lang) = language {
        if verbose {
            println!("🎯 Using specified language: {lang}");
        }
        run_with_language_override(project_path, &lang, port, watch, verbose)
    } else {
        if verbose {
            println!("🎯 Detected language: {detected_language:?}");
        }
        run_with_detected_language(project_path, port, watch, verbose)
    }
}

fn run_with_plugin(
    plugin_manager: &PluginManager,
    plugin_name: String,
    project_path: &str,
    port: Option<u16>,
    watch: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("🔌 Using plugin: {plugin_name}");
    }

    let builder = plugin_manager
        .get_builder_for_project(project_path)
        .ok_or_else(|| WasmrunError::from("Failed to get builder for project"))?;

    // Check dependencies
    let missing_deps = builder.check_dependencies();
    if !missing_deps.is_empty() {
        return Err(WasmrunError::from(format!(
            "Missing dependencies for {}: {}",
            plugin_name,
            missing_deps.join(", ")
        )));
    }

    let temp_dir = std::env::temp_dir().join("wasmrun");
    std::fs::create_dir_all(&temp_dir)?;
    let output_dir = temp_dir.to_string_lossy().to_string();

    if watch {
        run_with_watch(project_path, &output_dir, port, builder, verbose)
    } else {
        run_once(project_path, &output_dir, port, builder, verbose)
    }
}

fn run_with_language_override(
    project_path: &str,
    language: &str,
    port: Option<u16>,
    watch: bool,
    verbose: bool,
) -> Result<()> {
    if let Ok(plugin_manager) = PluginManager::new() {
        if let Some(plugin) = plugin_manager.get_plugin_by_language(language) {
            return run_with_plugin(
                &plugin_manager,
                plugin.info().name.clone(),
                project_path,
                port,
                watch,
                verbose,
            );
        }
    }

    if verbose {
        println!("🔄 Plugin not found for language '{language}', using legacy detection");
    }

    run_with_detected_language(project_path, port, watch, verbose)
}

fn run_with_detected_language(
    project_path: &str,
    port: Option<u16>,
    watch: bool,
    verbose: bool,
) -> Result<()> {
    let temp_dir = std::env::temp_dir().join("wasmrun");
    std::fs::create_dir_all(&temp_dir)?;
    let output_dir = temp_dir.to_string_lossy().to_string();

    if watch {
        run_with_watch_legacy(project_path, &output_dir, port, verbose)
    } else {
        run_once_legacy(project_path, &output_dir, port, verbose)
    }
}

fn run_once(
    project_path: &str,
    output_dir: &str,
    port: Option<u16>,
    builder: Box<dyn crate::compiler::builder::WasmBuilder>,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("🔧 Building project...");
    }

    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        optimization_level: OptimizationLevel::Release,
        verbose,
        watch: false,
        target_type: TargetType::Standard,
    };

    let result = builder.build(&config).map_err(WasmrunError::Compilation)?;

    if verbose {
        println!("✅ Build completed");
        println!("🚀 Starting server...");
    }

    let server_port = port.unwrap_or(8420);
    let primary_file = result.js_path.as_ref().unwrap_or(&result.wasm_path);

    println!("✅ Server would start on port {server_port} for file: {primary_file}");
    Ok(())
}

fn run_with_watch(
    project_path: &str,
    output_dir: &str,
    port: Option<u16>,
    builder: Box<dyn crate::compiler::builder::WasmBuilder>,
    verbose: bool,
) -> Result<()> {
    println!("👀 Watch mode enabled - monitoring for changes...");

    let server_port = port.unwrap_or(8420);

    // Initial build
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        optimization_level: OptimizationLevel::Release,
        verbose,
        watch: true,
        target_type: TargetType::Standard,
    };

    let initial_result = builder.build(&config).map_err(WasmrunError::Compilation)?;
    let primary_file = initial_result
        .js_path
        .as_ref()
        .unwrap_or(&initial_result.wasm_path);

    println!("✅ Initial build completed");
    println!("🚀 Server would start on port {server_port} for file: {primary_file}");
    println!("👀 Watching for changes... (press Ctrl+C to stop)");

    // Simple watch loop simulation
    loop {
        thread::sleep(Duration::from_secs(1));
        // In a real implementation, you'd use a file watcher here
        // For now, just keep the loop running
    }
}

fn run_once_legacy(
    project_path: &str,
    output_dir: &str,
    port: Option<u16>,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("🔧 Compiling project (legacy mode)...");
    }

    let primary_file = compile_for_execution(project_path, output_dir)?;

    if verbose {
        println!("✅ Compilation completed");
        println!("🚀 Starting server...");
    }

    let server_port = port.unwrap_or(8420);
    println!("✅ Server would start on port {server_port} for file: {primary_file}");
    Ok(())
}

fn run_with_watch_legacy(
    project_path: &str,
    output_dir: &str,
    port: Option<u16>,
    _verbose: bool,
) -> Result<()> {
    println!("👀 Watch mode enabled (legacy) - monitoring for changes...");

    let server_port = port.unwrap_or(8420);

    // Initial compilation
    let initial_file = compile_for_execution(project_path, output_dir)?;

    println!("✅ Initial compilation completed");
    println!("🚀 Server would start on port {server_port} for file: {initial_file}");
    println!("👀 Watching for changes... (press Ctrl+C to stop)");

    // Simple watch loop simulation
    loop {
        thread::sleep(Duration::from_secs(1));
        // In a real implementation, you'd use a file watcher here
    }
}
