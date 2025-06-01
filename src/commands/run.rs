use crate::cli::CommandValidator;
use crate::error::Result;
use crate::server::{self};

/// Handle run command
pub fn handle_run_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    port: u16,
    language: &Option<String>,
    watch: bool,
) -> Result<()> {
    let (project_path, validated_port) =
        CommandValidator::validate_run_args(path, positional_path, port)?;

    // Print initial status
    println!("\n🚀 \x1b[1;36mInitializing Chakra...\x1b[0m");

    // Detect what we're running and show preview
    let path_obj = std::path::Path::new(&project_path);

    if path_obj.is_file() {
        if let Some(ext) = path_obj.extension() {
            match ext.to_str() {
                Some("wasm") => {
                    println!("📦 \x1b[1;34mDetected:\x1b[0m WebAssembly file");

                    // Quick preview without full analysis
                    if let Ok(metadata) = std::fs::metadata(&project_path) {
                        let size = format_file_size(metadata.len());
                        println!("💾 \x1b[1;34mSize:\x1b[0m {}", size);
                    }
                }
                Some("js") => {
                    println!("📜 \x1b[1;34mDetected:\x1b[0m JavaScript file (checking for WASM bindings...)");
                }
                _ => {
                    println!("❓ \x1b[1;33mUnknown file type, attempting to run...\x1b[0m");
                }
            }
        }
    } else if path_obj.is_dir() {
        println!("📁 \x1b[1;34mDetected:\x1b[0m Project directory");

        // Quick language detection
        let language = crate::compiler::detect_project_language(&project_path);
        let language_icon = match language {
            crate::compiler::ProjectLanguage::Rust => "🦀",
            crate::compiler::ProjectLanguage::Go => "🐹",
            crate::compiler::ProjectLanguage::C => "🔧",
            crate::compiler::ProjectLanguage::AssemblyScript => "📜",
            crate::compiler::ProjectLanguage::Python => "🐍",
            _ => "❓",
        };

        println!(
            "{} \x1b[1;34mLanguage:\x1b[0m {:?}",
            language_icon, language
        );

        if language == crate::compiler::ProjectLanguage::Rust
            && crate::compiler::is_rust_web_application(&project_path)
        {
            println!("🌐 \x1b[1;32mWeb Application detected\x1b[0m");
        }

        if watch {
            println!("👀 \x1b[1;32mWatch mode enabled\x1b[0m");
        }
    } else {
        println!("❌ \x1b[1;31mPath not found:\x1b[0m {}", project_path);
    }

    // Add a small delay for user to see the preview
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Run the actual server with full analysis
    server::run_project(&project_path, validated_port, language.clone(), watch)?;
    Ok(())
}

/// Helper function to format file sizes
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} bytes", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
