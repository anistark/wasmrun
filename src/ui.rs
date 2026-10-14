/// Print a success message
pub fn print_success(title: &str, message: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅ \x1b[1;36m{title}\x1b[0m");
    println!();
    println!("  ✅ \x1b[1;32m{message}\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ℹ️  \x1b[1;34m{message}\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
}

/// Print a status message
pub fn print_status(message: &str) {
    println!("\n⏳ {message}");
}

/// Print init command information
#[allow(dead_code)]
pub fn print_init_info(project_name: &str, template: &str, target_dir: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🚀 \x1b[1;36mInitializing New Wasmrun Project\x1b[0m\n");
    println!("  📦 \x1b[1;34mProject Name:\x1b[0m \x1b[1;33m{project_name}\x1b[0m");
    println!("  🎯 \x1b[1;34mTemplate:\x1b[0m \x1b[1;33m{template}\x1b[0m");
    println!("  📂 \x1b[1;34mDirectory:\x1b[0m \x1b[1;33m{target_dir}\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");
}

/// Print clean command information
pub fn print_clean_info(project_path: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🧹 \x1b[1;36mCleaning Project\x1b[0m\n");
    println!("  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{project_path}\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");
}
