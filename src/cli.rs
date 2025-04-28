use clap::{Parser, Subcommand};

/// Chakra - Run WebAssembly directly in browser 🌟
#[derive(Parser, Debug)]
#[command(author, version = get_version_string(), about, long_about = None, 
          after_help = "If you find Chakra useful, please consider starring the repository \
                       on GitHub to support this open source project! ✨\n\
                       https://github.com/anistark/chakra")]
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

/// Get enhanced version string with colors
fn get_version_string() -> &'static str {
    // Note: We need to use a static string because clap expects static lifetime for version
    // This means we can't use dynamic formatting with colors here directly
    // We'll rely on version printing hooks instead
    env!("CARGO_PKG_VERSION")
}

pub fn get_args() -> Args {
    // Print our custom styled version if the user asked for the version
    if std::env::args().any(|arg| arg == "-V" || arg == "--version") {
        print_styled_version();
        std::process::exit(0);
    }
    
    Args::parse()
}

/// Print a styled version output
fn print_styled_version() {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");
    
    println!(
        "\n\x1b[1;34m╭\x1b[0m\n\
         \x1b[1;34m│\x1b[0m  🌀 \x1b[1;36m{} v{}\x1b[0m\n\
         \x1b[1;34m│\x1b[0m  \x1b[0;90mA lightweight WebAssembly runner\x1b[0m\n\
         \x1b[1;34m╰\x1b[0m\n",
        name, version
    );
}