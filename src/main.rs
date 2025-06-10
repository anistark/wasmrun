mod cli;
mod commands;
mod compiler;
mod error;
mod plugin;
mod server;
mod template;
mod ui;
mod utils;
mod watcher;

use cli::Commands;
use error::{ChakraError, Result};
use std::error::Error;
use ui::print_webapp_detected;

fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n🔥 Chakra encountered an unexpected error:");
        eprintln!("{}", panic_info);
        eprintln!("\n💡 This is likely a bug. Please report it at:");
        eprintln!("   https://github.com/anistark/chakra/issues");
        eprintln!("\n📋 Include your command, WASM file, and this error message.");
    }));

    let args = cli::get_args();

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

        Some(Commands::Plugin(plugin_cmd)) => {
            commands::handle_plugin_command(plugin_cmd).map_err(|e| match e {
                ChakraError::Command(_) | ChakraError::Path { .. } => e,
                _ => e,
            })
        }

        // TODO: WASM project using Chakra
        // Some(Commands::Init {
        //     name,
        //     template,
        //     directory,
        // }) => commands::handle_init_command(name, template, directory).map_err(|e| match e {
        //     ChakraError::Command(_) | ChakraError::Path { .. } => e,
        //     _ => e,
        // }),
        Some(Commands::Clean {
            path,
            positional_path,
        }) => commands::handle_clean_command(path, positional_path).map_err(|e| match e {
            ChakraError::Command(_) | ChakraError::Path { .. } => e,
            _ => e,
        }),

        None => match cli::ResolvedArgs::from_args(args) {
            Ok(resolved_args) => handle_default_command(&resolved_args),
            Err(e) => Err(e),
        },
    };

    if let Err(error) = result {
        handle_error(error);
        std::process::exit(1);
    }
}

fn handle_default_command(args: &cli::ResolvedArgs) -> Result<()> {
    if args.wasm {
        println!("🎯 \x1b[1;34mMode:\x1b[0m Direct WASM execution");
        println!("📦 \x1b[1;34mFile:\x1b[0m {}", args.path);

        std::thread::sleep(std::time::Duration::from_millis(300));

        server::run_wasm_file(&args.path, args.port)?;
    } else {
        let path_obj = std::path::Path::new(&args.path);

        if path_obj.exists() && path_obj.is_dir() {
            let detected_language = compiler::detect_project_language(&args.path);

            if detected_language == compiler::ProjectLanguage::Rust
                && compiler::is_rust_web_application(&args.path)
            {
                print_webapp_detected(3000);

                std::thread::sleep(std::time::Duration::from_millis(300));

                // Run as a web application on port 3000
                // TODO: Make port configurable
                server::run_webapp(&args.path, 3000, args.watch)?;
            } else {
                println!("🎯 \x1b[1;34mMode:\x1b[0m Project compilation and execution");

                let language_icon = match detected_language {
                    compiler::ProjectLanguage::Rust => "🦀",
                    compiler::ProjectLanguage::Go => "🐹",
                    compiler::ProjectLanguage::C => "🔧",
                    compiler::ProjectLanguage::AssemblyScript => "📜",
                    compiler::ProjectLanguage::Python => "🐍",
                    _ => "❓",
                };

                println!(
                    "{} \x1b[1;34mLanguage:\x1b[0m {:?}",
                    language_icon, detected_language
                );

                if args.watch {
                    println!("👀 \x1b[1;34mWatch Mode:\x1b[0m Enabled");
                }

                std::thread::sleep(std::time::Duration::from_millis(300));

                server::run_project(&args.path, args.port, None, args.watch)?;
            }
        } else if path_obj.is_file() {
            println!("🎯 \x1b[1;34mMode:\x1b[0m File execution");

            if let Some(ext) = path_obj.extension() {
                match ext.to_str() {
                    Some("wasm") => {
                        println!("📦 \x1b[1;34mType:\x1b[0m WebAssembly file");
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        server::run_wasm_file(&args.path, args.port)?;
                    }
                    Some("js") => {
                        println!(
                            "📜 \x1b[1;34mType:\x1b[0m JavaScript (checking for WASM bindings)"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        server::run_project(&args.path, args.port, None, args.watch)?;
                    }
                    _ => {
                        println!("❓ \x1b[1;33mUnknown file type, attempting to run...\x1b[0m");
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        server::run_project(&args.path, args.port, None, args.watch)?;
                    }
                }
            }
        } else {
            return Err(ChakraError::path(format!("Path not found: {}", args.path)));
        }
    }
    Ok(())
}

/// Error handling with better context
fn handle_error(error: ChakraError) {
    println!();

    eprintln!(
        "\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m"
    );
    eprintln!("\x1b[1;34m│\x1b[0m  ❌ \x1b[1;31mChakra Error\x1b[0m                                          \x1b[1;34m│\x1b[0m");
    eprintln!(
        "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
    );

    let user_message = error.user_message();
    let lines = wrap_text(&user_message, 61);

    for line in lines {
        eprintln!("\x1b[1;34m│\x1b[0m  {:<61} \x1b[1;34m│\x1b[0m", line);
    }

    let suggestions = error.suggestions();
    if !suggestions.is_empty() {
        eprintln!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );
        eprintln!("\x1b[1;34m│\x1b[0m  💡 \x1b[1;36mSuggestions:\x1b[0m                                        \x1b[1;34m│\x1b[0m");

        for suggestion in suggestions.iter().take(3) {
            let suggestion_lines = wrap_text(&format!("• {}", suggestion), 59);
            for line in suggestion_lines {
                eprintln!("\x1b[1;34m│\x1b[0m    {:<59} \x1b[1;34m│\x1b[0m", line);
            }
        }
    }

    eprintln!(
        "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
    );

    // Additional debug information
    if std::env::var("CHAKRA_DEBUG").is_ok() || std::env::var("RUST_BACKTRACE").is_ok() {
        eprintln!("\n\x1b[1;34m🔍 Debug Information:\x1b[0m");
        eprintln!("\x1b[0;37m{:?}\x1b[0m", error);

        let mut source = error.source();
        let mut level = 1;
        while let Some(err) = source {
            eprintln!(
                "  \x1b[1;34m{}.\x1b[0m Caused by: \x1b[0;37m{}\x1b[0m",
                level, err
            );
            source = err.source();
            level += 1;
        }
    } else {
        eprintln!("\n💡 \x1b[1;34mFor detailed debugging information, run with:\x1b[0m");
        eprintln!("   \x1b[1;37mCHAKRA_DEBUG=1 chakra [your command]\x1b[0m");
        eprintln!("   \x1b[1;37mRUST_BACKTRACE=1 chakra [your command]\x1b[0m");
    }

    if error.is_recoverable() {
        eprintln!(
            "\n🔄 \x1b[1;32mThis error might be recoverable.\x1b[0m Try the suggestions above."
        );
    }

    eprintln!();
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.is_empty() {
        return lines;
    }

    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}
