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
