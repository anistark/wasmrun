use crate::cli::CommandValidator;
use crate::error::Result;
use crate::server::{self};
use crate::utils::CommandExecutor;

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

    println!("\n🚀 \x1b[1;36mInitializing Chakra...\x1b[0m");

    let path_obj = std::path::Path::new(&project_path);

    if path_obj.is_file() {
        if let Some(ext) = path_obj.extension() {
            match ext.to_str() {
                Some("wasm") => {
                    println!("📦 \x1b[1;34mDetected:\x1b[0m WebAssembly file");

                    if let Ok(metadata) = std::fs::metadata(&project_path) {
                        let size = CommandExecutor::format_file_size(metadata.len());
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

        let language = crate::compiler::detect_project_language(&project_path);
        let language_icon = match language {
            crate::compiler::ProjectLanguage::Rust => "🦀",
            crate::compiler::ProjectLanguage::Go => "🐹",
            crate::compiler::ProjectLanguage::C => "🔧",
            crate::compiler::ProjectLanguage::Asc => "📜",
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

    // TODO: Remove delay when server is ready
    std::thread::sleep(std::time::Duration::from_millis(500));

    server::run_project(&project_path, validated_port, language.clone(), watch)?;
    Ok(())
}
