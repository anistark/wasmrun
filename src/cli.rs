use clap::{Parser, Subcommand};

/// Chakra - Run WebAssembly directly in browser 🌟
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Subcommands to control Chakra server
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to .wasm file (only required for the start command)
    #[arg(short = 'p', long)]
    pub path: Option<String>,

    /// Port to serve (default: 8420)
    #[arg(short = 'P', long, default_value_t = 8420)]
    pub port: u16,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Stop the running Chakra server
    Stop,
}

pub fn get_args() -> Args {
    Args::parse()
}