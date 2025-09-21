mod cli;
mod commands;
mod compiler;
mod config;
mod debug;
mod error;
mod plugin;
mod server;
mod template;
mod ui;
mod utils;
mod watcher;

// Macros are automatically available from crate root

use crate::compiler::builder::OptimizationLevel;
use crate::utils::PathResolver;
use cli::{get_args, Commands, ResolvedArgs};
use debug::enable_debug;
use error::WasmrunError;
use std::error::Error;

fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n🔥 Wasmrun encountered an unexpected error:");
        eprintln!("{panic_info}");
        eprintln!("\n💡 This is likely a bug. Please report it at:");
        eprintln!("   https://github.com/anistark/wasmrun/issues");
        eprintln!("\n📋 Include your command, WASM file, and this error message.");
    }));

    let args = get_args();

    if args.debug {
        enable_debug();
    }

    debug_enter!("main", "args = {:?}", args);

    let result = match &args.command {
        Some(Commands::Stop) => commands::handle_stop_command(),

        Some(Commands::Compile {
            path,
            positional_path,
            output,
            verbose,
            optimization,
        }) => {
            debug_println!("Processing compile command");
            let project_path =
                PathResolver::resolve_input_path(positional_path.clone(), path.clone());
            let output_dir = output.clone().unwrap_or_else(|| ".".to_string());
            debug_println!(
                "Resolved paths: project={}, output={}",
                project_path,
                output_dir
            );

            let opt_level = match optimization.as_str() {
                "debug" => OptimizationLevel::Debug,
                "size" => OptimizationLevel::Size,
                _ => OptimizationLevel::Release,
            };
            debug_println!("Optimization level: {:?}", opt_level);

            commands::handle_compile_command(project_path, output_dir, opt_level, *verbose)
        }
        .map_err(|e| match e {
            WasmrunError::Command(_) | WasmrunError::Compilation(_) | WasmrunError::Path { .. } => {
                e
            }
            _ => e,
        }),

        Some(Commands::Verify {
            path,
            positional_path,
            detailed,
        }) => {
            debug_println!("Processing verify command with detailed={}", detailed);
            commands::handle_verify_command(path, positional_path, *detailed).map_err(|e| match e {
                WasmrunError::Command(_) | WasmrunError::Wasm(_) | WasmrunError::Path { .. } => e,
                _ => e,
            })
        }

        Some(Commands::Inspect {
            path,
            positional_path,
        }) => commands::handle_inspect_command(path, positional_path).map_err(|e| match e {
            WasmrunError::Command(_) | WasmrunError::Wasm(_) | WasmrunError::Path { .. } => e,
            _ => e,
        }),

        Some(Commands::Run {
            path,
            positional_path,
            port,
            language,
            watch,
            verbose: _verbose,
        }) => {
            debug_println!(
                "Processing run command: port={}, language={:?}, watch={}",
                port,
                language,
                watch
            );
            commands::handle_run_command(path, positional_path, *port, language, *watch).map_err(
                |e| match e {
                    WasmrunError::Command(_)
                    | WasmrunError::Server(_)
                    | WasmrunError::Path { .. } => e,
                    _ => e,
                },
            )
        }

        Some(Commands::Plugin(plugin_cmd)) => {
            commands::run_plugin_command(plugin_cmd).map_err(|e| match e {
                WasmrunError::Command(_) | WasmrunError::Path { .. } => e,
                _ => e,
            })
        }

        Some(Commands::Clean {
            path,
            positional_path,
            all,
        }) => commands::handle_clean_command(&path.clone(), &positional_path.clone(), *all),

        None => {
            debug_println!("No subcommand provided, running default server mode");
            let resolved_args = match ResolvedArgs::from_args(args) {
                Ok(args) => {
                    debug_println!("Resolved args: {:?}", args);
                    args
                }
                Err(e) => {
                    error_println!("{e}");
                    debug_println!("Failed to resolve args: {:?}", e);
                    std::process::exit(1);
                }
            };
            if resolved_args.wasm {
                debug_println!("Running WASM file: {}", resolved_args.path);
                server::run_wasm_file(&resolved_args.path, resolved_args.port)
            } else {
                debug_println!(
                    "Running project: {}, watch: {}",
                    resolved_args.path,
                    resolved_args.watch
                );
                server::run_project(
                    &resolved_args.path,
                    resolved_args.port,
                    None,
                    resolved_args.watch,
                )
            }
        }
    };

    if let Err(e) = result {
        debug_println!("Command execution failed: {:?}", e);
        let mut error_source: &dyn Error = &e;
        eprintln!("❌ {error_source}");

        while let Some(source) = error_source.source() {
            eprintln!("   Caused by: {source}");
            debug_println!("Error chain: {}", source);
            error_source = source;
        }

        debug_exit!("main", "exit code: 1");
        std::process::exit(1);
    }

    debug_exit!("main", "exit code: 0");
}
