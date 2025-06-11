use crate::compiler;
use crate::error::{ChakraError, Result};
use crate::ui::print_clean_info;
use crate::utils::PathResolver;

/// Handle clean command
pub fn handle_clean_command(path: &Option<String>, positional_path: &Option<String>) -> Result<()> {
    let project_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());
    PathResolver::validate_directory_exists(&project_path)?;

    print_clean_info(&project_path);

    let language = compiler::detect_project_language(&project_path);

    match language {
        compiler::ProjectLanguage::Rust => clean_rust_project(&project_path),
        compiler::ProjectLanguage::Go => clean_go_project(&project_path),
        compiler::ProjectLanguage::C => clean_c_project(&project_path),
        compiler::ProjectLanguage::Asc => clean_asc_project(&project_path),
        _ => {
            println!(
                "⚠️ Clean operation not specifically implemented for {:?}",
                language
            );
            println!("💡 You can manually delete build artifacts in your project directory.");
            Ok(())
        }
    }
}

fn clean_rust_project(project_path: &str) -> Result<()> {
    let target_dir = PathResolver::join_paths(project_path, "target");
    let pkg_dir = PathResolver::join_paths(project_path, "pkg");

    let mut cleaned = Vec::new();

    if std::path::Path::new(&target_dir).exists() {
        PathResolver::remove_dir_all(&target_dir)?;
        cleaned.push("target/");
    }

    if std::path::Path::new(&pkg_dir).exists() {
        PathResolver::remove_dir_all(&pkg_dir)?;
        cleaned.push("pkg/");
    }

    if cleaned.is_empty() {
        println!("✨ Project is already clean!");
    } else {
        println!("🧹 Cleaned: {}", cleaned.join(", "));
    }

    Ok(())
}

fn clean_go_project(project_path: &str) -> Result<()> {
    let output = std::process::Command::new("go")
        .args(["clean", "-cache", "-modcache"])
        .current_dir(project_path)
        .output()
        .map_err(|e| ChakraError::from(format!("Failed to run go clean: {}", e)))?;

    if output.status.success() {
        println!("🧹 Go project cleaned successfully");
    } else {
        println!("⚠️ Go clean completed with warnings");
    }

    Ok(())
}

fn clean_c_project(project_path: &str) -> Result<()> {
    let mut cleaned = Vec::new();

    let wasm_files = PathResolver::find_files_with_extension(project_path, "wasm")?;
    for file in wasm_files {
        PathResolver::remove_file(&file)?;
        cleaned.push(PathResolver::get_filename(&file)?);
    }

    if cleaned.is_empty() {
        println!("✨ No C build artifacts found to clean");
    } else {
        println!("🧹 Cleaned: {}", cleaned.join(", "));
    }

    Ok(())
}

fn clean_asc_project(project_path: &str) -> Result<()> {
    let build_dir = PathResolver::join_paths(project_path, "build");
    let dist_dir = PathResolver::join_paths(project_path, "dist");

    let mut cleaned = Vec::new();

    for dir in [&build_dir, &dist_dir] {
        if std::path::Path::new(dir).exists() {
            PathResolver::remove_dir_all(dir)?;
            cleaned.push(PathResolver::get_filename(dir)?);
        }
    }

    if cleaned.is_empty() {
        println!("✨ Project is already clean!");
    } else {
        println!("🧹 Cleaned: {}", cleaned.join(", "));
    }

    Ok(())
}
