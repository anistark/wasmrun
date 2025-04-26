use clap::Parser;

/// Chakra - Run WebAssembly directly in browser 🌟
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to .wasm file
    #[arg(short = 'p', long)]
    pub path: String,

    /// Port to serve (default: 8420)
    #[arg(short = 'P', long, default_value_t = 8420)]
    pub port: u16,
}

pub fn get_args() -> Args {
    Args::parse()
}
