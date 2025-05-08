use std::fs;
use std::path::Path;
use std::process::Command;

/// Build a WASM file from a C project using Emscripten
pub fn build_wasm(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🅲 Building WASM from C project at: {}", project_path);

    // Check if emcc is installed
    let check_emcc = Command::new("emcc").arg("--version").output();

    if check_emcc.is_err() {
        return Err(
            "Emscripten (emcc) is not installed or not in PATH. Please install Emscripten."
                .to_string(),
        );
    }

    // Find main.c or similar entry point
    let mut entry_file = None;
    let common_entry_files = ["main.c", "index.c", "app.c"];

    for entry_name in common_entry_files.iter() {
        let entry_path = Path::new(project_path).join(entry_name);
        if entry_path.exists() {
            entry_file = Some(entry_path);
            break;
        }
    }

    // If no common entry file found, look for any .c file
    if entry_file.is_none() {
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "c" {
                        entry_file = Some(entry.path());
                        break;
                    }
                }
            }
        }
    }

    let entry_path =
        entry_file.ok_or_else(|| "No C source files found in project directory".to_string())?;

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

    // Build with emcc
    println!("🔨 Building with Emscripten...");
    let build_output = Command::new("emcc")
        .current_dir(project_path)
        .args([
            "-O2",
            entry_path.to_str().unwrap(),
            "-o",
            output_file.to_str().unwrap(),
            "-s",
            "WASM=1",
            "-s",
            "STANDALONE_WASM=1",
        ])
        .output()
        .map_err(|e| format!("Failed to run emcc: {}", e))?;

    if !build_output.status.success() {
        return Err(format!(
            "Build failed: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    Ok(output_file.to_string_lossy().to_string())
}

/// Build a WASM file from a C project with more detailed output
pub fn build_wasm_verbose(project_path: &str, output_dir: &str) -> Result<String, String> {
    println!("🅲 Building WASM from C project at: {}", project_path);

    // Check if emcc is installed
    let check_emcc = Command::new("emcc").arg("--version").output();

    if check_emcc.is_err() {
        return Err(
            "Emscripten (emcc) is not installed or not in PATH. Please install Emscripten."
                .to_string(),
        );
    }

    // Print Emscripten version for debugging
    if let Ok(version_output) = check_emcc {
        if version_output.status.success() {
            let version = String::from_utf8_lossy(&version_output.stdout);
            println!("✅ Using Emscripten version:");
            println!("{}", version.trim());
        }
    }

    // Check for Makefile which might indicate a more complex project
    let makefile_path = Path::new(project_path).join("Makefile");
    let cmake_path = Path::new(project_path).join("CMakeLists.txt");

    if makefile_path.exists() {
        println!("📋 Found Makefile, this might be a Make-based project");
        // TODO: Add support for Make-based C projects
    } else if cmake_path.exists() {
        println!("📋 Found CMakeLists.txt, this might be a CMake-based project");
        // TODO: Add support for CMake-based C projects
    }

    // Find main.c or similar entry point
    let mut entry_file = None;
    let common_entry_files = ["main.c", "index.c", "app.c"];

    for entry_name in common_entry_files.iter() {
        let entry_path = Path::new(project_path).join(entry_name);
        if entry_path.exists() {
            println!("✅ Found entry point: {}", entry_path.display());
            entry_file = Some(entry_path);
            break;
        }
    }

    // If no common entry file found, look for any .c file
    if entry_file.is_none() {
        println!("⚠️ No common entry file found, searching for any .c file...");
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "c" {
                        println!("✅ Found C file: {}", entry.path().display());
                        entry_file = Some(entry.path());
                        break;
                    }
                }
            }
        }
    }

    let entry_path =
        entry_file.ok_or_else(|| "No C source files found in project directory".to_string())?;

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

    // Look for additional source files and include directories
    let mut source_files = Vec::new();
    let mut include_dirs = Vec::new();

    // Add the main source file
    source_files.push(entry_path.to_string_lossy().to_string());

    // Check for include directory
    let include_dir = Path::new(project_path).join("include");
    if include_dir.exists() && include_dir.is_dir() {
        include_dirs.push(format!("-I{}", include_dir.display()));
        println!("✅ Found include directory: {}", include_dir.display());
    }

    // Check for src directory for additional source files
    let src_dir = Path::new(project_path).join("src");
    if src_dir.exists() && src_dir.is_dir() {
        println!("✅ Found src directory: {}", src_dir.display());
        if let Ok(entries) = fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "c" {
                        source_files.push(entry.path().to_string_lossy().to_string());
                        println!("  + Adding source file: {}", entry.path().display());
                    }
                }
            }
        }
    } else {
        // Look for other .c files in the same directory
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if entry.path() == entry_path {
                    continue; // Skip the main file we already added
                }

                if let Some(extension) = entry.path().extension() {
                    if extension == "c" {
                        source_files.push(entry.path().to_string_lossy().to_string());
                        println!("  + Adding source file: {}", entry.path().display());
                    }
                }
            }
        }
    }

    // Build command arguments
    let mut args = Vec::new();

    // Add optimization level
    args.push("-O2");

    // Add all source files
    for source in &source_files {
        args.push(source);
    }

    // Add include directories
    for include in &include_dirs {
        args.push(include);
    }

    // Add output file
    args.push("-o");
    args.push(output_file.to_str().unwrap());

    // Add WASM-specific flags
    args.push("-s");
    args.push("WASM=1");
    args.push("-s");
    args.push("STANDALONE_WASM=1");

    // Print compilation command
    println!("🔨 Building with command: emcc {}", args.join(" "));
    println!("📝 Log output:");

    // Build with emcc with inherit stdout/stderr for better user feedback
    let build_status = Command::new("emcc")
        .current_dir(project_path)
        .args(&args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to execute emcc: {}", e))?;

    if !build_status.success() {
        return Err("Build failed. See output above for details.".to_string());
    }

    println!("✅ Emscripten build completed successfully");

    // Check if the WASM file was created
    if !output_file.exists() {
        return Err(format!(
            "Emscripten build completed but WASM file was not created at expected location: {}",
            output_file.display()
        ));
    }

    println!(
        "📦 WASM file created: {} ({} bytes)",
        output_file.display(),
        fs::metadata(&output_file).map(|m| m.len()).unwrap_or(0)
    );

    // Check for other generated files
    let js_file = output_file.with_extension("js");
    if js_file.exists() {
        println!(
            "📝 JavaScript glue file created: {} ({} bytes)",
            js_file.display(),
            fs::metadata(&js_file).map(|m| m.len()).unwrap_or(0)
        );

        println!(
            "ℹ️ Note: Chakra currently uses only the .wasm file, not the JavaScript glue code"
        );
    }

    Ok(output_file.to_string_lossy().to_string())
}
