use std::fs;
use std::path::Path;
use std::process::Command;

/// Check if a Rust project uses wasm-bindgen
pub fn uses_wasm_bindgen(project_path: &str) -> bool {
    let cargo_toml_path = Path::new(project_path).join("Cargo.toml");

    if let Ok(cargo_toml) = fs::read_to_string(cargo_toml_path) {
        // Check for wasm-bindgen dependencies
        if cargo_toml.contains("wasm-bindgen") {
            return true;
        }

        // Also check for common patterns that indicate wasm-bindgen usage
        if cargo_toml.contains("web-sys") || cargo_toml.contains("js-sys") {
            return true;
        }
    }

    false
}

/// Build a WASM file from a Rust project using wasm-bindgen
pub fn build_wasm_bindgen(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!(
        "🦀 Building WASM from Rust project with wasm-bindgen at: {}",
        project_path
    );

    // Check if wasm-pack is installed
    let check_wasm_pack = Command::new("wasm-pack").arg("--version").output();

    if check_wasm_pack.is_err() || !check_wasm_pack.as_ref().unwrap().status.success() {
        println!("⚠️ wasm-pack is not installed. Attempting to install it...");

        let install_output = Command::new("cargo")
            .args(["install", "wasm-pack"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to install wasm-pack: {}", e))?;

        if !install_output.success() {
            return Err("Failed to install wasm-pack. Please install it manually with 'cargo install wasm-pack'".to_string());
        }

        println!("✅ wasm-pack installed successfully");
    } else if let Ok(output) = check_wasm_pack {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("✅ Using wasm-pack version: {}", version.trim());
    }

    // Build the project with wasm-pack
    println!("🔨 Building with wasm-pack...");

    // Create a temporary directory for build output
    let _build_target_dir = Path::new(project_path).join("pkg");

    let build_output = Command::new("wasm-pack")
        .current_dir(project_path)
        .args(["build", "--target", "web"])
        .output()
        .map_err(|e| format!("Failed to build project with wasm-pack: {}", e))?;

    if !build_output.status.success() {
        return Err(format!(
            "Build failed: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Find the wasm file in the pkg directory
    let pkg_dir = Path::new(project_path).join("pkg");

    if !pkg_dir.exists() {
        return Err("wasm-pack build completed but pkg directory was not created".to_string());
    }

    // Find the wasm file and js file
    let mut wasm_file = None;
    let mut js_file = None;

    if let Ok(entries) = fs::read_dir(&pkg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "wasm" {
                    wasm_file = Some(path.clone());
                } else if ext == "js" {
                    // We're looking for the main JS file which usually has the same name as the package
                    if !path.file_name().unwrap().to_string_lossy().contains(".d.") {
                        js_file = Some(path.clone());
                    }
                }
            }
        }
    }

    let wasm_path = wasm_file.ok_or_else(|| "No WASM file found in pkg directory".to_string())?;
    let js_path = js_file.ok_or_else(|| "No JS file found in pkg directory".to_string())?;

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Copy the wasm file to the output directory
    let output_wasm_file = output_path.join(wasm_path.file_name().unwrap());
    fs::copy(&wasm_path, &output_wasm_file)
        .map_err(|e| format!("Failed to copy WASM file: {}", e))?;

    // Copy the js file to the output directory
    let output_js_file = output_path.join(js_path.file_name().unwrap());
    fs::copy(&js_path, &output_js_file).map_err(|e| format!("Failed to copy JS file: {}", e))?;

    // Also copy any .d.ts files if they exist
    if let Ok(entries) = fs::read_dir(&pkg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension.to_string_lossy() == "ts" {
                    let output_ts_file = output_path.join(path.file_name().unwrap());
                    let _ = fs::copy(&path, &output_ts_file); // Ignore errors for .d.ts files
                }
            }
        }
    }

    println!(
        "📦 WASM bundle created: {} with JS glue code",
        output_wasm_file.to_string_lossy()
    );

    // Return the path to the JS file as the main entry point
    Ok(output_js_file.to_string_lossy().to_string())
}

/// Build a WASM file from a Rust project with wasm-bindgen with more detailed output
pub fn build_wasm_bindgen_verbose(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!(
        "🦀 Building WASM from Rust project with wasm-bindgen at: {}",
        project_path
    );

    // Check if wasm-pack is installed
    let check_wasm_pack = Command::new("wasm-pack").arg("--version").output();

    if check_wasm_pack.is_err() || !check_wasm_pack.as_ref().unwrap().status.success() {
        println!("⚠️ wasm-pack is not installed. Attempting to install it...");

        let install_output = Command::new("cargo")
            .args(["install", "wasm-pack"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to install wasm-pack: {}", e))?;

        if !install_output.success() {
            return Err("Failed to install wasm-pack. Please install it manually with 'cargo install wasm-pack'".to_string());
        }

        println!("✅ wasm-pack installed successfully");
    } else if let Ok(output) = check_wasm_pack {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("✅ Using wasm-pack version: {}", version.trim());
    }

    // Check for wasm32-unknown-unknown target
    let check_target = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|e| format!("Failed to run rustup: {}", e))?;

    let target_output = String::from_utf8_lossy(&check_target.stdout);
    if !target_output.contains("wasm32-unknown-unknown") {
        println!("⚙️ Installing wasm32-unknown-unknown target...");

        let install_target = Command::new("rustup")
            .args(["target", "add", "wasm32-unknown-unknown"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to install wasm32 target: {}", e))?;

        if !install_target.success() {
            return Err("Failed to install wasm32-unknown-unknown target".to_string());
        }

        println!("✅ wasm32-unknown-unknown target installed");
    }

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Build with wasm-pack with detailed output
    println!("🔨 Building with wasm-pack...");
    println!("📝 Log output:");

    // Use Command::new with inherit stdout/stderr for better user feedback
    let build_status = Command::new("wasm-pack")
        .current_dir(project_path)
        .args(["build", "--target", "web", "--dev"]) // Use --dev flag for faster builds during development
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to execute wasm-pack build: {}", e))?;

    if !build_status.success() {
        return Err("Build failed. See output above for details.".to_string());
    }

    println!("✅ wasm-pack build completed successfully");

    // Find the wasm file and js file in the pkg directory
    let pkg_dir = Path::new(project_path).join("pkg");

    if !pkg_dir.exists() {
        return Err("wasm-pack build completed but pkg directory was not created".to_string());
    }

    // Find the wasm file and js file
    let mut wasm_file = None;
    let mut js_file = None;
    let mut found_files = Vec::new();

    if let Ok(entries) = fs::read_dir(&pkg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            found_files.push(path.file_name().unwrap().to_string_lossy().to_string());

            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "wasm" {
                    wasm_file = Some(path.clone());
                    println!("✅ Found WASM file: {}", path.display());
                } else if ext == "js" {
                    // We're looking for the main JS file which usually has the same name as the package
                    if !path.file_name().unwrap().to_string_lossy().contains(".d.") {
                        js_file = Some(path.clone());
                        println!("✅ Found JS file: {}", path.display());
                    }
                }
            }
        }
    }

    if wasm_file.is_none() || js_file.is_none() {
        println!("❌ Could not find required files in the pkg directory");
        println!("Files in pkg directory: {}", found_files.join(", "));
        return Err("Missing WASM or JS files in pkg directory".to_string());
    }

    let wasm_path = wasm_file.unwrap();
    let js_path = js_file.unwrap();

    // Copy the wasm file to the output directory
    let output_wasm_file = output_path.join(wasm_path.file_name().unwrap());
    fs::copy(&wasm_path, &output_wasm_file)
        .map_err(|e| format!("Failed to copy WASM file: {}", e))?;

    // Copy the js file to the output directory
    let output_js_file = output_path.join(js_path.file_name().unwrap());
    fs::copy(&js_path, &output_js_file).map_err(|e| format!("Failed to copy JS file: {}", e))?;

    // Also copy any .d.ts files if they exist
    if let Ok(entries) = fs::read_dir(&pkg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension.to_string_lossy() == "ts" {
                    let output_ts_file = output_path.join(path.file_name().unwrap());
                    let _ = fs::copy(&path, &output_ts_file);
                    println!(
                        "✅ Copied TypeScript definition: {}",
                        path.file_name().unwrap().to_string_lossy()
                    );
                }
            }
        }
    }

    println!(
        "📦 WASM bundle created: {} with JS glue code: {}",
        output_wasm_file.display(),
        output_js_file.display()
    );

    // Return the path to the JS file as the main entry point
    Ok(output_js_file.to_string_lossy().to_string())
}

/// Build a WASM file from a Rust project
pub fn build_wasm(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🦀 Building WASM from Rust project at: {}", project_path);

    // Check if the project uses wasm-bindgen
    if uses_wasm_bindgen(project_path) {
        println!("🔍 Detected wasm-bindgen usage, using wasm-pack for compilation...");
        return build_wasm_bindgen(project_path, output_dir);
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

    // Check if the project uses wasm-bindgen
    if uses_wasm_bindgen(project_path) {
        println!("🔍 Detected wasm-bindgen usage, using wasm-pack for compilation...");
        return build_wasm_bindgen_verbose(project_path, output_dir);
    }

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
