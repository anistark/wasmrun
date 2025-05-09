use std::fs;
use std::path::Path;
use std::process::Command;

/// Build a WASM file from an AssemblyScript project
pub fn build_wasm(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!(
        "⚡️ Building WASM from AssemblyScript project at: {}",
        project_path
    );

    // Check if Node.js is installed
    let check_node = Command::new("node").arg("--version").output();

    if check_node.is_err() {
        return Err("Node.js is not installed or not in PATH. Please install Node.js.".to_string());
    }

    // Check if asc is installed (either globally or locally via npx)
    println!("⚙️ Building the project with AssemblyScript...");

    // Try to build with npx asc first
    let build_output = Command::new("npx")
        .current_dir(project_path)
        .args([
            "asc",
            "--optimize",
            "--outFile",
            "build/release.wasm",
            "assembly/index.ts",
        ])
        .output();

    let wasm_file = if let Ok(output) = build_output {
        if output.status.success() {
            Path::new(project_path).join("build/release.wasm")
        } else {
            // Try npm build command instead
            let npm_build = Command::new("npm")
                .current_dir(project_path)
                .args(["run", "asbuild"])
                .output()
                .map_err(|e| format!("Failed to build AssemblyScript project: {}", e))?;

            if !npm_build.status.success() {
                return Err(format!(
                    "Build failed: {}",
                    String::from_utf8_lossy(&npm_build.stderr)
                ));
            }

            // Look for build output files
            let build_dir = Path::new(project_path).join("build");
            let mut wasm_path = None;

            if build_dir.exists() {
                if let Ok(entries) = fs::read_dir(&build_dir) {
                    for entry in entries.flatten() {
                        if let Some(extension) = entry.path().extension() {
                            if extension == "wasm" {
                                wasm_path = Some(entry.path());
                                break;
                            }
                        }
                    }
                }
            }

            wasm_path.ok_or_else(|| "No WASM file found after build".to_string())?
        }
    } else {
        return Err(
            "AssemblyScript compiler not found. Make sure it's installed in your project."
                .to_string(),
        );
    };

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Copy the wasm file to the output directory
    let output_file = output_path.join(wasm_file.file_name().unwrap());
    fs::copy(&wasm_file, &output_file).map_err(|e| format!("Failed to copy WASM file: {}", e))?;

    Ok(output_file.to_string_lossy().to_string())
}

