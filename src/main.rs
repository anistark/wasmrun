mod cli;
mod server;
mod template;
mod utils;

fn main() {
    // Parse command line arguments
    let args = cli::get_args();

    match args.command {
        Some(cli::Commands::Stop) => {
            // Check if a server is running before attempting to stop it
            if !server::is_server_running() {
                // Display a nice message when no server is running
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
                    // Display a nice box for successful server stop
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  🛑 \x1b[1;36mChakra Server Stopped\x1b[0m");
                    println!();
                    println!("  ✅ \x1b[1;32mServer terminated successfully\x1b[0m");
                    println!("\x1b[1;34m╰\x1b[0m");
                },
                Err(e) => {
                    // Display error in a box
                    println!("\n\x1b[1;34m╭\x1b[0m");
                    println!("  ❌ \x1b[1;31mError stopping server:\x1b[0m");
                    println!("  \x1b[0;91m{}\x1b[0m", e);
                    println!("\x1b[1;34m╰\x1b[0m");
                }
            }
        }

        None => {
            // Default to start if no subcommand is provided
            if let Some(path) = args.path.clone() {
                // Run the server with the provided path and port
                if let Err(e) = server::run_server(&path, args.port) {
                    // Display a nice error box
                    eprintln!("\n\x1b[1;34m╭\x1b[0m");
                    eprintln!("  ❌ \x1b[1;31mError Running Chakra Server\x1b[0m");
                    eprintln!();
                    
                    // Split error message into multiple lines if needed
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
                    
                    // Print the last line
                    if current_line.len() > 2 {
                        eprintln!("\x1b[0;91m{}\x1b[0m", current_line);
                    }
                    
                    eprintln!("\x1b[1;34m╰\x1b[0m");
                }
            } else {
                // Nice error box for missing path
                eprintln!("\n\x1b[1;34m╭\x1b[0m");
                eprintln!("  ❌ \x1b[1;31mError: No path provided for the WASM file\x1b[0m");
                eprintln!();
                eprintln!("  \x1b[1;37mPlease specify a path using the --path option:\x1b[0m");
                eprintln!();
                eprintln!("  \x1b[1;33mchakra --path /path/to/your/file.wasm\x1b[0m");
                eprintln!();
                eprintln!("  \x1b[0;90mRun 'chakra --help' for more information\x1b[0m");
                eprintln!("\x1b[1;34m╰\x1b[0m");
            }
        }
    }
}