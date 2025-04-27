mod cli;
mod server;
mod template;
mod utils;

fn main() {
    // Parse command line arguments
    let args = cli::get_args();

    match args.command {
        Some(cli::Commands::Stop) => {
            // Stop the existing server
            if let Err(e) = server::stop_existing_server() {
                eprintln!("❗ Error stopping the server: {e}");
            } else {
                println!("💀 Existing server stopped.");
            }
        }

        None => {
            // Default to start if no subcommand is provided
            if let Some(path) = args.path {
                // Run the server with the provided path and port
                if let Err(e) = server::run_server(&path, args.port) {
                    eprintln!("❗ Error running the server: {e}");
                }
            } else {
                eprintln!("❗ No path provided for the WASM file. Please specify a path.");
                eprintln!("  Example: chakra --path /path/to/your/file.wasm");
            }
        }
    }
}