/// Build a WASM file from an AssemblyScript project with more detailed output
pub fn build_wasm_verbose(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!(
        "⚡️ Building WASM from AssemblyScript project at: {}",
        project_path
    );

    // Check if Node.js is installed
    let check_node = Command::new("node").arg("--version").output();

    if check_node.is_err() {
        return Err("Node.js is not installed or not in PATH. Please install Node.js.".to_string());
    }

    // Print Node.js version for debugging
    if let Ok(node_output) = check_node {
        if node_output.status.success() {
            let version = String::from_utf8_lossy(&node_output.stdout);
            println!("✅ Using Node.js version: {}", version.trim());
        }
    }

    // Check for package.json
    let package_json_path = Path::new(project_path).join("package.json");
    if !package_json_path.exists() {
        return Err(format!(
            "No package.json found in {}. AssemblyScript requires an npm project.",
            project_path
        ));
    }

    // Read package.json to check if it contains AssemblyScript
    let package_json_content = fs::read_to_string(&package_json_path)
        .map_err(|e| format!("Failed to read package.json: {}", e))?;

    if !package_json_content.contains("\"assemblyscript\"") {
        println!("⚠️ package.json doesn't seem to list assemblyscript as a dependency!");
        println!(
            "   This might not be an AssemblyScript project, but we'll try to build it anyway."
        );
    } else {
        println!("✅ Found AssemblyScript dependency in package.json");
    }

    // Check for asconfig.json
    let asconfig_path = Path::new(project_path).join("asconfig.json");
    if asconfig_path.exists() {
        println!("✅ Found asconfig.json configuration file");
    } else {
        println!(
            "⚠️ No asconfig.json found. This file is usually present in AssemblyScript projects."
        );
        println!("   We'll try to build using default paths and settings.");
    }

    // Check for assembly directory and index.ts
    let assembly_dir = Path::new(project_path).join("assembly");
    let index_ts = assembly_dir.join("index.ts");

    if !assembly_dir.exists() || !assembly_dir.is_dir() {
        println!("⚠️ No 'assembly' directory found. This is the standard source directory for AssemblyScript.");
        println!("   Checking for other possible entry points...");

        // Look for any .ts files in the project root
        let mut found_ts = false;
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "ts" {
                        println!("✅ Found TypeScript file: {}", entry.path().display());
                        found_ts = true;
                    }
                }
            }
        }

        if !found_ts {
            return Err("No TypeScript source files found in project directory".to_string());
        }
    } else if !index_ts.exists() {
        println!("⚠️ 'assembly' directory found but no index.ts inside it.");
        println!("   Looking for other TypeScript files in the assembly directory...");

        // Look for any .ts files in the assembly directory
        let mut found_ts = false;
        if let Ok(entries) = fs::read_dir(&assembly_dir) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "ts" {
                        println!("✅ Found TypeScript file: {}", entry.path().display());
                        found_ts = true;
                    }
                }
            }
        }

        if !found_ts {
            return Err("No TypeScript source files found in assembly directory".to_string());
        }
    } else {
        println!("✅ Found assembly/index.ts entry point");
    }

    // Create output directory if it doesn't exist
    let output_path = Path::new(output_dir);
    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // First, try to build with npm run asbuild (preferred method)
    println!("🔨 Trying to build with 'npm run asbuild'...");
    println!("📝 Log output:");

    let npm_build_status = Command::new("npm")
        .current_dir(project_path)
        .args(["run", "asbuild"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    if let Ok(status) = npm_build_status {
        if status.success() {
            println!("✅ npm run asbuild completed successfully");

            // Look for the output in standard locations
            let mut wasm_file = None;

            // Common output directories
            let output_dirs = [
                Path::new(project_path).join("build"),
                Path::new(project_path).join("dist"),
                Path::new(project_path).join("out"),
                Path::new(project_path).to_path_buf(),
            ];

            // Search for WASM files
            for dir in &output_dirs {
                if dir.exists() && dir.is_dir() {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            if let Some(extension) = entry.path().extension() {
                                if extension == "wasm" {
                                    wasm_file = Some(entry.path());
                                    println!("✅ Found WASM file: {}", entry.path().display());
                                    break;
                                }
                            }
                        }
                    }
                }

                if wasm_file.is_some() {
                    break;
                }
            }

            if let Some(wasm_path) = wasm_file {
                // Copy the wasm file to the output directory
                let output_file = output_path.join(wasm_path.file_name().unwrap());
                fs::copy(&wasm_path, &output_file)
                    .map_err(|e| format!("Failed to copy WASM file: {}", e))?;

                println!(
                    "📦 WASM file copied to: {} ({} bytes)",
                    output_file.display(),
                    fs::metadata(&output_file).map(|m| m.len()).unwrap_or(0)
                );

                return Ok(output_file.to_string_lossy().to_string());
            }
        }
    }

    // If npm run asbuild failed or didn't produce a WASM file, try direct npx asc command
    println!("⚙️ Trying to build directly with npx asc...");

    // Create build directory if it doesn't exist
    let build_dir = Path::new(project_path).join("build");
    if !build_dir.exists() {
        fs::create_dir_all(&build_dir)
            .map_err(|e| format!("Failed to create build directory: {}", e))?;
        println!("✅ Created build directory: {}", build_dir.display());
    }

    // Set up output file path
    let wasm_output = build_dir.join("release.wasm");

    // Determine source file (entry point)
    let entry_point = if index_ts.exists() {
        "assembly/index.ts".to_string()
    } else {
        // Find any .ts file in assembly directory
        let mut ts_file = String::new();
        if assembly_dir.exists() {
            if let Ok(entries) = fs::read_dir(&assembly_dir) {
                for entry in entries.flatten() {
                    if let Some(extension) = entry.path().extension() {
                        if extension == "ts" {
                            ts_file = format!(
                                "assembly/{}",
                                entry.path().file_name().unwrap().to_string_lossy()
                            );
                            break;
                        }
                    }
                }
            }
        }

        // If no ts file found in assembly directory, look in root
        if ts_file.is_empty() {
            if let Ok(entries) = fs::read_dir(project_path) {
                for entry in entries.flatten() {
                    if let Some(extension) = entry.path().extension() {
                        if extension == "ts" {
                            ts_file = entry
                                .path()
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .to_string();
                            break;
                        }
                    }
                }
            }
        }

        if ts_file.is_empty() {
            return Err("No TypeScript source files found in project".to_string());
        }

        ts_file
    };

    println!(
        "🔨 Building with command: npx asc {} --optimize --outFile {}",
        entry_point,
        wasm_output.file_name().unwrap().to_string_lossy()
    );
    println!("📝 Log output:");

    // Build with npx asc
    let build_status = Command::new("npx")
        .current_dir(project_path)
        .args([
            "asc",
            &entry_point,
            "--optimize",
            "--outFile",
            wasm_output.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to execute npx asc: {}", e))?;

    if !build_status.success() {
        return Err("Build failed. See output above for details.".to_string());
    }

    println!("✅ AssemblyScript build completed successfully");

    // Check if the WASM file was created
    if !wasm_output.exists() {
        return Err(format!(
            "AssemblyScript build completed but WASM file was not created at expected location: {}",
            wasm_output.display()
        ));
    }

    // Copy the wasm file to the output directory
    let output_file = output_path.join(wasm_output.file_name().unwrap());
    fs::copy(&wasm_output, &output_file).map_err(|e| format!("Failed to copy WASM file: {}", e))?;

    println!(
        "📦 WASM file created: {} ({} bytes)",
        output_file.display(),
        fs::metadata(&output_file).map(|m| m.len()).unwrap_or(0)
    );

    Ok(output_file.to_string_lossy().to_string())
}
