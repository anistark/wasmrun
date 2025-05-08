use std::fs;
use std::path::Path;
use std::process::Command;

/// Build a WASM file from a Rust project
pub fn build_wasm(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🦀 Building WASM from Rust project at: {}", project_path);

    // Check if wasm32 target is installed
    let check_target = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|e| format!("Failed to run rustup: {}", e))?;

    let target_output = String::from_utf8_lossy(&check_target.stdout);
    if !target_output.contains("wasm32-unknown-unknown") {
        // Target not installed, try to install it
        println!("⚙️ Installing wasm32-unknown-unknown target...");

        let install_target = Command::new("rustup")
            .args(["target", "add", "wasm32-unknown-unknown"])
            .output()
            .map_err(|e| format!("Failed to install wasm32 target: {}", e))?;

        if !install_target.status.success() {
            return Err(format!(
                "Failed to install wasm32 target: {}",
                String::from_utf8_lossy(&install_target.stderr)
            ));
        }
    }

    // Build the project
    println!("🔨 Building the project...");

    let build_output = Command::new("cargo")
        .current_dir(project_path)
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .output()
        .map_err(|e| format!("Failed to build project: {}", e))?;

    if !build_output.status.success() {
        return Err(format!(
            "Build failed: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Find the wasm file
    let target_dir = Path::new(project_path).join("target/wasm32-unknown-unknown/release");

    let mut wasm_file = None;
    if let Ok(entries) = fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            if let Some(extension) = entry.path().extension() {
                if extension == "wasm" {
                    wasm_file = Some(entry.path());
                    break;
                }
            }
        }
    }

    let wasm_path =
        wasm_file.ok_or_else(|| "No WASM file found in target directory".to_string())?;

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Copy the wasm file to the output directory
    let output_file = output_path.join(wasm_path.file_name().unwrap());
    fs::copy(&wasm_path, &output_file).map_err(|e| format!("Failed to copy WASM file: {}", e))?;

    Ok(output_file.to_string_lossy().to_string())
}

/// Build a WASM file from a Rust project with more detailed output
pub fn build_wasm_verbose(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🦀 Building WASM from Rust project at: {}", project_path);

    // Check for Cargo.toml
    let cargo_path = Path::new(project_path).join("Cargo.toml");
    if !cargo_path.exists() {
        return Err(format!("No Cargo.toml found in {}", project_path));
    }

    // Check if wasm32 target is installed
    let check_target = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|e| format!("Failed to run rustup: {}", e))?;

    let target_output = String::from_utf8_lossy(&check_target.stdout);
    if !target_output.contains("wasm32-unknown-unknown") {
        // Target not installed, try to install it
        println!("⚙️ Installing wasm32-unknown-unknown target...");

        let install_target = Command::new("rustup")
            .args(["target", "add", "wasm32-unknown-unknown"])
            .output()
            .map_err(|e| format!("Failed to install wasm32 target: {}", e))?;

        if !install_target.status.success() {
            return Err(format!(
                "Failed to install wasm32 target: {}",
                String::from_utf8_lossy(&install_target.stderr)
            ));
        }
    }

    // Build the project with more verbose output
    println!("🔨 Building with cargo build...");
    println!("📝 Log output:");

    // Use Command::new with inherit stdout/stderr for better user feedback
    let build_status = Command::new("cargo")
        .current_dir(project_path)
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to execute cargo build: {}", e))?;

    if !build_status.success() {
        return Err("Build failed. See output above for details.".to_string());
    }

    // Find the wasm file
    let target_dir = Path::new(project_path).join("target/wasm32-unknown-unknown/release");

    let mut wasm_file = None;
    if let Ok(entries) = fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            if let Some(extension) = entry.path().extension() {
                if extension == "wasm" {
                    wasm_file = Some(entry.path());
                    break;
                }
            }
        }
    }

    let wasm_path =
        wasm_file.ok_or_else(|| "No WASM file found in target directory".to_string())?;

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Copy the wasm file to the output directory
    let output_file = output_path.join(wasm_path.file_name().unwrap());
    fs::copy(&wasm_path, &output_file).map_err(|e| format!("Failed to copy WASM file: {}", e))?;

    Ok(output_file.to_string_lossy().to_string())
}
