use crate::compiler::builder::OptimizationLevel;

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

/// Print compilation information
#[allow(dead_code)]
pub fn print_compile_info(
    project_path: &str,
    language: &crate::compiler::ProjectLanguage,
    output_dir: &str,
    optimization: &OptimizationLevel,
    verbose: bool,
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🅦 \x1b[1;36mWasmrun WASM Compiler\x1b[0m\n");
    println!("  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{project_path}\x1b[0m");
    println!("  🔍 \x1b[1;34mDetected Language:\x1b[0m \x1b[1;32m{language:?}\x1b[0m");
    println!("  📤 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{output_dir}\x1b[0m");
    println!("  ⚡ \x1b[1;34mOptimization:\x1b[0m \x1b[1;33m{optimization:?}\x1b[0m");

    if verbose {
        println!("  🔊 \x1b[1;34mVerbose Mode:\x1b[0m \x1b[1;32mEnabled\x1b[0m");
    }

    println!("\x1b[1;34m╰\x1b[0m\n");
}

/// Print missing tools warning
#[allow(dead_code)]
pub fn print_missing_tools(missing_tools: &[String]) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ⚠️  \x1b[1;33mMissing Required Tools:\x1b[0m");
    for tool in missing_tools {
        println!("     \x1b[1;31m• {tool}\x1b[0m");
    }
    println!("\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");
}

/// Print compilation success message
#[allow(dead_code)]
pub fn print_compilation_success(
    wasm_path: &str,
    js_path: &Option<String>,
    additional_files: &[String],
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅ \x1b[1;36mWASM Compiled Successfully\x1b[0m\n");
    println!("  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{wasm_path}\x1b[0m");

    if let Some(js_file) = js_path {
        println!("  📝 \x1b[1;34mJS File:\x1b[0m \x1b[1;32m{js_file}\x1b[0m");
    }

    if !additional_files.is_empty() {
        println!("  📄 \x1b[1;34mAdditional Files:\x1b[0m");
        for file in additional_files {
            println!("     \x1b[1;37m• {file}\x1b[0m");
        }
    }

    println!("\n  🚀 \x1b[1;33mRun it with:\x1b[0m");
    println!("     \x1b[1;37mwasmrun --wasm --path {wasm_path}\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m");
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
