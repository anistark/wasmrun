use crate::compiler::builder::OptimizationLevel;

/// Print a success message
pub fn print_success(title: &str, message: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅ \x1b[1;36m{}\x1b[0m", title);
    println!();
    println!("  ✅ \x1b[1;32m{}\x1b[0m", message);
    println!("\x1b[1;34m╰\x1b[0m");
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ℹ️  \x1b[1;34m{}\x1b[0m", message);
    println!("\x1b[1;34m╰\x1b[0m");
}

/// Print a status message
pub fn print_status(message: &str) {
    println!("\n⏳ {}", message);
}

/// Print compilation information
pub fn print_compile_info(
    project_path: &str,
    language: &crate::compiler::ProjectLanguage,
    output_dir: &str,
    optimization: &OptimizationLevel,
    verbose: bool,
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🅦 \x1b[1;36mWasmrun WASM Compiler\x1b[0m\n");
    println!(
        "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
        project_path
    );
    println!(
        "  🔍 \x1b[1;34mDetected Language:\x1b[0m \x1b[1;32m{:?}\x1b[0m",
        language
    );
    println!(
        "  📤 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{}\x1b[0m",
        output_dir
    );
    println!(
        "  ⚡ \x1b[1;34mOptimization:\x1b[0m \x1b[1;33m{:?}\x1b[0m",
        optimization
    );

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
        println!("     \x1b[1;31m• {}\x1b[0m", tool);
    }
    println!("\n  \x1b[0;37mPlease install the required tools to compile this project.\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");
}

/// Print compilation success message
pub fn print_compilation_success(
    wasm_path: &str,
    js_path: &Option<String>,
    additional_files: &[String],
) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  ✅ \x1b[1;36mWASM Compiled Successfully\x1b[0m\n");
    println!(
        "  📦 \x1b[1;34mWASM File:\x1b[0m \x1b[1;32m{}\x1b[0m",
        wasm_path
    );

    if let Some(js_file) = js_path {
        println!(
            "  📝 \x1b[1;34mJS File:\x1b[0m \x1b[1;32m{}\x1b[0m",
            js_file
        );
    }

    if !additional_files.is_empty() {
        println!("  📄 \x1b[1;34mAdditional Files:\x1b[0m");
        for file in additional_files {
            println!("     \x1b[1;37m• {}\x1b[0m", file);
        }
    }

    println!("\n  🚀 \x1b[1;33mRun it with:\x1b[0m");
    println!("     \x1b[1;37mwasmrun --wasm --path {}\x1b[0m", wasm_path);
    println!("\x1b[1;34m╰\x1b[0m");
}

/// Print init command information
#[allow(dead_code)]
pub fn print_init_info(project_name: &str, template: &str, target_dir: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🚀 \x1b[1;36mInitializing New Wasmrun Project\x1b[0m\n");
    println!(
        "  📦 \x1b[1;34mProject Name:\x1b[0m \x1b[1;33m{}\x1b[0m",
        project_name
    );
    println!(
        "  🎯 \x1b[1;34mTemplate:\x1b[0m \x1b[1;33m{}\x1b[0m",
        template
    );
    println!(
        "  📂 \x1b[1;34mDirectory:\x1b[0m \x1b[1;33m{}\x1b[0m",
        target_dir
    );
    println!("\x1b[1;34m╰\x1b[0m\n");
}

/// Print clean command information
pub fn print_clean_info(project_path: &str) {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🧹 \x1b[1;36mCleaning Project\x1b[0m\n");
    println!(
        "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
        project_path
    );
    println!("\x1b[1;34m╰\x1b[0m\n");
}
