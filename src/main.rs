mod cli;
mod compiler;
mod server;
mod template;
mod utils;
mod verify;

use std::path::Path;

fn main() {
    // Parse command line arguments
    let args = cli::get_args();

    match args.command {
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
        Some(cli::Commands::Compile { path, output }) => {
            let output_dir = output.unwrap_or_else(|| ".".to_string());

            // Detect project language and operating system
            let language = compiler::detect_project_language(&path);
            let os = compiler::detect_operating_system();

            // Get system information
            compiler::print_system_info();

            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  🌀 \x1b[1;36mChakra WASM Compiler\x1b[0m\n");
            println!(
                "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
                path
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
            match compiler::create_wasm_from_project(&path, &output_dir) {
                Ok(wasm_path) => {
                    // WASM compiled successfully
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ✅ \x1b[1;36mWASM Compiled Successfully\x1b[0m\n");
                    println!(
                        "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
                        wasm_path
                    );
                    println!("\n  🚀 \x1b[1;33mRun it with:\x1b[0m");
                    println!("     \x1b[1;37mchakra --path {}\x1b[0m", wasm_path);
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
        Some(cli::Commands::Verify { path, detailed }) => {
            println!("🔍 Verifying WebAssembly file: {}", path);

            match verify::verify_wasm(&path) {
                Ok(result) => {
                    // Display verification results
                    verify::print_verification_results(&path, &result, detailed);
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
        Some(cli::Commands::Inspect { path }) => {
            println!("🔍 Inspecting WebAssembly file: {}", path);
            match verify::print_detailed_binary_info(&path) {
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

        // Default case: Start the chakra server
        None => {
            let path = args.path;
            let path_obj = Path::new(&path);

            // Check if path is a directory
            if path_obj.is_dir() {
                println!("\n\x1b[1;34m╭\x1b[0m");
                println!("  🔍 \x1b[1;36mDetected directory: {}\x1b[0m", path);

                // Try to detect project language
                let language = compiler::detect_project_language(&path);

                // If it's a known project type, offer to compile
                if language != compiler::ProjectLanguage::Unknown {
                    println!("  📦 \x1b[1;34mDetected a {:?} project\x1b[0m", language);
                    println!("\n  💡 \x1b[1;33mTip: To compile this project to WASM, run:\x1b[0m");
                    println!("     \x1b[1;37mchakra compile --path {}\x1b[0m", path);
                    println!("\x1b[1;34m╰\x1b[0m");
                } else {
                    // If we can't find any WASM files in the directory, suggest compilation
                    println!("  ❓ \x1b[1;33mNo WASM files found in directory\x1b[0m");
                    println!("\n  💡 \x1b[1;33mTo run a WASM file, use --path to specify its location\x1b[0m");
                    println!("     \x1b[1;37mchakra --path /path/to/your/file.wasm\x1b[0m");
                    println!("\x1b[1;34m╰\x1b[0m");
                }
                return;
            }

            // Check if path is a WASM file
            if path_obj.extension().map_or(false, |ext| ext == "wasm") {
                // Run the server with the provided path and port
                if let Err(e) = server::run_server(&path, args.port) {
                    // Chakra server failed to start
                    eprintln!("\n\x1b[1;34m╭\x1b[0m");
                    eprintln!("  ❌ \x1b[1;31mError Running Chakra Server\x1b[0m\n");

                    let words: Vec<&str> = e.split_whitespace().collect();
                    let mut current_line = String::from("  ");

                    for word in words {
                        if current_line.len() + word.len() + 1 > 58 {
                            eprintln!("\x1b[0;91m{}\x1b[0m", current_line);
                            current_line = String::from("  ");
                        }

                        current_line.push_str(word);
                        current_line.push(' ');
                    }

                    if current_line.len() > 2 {
                        eprintln!("\x1b[0;91m{}\x1b[0m", current_line);
                    }

                    eprintln!("\x1b[1;34m╰\x1b[0m");
                }
            } else {
                // Not a WASM file
                eprintln!("\n\x1b[1;34m╭\x1b[0m");
                eprintln!("  ❌ \x1b[1;31mError: Not a WASM file: {}\x1b[0m", path);
                eprintln!("\n  \x1b[1;37mPlease specify a path to a .wasm file:\x1b[0m\n");
                eprintln!("  \x1b[1;33mchakra --path /path/to/your/file.wasm\x1b[0m\n");
                eprintln!("  \x1b[0;90mRun 'chakra --help' for more information\x1b[0m");
                eprintln!("\x1b[1;34m╰\x1b[0m");
            }
        }
    }
}
