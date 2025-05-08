use std::fs;
use std::path::Path;
use std::process::Command;

/// Build a WASM file from a Go project using TinyGo
pub fn build_wasm(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🐹 Building WASM from Go project at: {}", project_path);

    // Check if TinyGo is installed
    let check_tinygo = Command::new("tinygo").arg("version").output();

    if check_tinygo.is_err() {
        return Err(
            "TinyGo is not installed or not in PATH. Please install TinyGo for WASM compilation."
                .to_string(),
        );
    }

    // Find main.go or similar entry point
    let mut entry_file = None;
    let common_entry_files = ["main.go", "cmd/main.go", "app.go"];

    for entry_name in common_entry_files.iter() {
        let entry_path = Path::new(project_path).join(entry_name);
        if entry_path.exists() {
            entry_file = Some(entry_path);
            break;
        }
    }

    // If no common entry file found, look for any .go file
    if entry_file.is_none() {
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "go" {
                        entry_file = Some(entry.path());
                        break;
                    }
                }
            }
        }
    }

    let entry_path =
        entry_file.ok_or_else(|| "No Go source files found in project directory".to_string())?;

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Get the output filename
    let output_name = entry_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string()
        + ".wasm";
    let output_file = output_path.join(&output_name);

    println!("🔨 Building with TinyGo...");

    // Build with TinyGo
    let build_output = Command::new("tinygo")
        .current_dir(project_path)
        .args([
            "build",
            "-o",
            output_file.to_str().unwrap(),
            "-target=wasm",
            entry_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run TinyGo: {}", e))?;

    if !build_output.status.success() {
        return Err(format!(
            "Build failed: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    Ok(output_file.to_string_lossy().to_string())
}

/// Build a WASM file from a Go project with more detailed output
pub fn build_wasm_verbose(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🐹 Building WASM from Go project at: {}", project_path);

    // Check if TinyGo is installed
    let check_tinygo = Command::new("tinygo").arg("version").output();

    if check_tinygo.is_err() {
        return Err(
            "TinyGo is not installed or not in PATH. Please install TinyGo for WASM compilation."
                .to_string(),
        );
    }

    // Check for go.mod or other Go project indicators
    let go_mod_path = Path::new(project_path).join("go.mod");
    if !go_mod_path.exists() {
        let mut has_go_files = false;
        // Check if there are any .go files
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "go" {
                        has_go_files = true;
                        break;
                    }
                }
            }
        }

        if !has_go_files {
            return Err(format!(
                "No Go project indicators found in {}",
                project_path
            ));
        }

        println!("⚠️ No go.mod found, but Go files detected. Continuing without module support.");
    } else {
        println!("✅ Found go.mod file");
    }

    // Find main.go or similar entry point
    let mut entry_file = None;
    let common_entry_files = ["main.go", "cmd/main.go", "app.go"];

    for entry_name in common_entry_files.iter() {
        let entry_path = Path::new(project_path).join(entry_name);
        if entry_path.exists() {
            println!("✅ Found entry point: {}", entry_path.display());
            entry_file = Some(entry_path);
            break;
        }
    }

    // If no common entry file found, look for any .go file
    if entry_file.is_none() {
        println!("⚠️ No common entry file found, searching for any .go file...");
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "go" {
                        println!("✅ Found Go file: {}", entry.path().display());
                        entry_file = Some(entry.path());
                        break;
                    }
                }
            }
        }
    }

    let entry_path =
        entry_file.ok_or_else(|| "No Go source files found in project directory".to_string())?;

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Get the output filename
    let output_name = entry_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string()
        + ".wasm";
    let output_file = output_path.join(&output_name);

    println!("🔨 Building with TinyGo...");
    println!("📝 Log output:");

    // Build with TinyGo with inherit stdout/stderr for better user feedback
    let build_status = Command::new("tinygo")
        .current_dir(project_path)
        .args([
            "build",
            "-o",
            output_file.to_str().unwrap(),
            "-target=wasm",
            entry_path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to execute TinyGo: {}", e))?;

    if !build_status.success() {
        return Err("Build failed. See output above for details.".to_string());
    }

    println!("✅ TinyGo build completed successfully");

    // Check if the WASM file was created
    if !output_file.exists() {
        return Err(format!(
            "TinyGo build completed but WASM file was not created at expected location: {}",
            output_file.display()
        ));
    }

    println!(
        "📦 WASM file created: {} ({} bytes)",
        output_file.display(),
        fs::metadata(&output_file).map(|m| m.len()).unwrap_or(0)
    );

    Ok(output_file.to_string_lossy().to_string())
}
