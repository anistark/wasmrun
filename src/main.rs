mod cli;
mod commands;
mod compiler;
mod error;
mod server;
mod template;
mod ui;
mod utils;
mod verify;
mod watcher;

use cli::Commands;
use error::{ChakraError, Result};
use std::error::Error;
use ui::{print_error, print_webapp_detected};

fn main() {
    // Set up better panic handling
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n🔥 Chakra encountered an unexpected error:");
        eprintln!("{}", panic_info);
        eprintln!("\n💡 This is likely a bug. Please report it at:");
        eprintln!("   https://github.com/anistark/chakra/issues");
        eprintln!("\n📋 Include your command, WASM file, and this error message.");
    }));

    // Parse command line arguments (without validation here)
    let args = cli::get_args();

    // Handle commands and convert errors to proper error types
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
                .map_err(|e| match e {
                    ChakraError::Command(_)
                    | ChakraError::Compilation(_)
                    | ChakraError::Path { .. } => e,
                    _ => e,
                })
        }

        Some(Commands::Verify {
            path,
            positional_path,
            detailed,
        }) => {
            commands::handle_verify_command(path, positional_path, *detailed).map_err(|e| match e {
                ChakraError::Command(_) | ChakraError::Wasm(_) | ChakraError::Path { .. } => e,
                _ => e,
            })
        }

        Some(Commands::Inspect {
            path,
            positional_path,
        }) => commands::handle_inspect_command(path, positional_path).map_err(|e| match e {
            ChakraError::Command(_) | ChakraError::Wasm(_) | ChakraError::Path { .. } => e,
            _ => e,
        }),

        Some(Commands::Run {
            path,
            positional_path,
            port,
            language,
            watch,
            verbose: _verbose,
        }) => commands::handle_run_command(path, positional_path, *port, language, *watch).map_err(
            |e| match e {
                ChakraError::Command(_) | ChakraError::Server(_) | ChakraError::Path { .. } => e,
                _ => e,
            },
        ),

        Some(Commands::Init {
            name,
            template,
            directory,
        }) => commands::handle_init_command(name, template, directory).map_err(|e| match e {
            ChakraError::Command(_) | ChakraError::Path { .. } => e,
            _ => e,
        }),

        Some(Commands::Clean {
            path,
            positional_path,
        }) => commands::handle_clean_command(path, positional_path).map_err(|e| match e {
            ChakraError::Command(_) | ChakraError::Path { .. } => e,
            _ => e,
        }),

        None => {
            // For the default command, we need to resolve and validate args
            match cli::ResolvedArgs::from_args(args) {
                Ok(resolved_args) => handle_default_command(&resolved_args),
                Err(e) => Err(e),
            }
        }
    };

    // Handle result with improved error reporting
    if let Err(error) = result {
        handle_error(error);
        std::process::exit(1);
    }
}

fn handle_default_command(args: &cli::ResolvedArgs) -> Result<()> {
    if args.wasm {
        server::run_wasm_file(&args.path, args.port)?;
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
            server::run_webapp(&args.path, 3000, args.watch)?;
        } else {
            // Default to compile and run project
            server::run_project(&args.path, args.port, None, args.watch)?;
        }
    }
    Ok(())
}

/// Enhanced error handling with user-friendly messages and suggestions
fn handle_error(error: ChakraError) {
    // Print the main error message
    print_error(&error.user_message());

    // Print suggestions if available
    let suggestions = error.suggestions();
    if !suggestions.is_empty() {
        eprintln!("\n💡 Suggestions:");
        for suggestion in suggestions {
            eprintln!("   • {}", suggestion);
        }
    }

    // Print additional context for debugging
    if std::env::var("CHAKRA_DEBUG").is_ok() || std::env::var("RUST_BACKTRACE").is_ok() {
        eprintln!("\n🔍 Debug information:");
        eprintln!("{:?}", error);

        // Walk the error chain
        let mut source = error.source();
        let mut level = 1;
        while let Some(err) = source {
            eprintln!("  {}. Caused by: {}", level, err);
            source = err.source();
            level += 1;
        }
    } else {
        eprintln!("\n🔍 For more details, run with CHAKRA_DEBUG=1 or RUST_BACKTRACE=1");
    }

    // Print recovery information if the error is recoverable
    if error.is_recoverable() {
        eprintln!("\n🔄 This error might be recoverable. Try the suggestions above.");
    }
}
