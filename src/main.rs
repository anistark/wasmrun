mod cli;
mod compiler;
mod server;
mod template;
mod utils;
mod verify;
mod watcher;

use cli::{CommandValidator, Commands};
use compiler::builder::{BuildConfig, BuilderFactory, OptimizationLevel};
use utils::PathResolver;

fn main() {
    // Parse and validate command line arguments
    let args = match cli::get_validated_args() {
        Ok(args) => args,
        Err(e) => {
            print_error(&format!("Invalid arguments: {}", e));
            std::process::exit(1);
        }
    };

    // Handle commands
    let result = match &args.command {
        Some(Commands::Stop) => handle_stop_command(),
        Some(Commands::Compile {
            path,
            positional_path,
            output,
            verbose,
            optimization,
        }) => handle_compile_command(path, positional_path, output, *verbose, optimization),
        Some(Commands::Verify {
            path,
            positional_path,
            detailed,
        }) => handle_verify_command(path, positional_path, *detailed),
        Some(Commands::Inspect {
            path,
            positional_path,
        }) => handle_inspect_command(path, positional_path),
        Some(Commands::Run {
            path,
            positional_path,
            port,
            language,
            watch,
            verbose: _verbose,
        }) => handle_run_command(path, positional_path, *port, language, *watch),
        Some(Commands::Init {
            name,
            template,
            directory,
        }) => handle_init_command(name, template, directory),
        Some(Commands::Clean {
            path,
            positional_path,
        }) => handle_clean_command(path, positional_path),
        None => handle_default_command(&args),
    };

    // Handle result
    if let Err(e) = result {
        print_error(&e);
        std::process::exit(1);
    }
}

/// Handle stop command
fn handle_stop_command() -> Result<(), String> {
    if !server::is_server_running() {
        print_info("No Chakra server is currently running");
        return Ok(());
    }

    print_status("Stopping Chakra server...");

    match server::stop_existing_server() {
        Ok(()) => {
            print_success("Chakra Server Stopped", "Server terminated successfully");
            Ok(())
        }
        Err(e) => Err(format!("Error stopping server: {}", e)),
    }
}

/// Handle compile command
fn handle_compile_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    output: &Option<String>,
    verbose: bool,
    optimization: &str,
) -> Result<(), String> {
    let (project_path, output_dir) =
        CommandValidator::validate_compile_args(path, positional_path, output)?;

    // Parse optimization level
    let optimization_level = match optimization.to_lowercase().as_str() {
        "debug" => OptimizationLevel::Debug,
        "release" => OptimizationLevel::Release,
        "size" => OptimizationLevel::Size,
        _ => {
            return Err(format!(
                "Invalid optimization level '{}'. Valid options: debug, release, size",
                optimization
            ))
        }
    };

    // Detect project language and get system info
    let language = compiler::detect_project_language(&project_path);

    if verbose {
        compiler::print_system_info();
    }

    print_compile_info(
        &project_path,
        &language,
        &output_dir,
        &optimization_level,
        verbose,
    );

    // Check for missing tools
    let builder = BuilderFactory::create_builder(&language);
    let missing_tools = builder.check_dependencies();

    if !missing_tools.is_empty() {
        print_missing_tools(&missing_tools);
        return Err("Missing required tools for compilation".to_string());
    }

    // Create build configuration
    let config = BuildConfig {
        project_path,
        output_dir: output_dir.clone(),
        verbose,
        optimization_level,
        target_type: compiler::builder::TargetType::Standard,
    };

    // Compile WASM
    let result = if verbose {
        builder.build_verbose(&config)?
    } else {
        builder.build(&config)?
    };

    print_compilation_success(&result.wasm_path, &result.js_path, &result.additional_files);
    Ok(())
}

/// Handle verify command
fn handle_verify_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    detailed: bool,
) -> Result<(), String> {
    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    println!("🔍 Verifying WebAssembly file: {}", wasm_path);

    match verify::verify_wasm(&wasm_path) {
        Ok(result) => {
            verify::print_verification_results(&wasm_path, &result, detailed);
            Ok(())
        }
        Err(e) => Err(format!("Verification failed: {}", e)),
    }
}

/// Handle inspect command
fn handle_inspect_command(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<(), String> {
    let wasm_path = CommandValidator::validate_verify_args(path, positional_path)?;

    println!("🔍 Inspecting WebAssembly file: {}", wasm_path);

    match verify::print_detailed_binary_info(&wasm_path) {
        Ok(()) => {
            println!("Inspection completed successfully.");
            Ok(())
        }
        Err(e) => Err(format!("Inspection failed: {}", e)),
    }
}

/// Handle run command
fn handle_run_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    port: u16,
    language: &Option<String>,
    watch: bool,
) -> Result<(), String> {
    let (project_path, validated_port) =
        CommandValidator::validate_run_args(path, positional_path, port)?;

    server::run_project(&project_path, validated_port, language.clone(), watch);
    Ok(())
}

