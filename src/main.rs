mod cli;
mod commands;
mod compiler;
mod server;
mod template;
mod ui;
mod utils;
mod verify;
mod watcher;

use cli::Commands;
use ui::{print_error, print_webapp_detected};

fn main() {
    // Parse command line arguments (without validation here)
    let args = cli::get_args();

    // Handle commands
    let result = match &args.command {
        Some(Commands::Stop) => commands::handle_stop_command(),

        Some(Commands::Compile {
            path,
            positional_path,
            output,
            verbose,
            optimization,
        }) => {
            commands::handle_compile_command(path, positional_path, output, *verbose, optimization)
        }

        Some(Commands::Verify {
            path,
            positional_path,
            detailed,
        }) => commands::handle_verify_command(path, positional_path, *detailed),

        Some(Commands::Inspect {
            path,
            positional_path,
        }) => commands::handle_inspect_command(path, positional_path),

        Some(Commands::Run {
            path,
            positional_path,
            port,
            language,
            watch,
            verbose: _verbose,
        }) => commands::handle_run_command(path, positional_path, *port, language, *watch),

        Some(Commands::Init {
            name,
            template,
            directory,
        }) => commands::handle_init_command(name, template, directory),

        Some(Commands::Clean {
            path,
            positional_path,
        }) => commands::handle_clean_command(path, positional_path),

        None => {
            // For the default command, we need to resolve and validate args
            match cli::ResolvedArgs::from_args(args) {
                Ok(resolved_args) => handle_default_command(&resolved_args),
                Err(e) => {
                    print_error(&format!("Invalid arguments: {}", e));
                    std::process::exit(1);
                }
            }
        }
    };

    // Handle result
    if let Err(e) = result {
        print_error(&e);
        std::process::exit(1);
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
            print_webapp_detected(3000);

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
