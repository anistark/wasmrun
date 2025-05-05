use clap::{Parser, Subcommand};

/// Chakra - Run WebAssembly directly in browser 🌟
#[derive(Parser, Debug)]
#[command(author, version = get_version_string(), about, long_about = None, after_help = "If you find Chakra useful, please consider starring the repository \
                       on GitHub to support this open source project! ✨\n\
                       https://github.com/anistark/chakra")]
pub struct Args {
    /// Subcommands to control Chakra server
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to .wasm file
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

    /// Compile a project to WebAssembly
    Compile {
        /// Path to the project directory
        #[arg(short = 'p', long)]
        path: String,

        /// Output directory for the WASM file (default: current directory)
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
    
    /// Verify WebAssembly file
    Verify {
        /// Path to the WASM file
        #[arg(short = 'p', long)]
        path: String,
        
        /// Show detailed information about the WASM module
        #[arg(short = 'd', long)]
        detailed: bool,
    },
}

/// Get version string
fn get_version_string() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn get_args() -> Args {
    if std::env::args().any(|arg| arg == "-V" || arg == "--version") {
        print_styled_version();
        std::process::exit(0);
    }

    Args::parse()
}

/// Print styled version output
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