/// Handle init command
fn handle_init_command(
    name: &Option<String>,
    template: &str,
    directory: &Option<String>,
) -> Result<(), String> {
    let (project_name, template_name, target_dir) =
        CommandValidator::validate_init_args(name, template, directory)?;

    print_init_info(&project_name, &template_name, &target_dir);

    // TODO: Implement project initialization
    // This would create a new project from a template
    println!(
        "📦 Creating new {} project: {}",
        template_name, project_name
    );
    println!("📂 Target directory: {}", target_dir);

    // For now, return an error since this feature isn't implemented yet
    Err(
        "Project initialization is not yet implemented. This will be added in a future version."
            .to_string(),
    )
}

/// Handle clean command
fn handle_clean_command(
    path: &Option<String>,
    positional_path: &Option<String>,
) -> Result<(), String> {
    let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
    PathResolver::validate_directory_exists(&project_path)?;

    print_clean_info(&project_path);

    let language = compiler::detect_project_language(&project_path);

    // Clean based on project type
    match language {
        compiler::ProjectLanguage::Rust => clean_rust_project(&project_path),
        compiler::ProjectLanguage::Go => clean_go_project(&project_path),
        compiler::ProjectLanguage::C => clean_c_project(&project_path),
        compiler::ProjectLanguage::AssemblyScript => clean_assemblyscript_project(&project_path),
        _ => {
            println!(
                "⚠️ Clean operation not specifically implemented for {:?}",
                language
            );
            println!("💡 You can manually delete build artifacts in your project directory.");
            Ok(())
        }
    }
}

/// Handle default command (no subcommand specified)
fn handle_default_command(args: &cli::ResolvedArgs) -> Result<(), String> {
    if args.wasm {
        server::run_wasm_file(&args.path, args.port);
    } else {
        // Check if it's a Rust web application
        let path_obj = std::path::Path::new(&args.path);
        if path_obj.exists()
            && path_obj.is_dir()
            && compiler::detect_project_language(&args.path) == compiler::ProjectLanguage::Rust
            && compiler::is_rust_web_application(&args.path)
        {
            print_webapp_detected(args.port);

            // Run as a web application on port 3000
            if let Err(e) = server::run_webapp(&args.path, 3000, args.watch) {
                return Err(format!("Error running web application: {}", e));
            }
        } else {
            // Default to compile and run project
            server::run_project(&args.path, args.port, None, args.watch);
        }
    }
    Ok(())
}

// Clean functions for different project types
fn clean_rust_project(project_path: &str) -> Result<(), String> {
    let target_dir = PathResolver::join_paths(project_path, "target");
    let pkg_dir = PathResolver::join_paths(project_path, "pkg");

    let mut cleaned = Vec::new();

    if std::path::Path::new(&target_dir).exists() {
        std::fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("Failed to remove target directory: {}", e))?;
        cleaned.push("target/");
    }

    if std::path::Path::new(&pkg_dir).exists() {
        std::fs::remove_dir_all(&pkg_dir)
            .map_err(|e| format!("Failed to remove pkg directory: {}", e))?;
        cleaned.push("pkg/");
    }

    if cleaned.is_empty() {
        println!("✨ Project is already clean!");
    } else {
        println!("🧹 Cleaned: {}", cleaned.join(", "));
    }

    Ok(())
}

fn clean_go_project(project_path: &str) -> Result<(), String> {
    // Clean Go build cache and binaries
    let output = std::process::Command::new("go")
        .args(["clean", "-cache", "-modcache"])
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("Failed to run go clean: {}", e))?;

    if output.status.success() {
        println!("🧹 Go project cleaned successfully");
    } else {
        println!("⚠️ Go clean completed with warnings");
    }

    Ok(())
}

fn clean_c_project(project_path: &str) -> Result<(), String> {
    // Look for common C build artifacts
    let artifacts = ["*.o", "*.wasm", "*.js", "*.html"];
    let mut cleaned = Vec::new();

    for pattern in artifacts {
        // This is a simplified approach - in a real implementation,
        // you'd want to use glob patterns properly
        if pattern.ends_with(".wasm") {
            let wasm_files = PathResolver::find_files_with_extension(project_path, "wasm")?;
            for file in wasm_files {
                std::fs::remove_file(&file)
                    .map_err(|e| format!("Failed to remove {}: {}", file, e))?;
                cleaned.push(PathResolver::get_filename(&file)?);
            }
        }
    }

    if cleaned.is_empty() {
        println!("✨ No C build artifacts found to clean");
    } else {
        println!("🧹 Cleaned: {}", cleaned.join(", "));
    }

    Ok(())
}

