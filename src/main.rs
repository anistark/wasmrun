mod cli;
mod compiler;
mod server;
mod template;
mod utils;
mod verify;
mod watcher;

use std::path::Path;

fn main() {
    // Parse command line arguments
    let args = cli::get_args();

    match &args.command {
        // Stop the running Chakra server
        Some(cli::Commands::Stop) => {
            // Check if a server is running before attempting to stop it
            if !server::is_server_running() {
                println!("\n\x1b[1;34m╭\x1b[0m");
                println!("  ℹ️  \x1b[1;34mNo Chakra server is currently running\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m");
                return;
            }

            // Show stopping message
            println!("\n⏳ Stopping Chakra server...");

            // Attempt to stop the existing server
            match server::stop_existing_server() {
                Ok(()) => {
                    // Server stopped successfully
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  🛑 \x1b[1;36mChakra Server Stopped\x1b[0m");
                    println!();
                    println!("  ✅ \x1b[1;32mServer terminated successfully\x1b[0m");
                    println!("\x1b[1;34m╰\x1b[0m");
                }
                Err(e) => {
                    // Could not stop the server
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ❌ \x1b[1;31mError stopping server:\x1b[0m");
                    println!("  \x1b[0;91m{}\x1b[0m", e);
                    println!("\x1b[1;34m╰\x1b[0m");
                }
            }
        }

        // Compile project to WebAssembly
        Some(cli::Commands::Compile {
            path,
            positional_path,
            output,
        }) => {
            // Determine the actual path to use
            let actual_path = positional_path
                .clone()
                .unwrap_or_else(|| path.clone().unwrap_or_else(|| String::from("./")));

            let output_dir = output.clone().unwrap_or_else(|| String::from("."));

            // Detect project language and operating system
            let language = compiler::detect_project_language(&actual_path);
            let os = compiler::detect_operating_system();

            // Get system information
            compiler::print_system_info();

            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  🌀 \x1b[1;36mChakra WASM Compiler\x1b[0m\n");
            println!(
                "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
                actual_path
            );
            println!(
                "  🔍 \x1b[1;34mDetected Language:\x1b[0m \x1b[1;32m{:?}\x1b[0m",
                language
            );
            println!(
                "  📤 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{}\x1b[0m",
                output_dir
            );

            // Check for missing tools
            let missing_tools = compiler::get_missing_tools(&language, &os);
            if !missing_tools.is_empty() {
                println!("\n  ⚠️  \x1b[1;33mMissing Required Tools:\x1b[0m");
                for tool in &missing_tools {
                    println!("     \x1b[1;31m• {}\x1b[0m", tool);
                }
                println!("\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m");
                println!("\x1b[1;34m╰\x1b[0m\n");
                return;
            }
            println!("\x1b[1;34m╰\x1b[0m\n");

            // Compile WASM
            match compiler::create_wasm_from_project(&actual_path, &output_dir) {
                Ok(wasm_path) => {
                    // WASM compiled successfully
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ✅ \x1b[1;36mWASM Compiled Successfully\x1b[0m\n");
                    println!(
                        "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                        wasm_path
                    );
                    println!("\n  🚀 \x1b[1;33mRun it with:\x1b[0m");
                    println!("     \x1b[1;37mchakra --wasm --path {}\x1b[0m", wasm_path);
                    println!("\x1b[1;34m╰\x1b[0m");
                }
                Err(e) => {
                    // Wasm compilation failed
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ❌ \x1b[1;31mError Compiling WASM:\x1b[0m");
                    println!("  \x1b[0;91m{}\x1b[0m", e);
                    println!("\x1b[1;34m╰\x1b[0m");
                }
            }
        }

        // Verify wasm file
        Some(cli::Commands::Verify {
            path,
            positional_path,
            detailed,
        }) => {
            // Determine the actual path to use
            let actual_path = positional_path
                .clone()
                .unwrap_or_else(|| path.clone().unwrap_or_else(|| String::from("./")));

            println!("🔍 Verifying WebAssembly file: {}", actual_path);

            let path_obj = Path::new(&actual_path);

            // Check if it's a WASM file
            if !path_obj.extension().map_or(false, |ext| ext == "wasm") {
                // Not a WASM file
                eprintln!("\n\x1b[1;34m╭\x1b[0m");
                eprintln!(
                    "  ❌ \x1b[1;31mError: Not a WASM file: {}\x1b[0m",
                    actual_path
                );
                eprintln!("\n  \x1b[1;37mPlease specify a path to a .wasm file:\x1b[0m\n");
                eprintln!("  \x1b[1;33mchakra verify --path /path/to/your/file.wasm\x1b[0m\n");
                eprintln!("  \x1b[0;90mTo run a WASM file directly, use:\x1b[0m");
                eprintln!("  \x1b[1;33mchakra --wasm --path /path/to/your/file.wasm\x1b[0m");
                eprintln!("\x1b[1;34m╰\x1b[0m");
                return;
            }

            match verify::verify_wasm(&actual_path) {
                Ok(result) => {
                    // Display verification results
                    verify::print_verification_results(&actual_path, &result, *detailed);
                }
                Err(e) => {
                    // Error verifying the file
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ❌ \x1b[1;31mVerification Error:\x1b[0m");
                    println!("  \x1b[0;91m{}\x1b[0m", e);
                    println!("\x1b[1;34m╰\x1b[0m");
                }
            }
        }

        // Inspect wasm file
        Some(cli::Commands::Inspect {
            path,
            positional_path,
        }) => {
            // Determine the actual path to use
            let actual_path = positional_path
                .clone()
                .unwrap_or_else(|| path.clone().unwrap_or_else(|| String::from("./")));

            println!("🔍 Inspecting WebAssembly file: {}", actual_path);
            match verify::print_detailed_binary_info(&actual_path) {
                Ok(()) => {
                    println!("Inspection completed successfully.");
                }
                Err(e) => {
                    eprintln!("\n\x1b[1;34m╭\x1b[0m");
                    eprintln!("  ❌ \x1b[1;31mInspection Error:\x1b[0m");
                    eprintln!("  \x1b[0;91m{}\x1b[0m", e);
                    eprintln!("\x1b[1;34m╰\x1b[0m");
                }
            }
        }

        // Run command
        Some(cli::Commands::Run {
            path,
            positional_path,
            port,
            language,
            watch,
        }) => {
            // Determine the actual path to use
            let actual_path = positional_path
                .clone()
                .unwrap_or_else(|| path.clone().unwrap_or_else(|| String::from("./")));

            server::run_project(&actual_path, *port, language.clone(), *watch);
        }

        // Default case - either run WASM file or compile and run project
        None => {
            // Get path and port from args
            let path = &args.path;
            let port = args.port;

            // Check if user specified --wasm flag
            if args.wasm {
                server::run_wasm_file(path, port);
            } else {
                // Check if it's a Rust web application
                let path_obj = Path::new(path);
                if path_obj.exists()
                    && path_obj.is_dir()
                    && compiler::detect_project_language(path) == compiler::ProjectLanguage::Rust
                    && compiler::is_rust_web_application(path)
                {
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  🌐 \x1b[1;36mDetected Rust Web Application\x1b[0m");
                    println!("  \x1b[0;37mRunning as a web app on port {}\x1b[0m", 3000); // Use port 3000 for web apps
                    println!("\x1b[1;34m╰\x1b[0m\n");

                    // Run as a web application on port 3000
                    if let Err(e) = server::run_webapp(path, 3000, args.watch) {
                        eprintln!("\n\x1b[1;34m╭\x1b[0m");
                        eprintln!("  ❌ \x1b[1;31mError Running Web Application:\x1b[0m");
                        eprintln!("  \x1b[0;91m{}\x1b[0m", e);
                        eprintln!("\x1b[1;34m╰\x1b[0m");
                    }
                } else {
                    // Default to compile and run project
                    server::run_project(path, port, None, args.watch);
                }
            }
        }
    }
}
