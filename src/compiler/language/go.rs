use crate::compiler::builder::{BuildConfig, BuildResult, WasmBuilder};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct GoBuilder;

impl GoBuilder {
    pub fn new() -> Self {
        Self
    }
}

impl WasmBuilder for GoBuilder {
    fn language_name(&self) -> &str {
        "Go"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.go", "cmd/main.go", "app.go", "go.mod"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["go"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !self.is_tool_installed("tinygo") {
            missing.push("tinygo (install from https://tinygo.org)".to_string());
        }

        if !self.is_tool_installed("go") {
            missing.push("go (Go compiler)".to_string());
        }

        missing
    }

    fn build(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        // For now, call the existing build_wasm function
        // TODO: Refactor the existing Go build code to use this pattern
        let wasm_path = build_wasm(&config.project_path, &config.output_dir)?;

        Ok(BuildResult {
            wasm_path,
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }
}

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