fn clean_assemblyscript_project(project_path: &str) -> Result<(), String> {
    let build_dir = PathResolver::join_paths(project_path, "build");
    let dist_dir = PathResolver::join_paths(project_path, "dist");

    let mut cleaned = Vec::new();

    for dir in [&build_dir, &dist_dir] {
        if std::path::Path::new(dir).exists() {
            std::fs::remove_dir_all(dir)
                .map_err(|e| format!("Failed to remove directory {}: {}", dir, e))?;
            cleaned.push(PathResolver::get_filename(dir)?);
        }
    }

    if cleaned.is_empty() {
        println!("✨ Project is already clean!");
    } else {
        println!("🧹 Cleaned: {}", cleaned.join(", "));
    }

    Ok(())
}

// UI Helper functions
fn print_error(message: &str) {
    eprintln!("\n\x1b[1;34m╭\x1b[0m");
    eprintln!("  ❌ \x1b[1;31m{}\x1b[0m", message);
    eprintln!("\x1b[1;34m╰\x1b[0m");
}

fn print_success(title: &str, message: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅ \x1b[1;36m{}\x1b[0m", title);
    println!();
    println!("  ✅ \x1b[1;32m{}\x1b[0m", message);
    println!("\x1b[1;34m╰\x1b[0m");
}

fn print_info(message: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ℹ️  \x1b[1;34m{}\x1b[0m", message);
    println!("\x1b[1;34m╰\x1b[0m");
}

fn print_status(message: &str) {
    println!("\n⏳ {}", message);
}

fn print_compile_info(
    project_path: &str,
    language: &compiler::ProjectLanguage,
    output_dir: &str,
    optimization: &OptimizationLevel,
    verbose: bool,
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🌀 \x1b[1;36mChakra WASM Compiler\x1b[0m\n");
    println!(
        "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
        project_path
    );
    println!(
        "  🔍 \x1b[1;34mDetected Language:\x1b[0m \x1b[1;32m{:?}\x1b[0m",
        language
    );
    println!(
        "  📤 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{}\x1b[0m",
        output_dir
    );
    println!(
        "  ⚡ \x1b[1;34mOptimization:\x1b[0m \x1b[1;33m{:?}\x1b[0m",
        optimization
    );

    if verbose {
        println!("  🔊 \x1b[1;34mVerbose Mode:\x1b[0m \x1b[1;32mEnabled\x1b[0m");
    }

    println!("\x1b[1;34m╰\x1b[0m\n");
}

fn print_missing_tools(missing_tools: &[String]) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ⚠️  \x1b[1;33mMissing Required Tools:\x1b[0m");
    for tool in missing_tools {
        println!("     \x1b[1;31m• {}\x1b[0m", tool);
    }
    println!("\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");
}

fn print_compilation_success(
    wasm_path: &str,
    js_path: &Option<String>,
    additional_files: &[String],
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅ \x1b[1;36mWASM Compiled Successfully\x1b[0m\n");
    println!(
        "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
        wasm_path
    );

    if let Some(js_file) = js_path {
        println!(
            "  📝 \x1b[1;34mJS File:\x1b[0m \x1b[1;32m{}\x1b[0m",
            js_file
        );
    }

    if !additional_files.is_empty() {
        println!("  📄 \x1b[1;34mAdditional Files:\x1b[0m");
        for file in additional_files {
            println!("     \x1b[1;37m• {}\x1b[0m", file);
        }
    }

    println!("\n  🚀 \x1b[1;33mRun it with:\x1b[0m");
    println!("     \x1b[1;37mchakra --wasm --path {}\x1b[0m", wasm_path);
    println!("\x1b[1;34m╰\x1b[0m");
}

fn print_webapp_detected(port: u16) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🌐 \x1b[1;36mDetected Rust Web Application\x1b[0m");
    println!("  \x1b[0;37mRunning as a web app on port {}\x1b[0m", port);
    println!("\x1b[1;34m╰\x1b[0m\n");
}

fn print_init_info(project_name: &str, template: &str, target_dir: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🚀 \x1b[1;36mInitializing New Chakra Project\x1b[0m\n");
    println!(
        "  📦 \x1b[1;34mProject Name:\x1b[0m \x1b[1;33m{}\x1b[0m",
        project_name
    );
    println!(
        "  🎯 \x1b[1;34mTemplate:\x1b[0m \x1b[1;33m{}\x1b[0m",
        template
    );
    println!(
        "  📂 \x1b[1;34mDirectory:\x1b[0m \x1b[1;33m{}\x1b[0m",
        target_dir
    );
    println!("\x1b[1;34m╰\x1b[0m\n");
}

fn print_clean_info(project_path: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🧹 \x1b[1;36mCleaning Project\x1b[0m\n");
    println!(
        "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
        project_path
    );
    println!("\x1b[1;34m╰\x1b[0m\n");
}
