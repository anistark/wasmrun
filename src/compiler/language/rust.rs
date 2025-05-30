use crate::compiler::builder::{
    BuildConfig, BuildResult, OptimizationLevel, TargetType, WasmBuilder,
};
use crate::utils::PathResolver;
use std::fs;
use std::path::Path;

pub struct RustBuilder;

impl RustBuilder {
    pub fn new() -> Self {
        Self
    }
}

impl WasmBuilder for RustBuilder {
    fn language_name(&self) -> &str {
        "Rust"
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn supported_extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !self.is_tool_installed("rustc") {
            missing.push("rustc (Rust compiler)".to_string());
        }

        if !self.is_tool_installed("cargo") {
            missing.push("cargo (Rust package manager)".to_string());
        }

        // Check for wasm32 target
        let check_target = std::process::Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output();

        if let Ok(output) = check_target {
            let target_output = String::from_utf8_lossy(&output.stdout);
            if !target_output.contains("wasm32-unknown-unknown") {
                missing.push("wasm32-unknown-unknown target (install with: rustup target add wasm32-unknown-unknown)".to_string());
            }
        }

        // Check for wasm-pack if this looks like a wasm-bindgen project
        if self.uses_wasm_bindgen(&BuildConfig::default().project_path)
            && !self.is_tool_installed("wasm-pack")
        {
            missing.push("wasm-pack (install with: cargo install wasm-pack)".to_string());
        }

        missing
    }

    fn validate_project(&self, project_path: &str) -> Result<(), String> {
        PathResolver::validate_directory_exists(project_path)?;

        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");
        if !Path::new(&cargo_toml_path).exists() {
            return Err(format!("No Cargo.toml found in {}", project_path));
        }

        Ok(())
    }

    fn build(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        // Check if this is a wasm-bindgen project
        if self.uses_wasm_bindgen(&config.project_path) {
            self.build_wasm_bindgen(config)
        } else if self.is_rust_web_application(&config.project_path) {
            self.build_web_application(config)
        } else {
            self.build_standard_wasm(config)
        }
    }
}

impl RustBuilder {
    /// Check if a Rust project uses wasm-bindgen
    fn uses_wasm_bindgen(&self, project_path: &str) -> bool {
        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");

        if let Ok(cargo_toml) = fs::read_to_string(cargo_toml_path) {
            cargo_toml.contains("wasm-bindgen")
                || cargo_toml.contains("web-sys")
                || cargo_toml.contains("js-sys")
        } else {
            false
        }
    }

    /// Check if a project is a Rust web application
    fn is_rust_web_application(&self, project_path: &str) -> bool {
        let cargo_toml_path = PathResolver::join_paths(project_path, "Cargo.toml");

        if let Ok(cargo_toml) = fs::read_to_string(cargo_toml_path) {
            let uses_wasm_bindgen = self.uses_wasm_bindgen(project_path);

            if !uses_wasm_bindgen {
                return false;
            }

            // Look for web framework dependencies
            let web_frameworks = [
                "yew", "leptos", "dioxus", "sycamore", "mogwai", "seed", "percy", "iced", "dodrio",
                "smithy", "trunk",
            ];

            for framework in web_frameworks {
                if cargo_toml.contains(framework) {
                    return true;
                }
            }

            // Check for lib target with cdylib
            if cargo_toml.contains("[lib]") && cargo_toml.contains("cdylib") {
                // Check if there's an index.html in the project
                if Path::new(project_path).join("index.html").exists() {
                    return true;
                }

                // Check for static directories that might indicate a web app
                let potential_static_dirs = ["public", "static", "assets", "dist", "www"];
                for dir in potential_static_dirs {
                    if Path::new(project_path).join(dir).exists() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Install wasm32 target if not present
    fn ensure_wasm32_target(&self) -> Result<(), String> {
        let check_target =
            self.execute_command("rustup", &["target", "list", "--installed"], ".", false)?;
        let target_output = String::from_utf8_lossy(&check_target.stdout);

        if !target_output.contains("wasm32-unknown-unknown") {
            println!("⚙️ Installing wasm32-unknown-unknown target...");
            self.execute_command_with_output(
                "rustup",
                &["target", "add", "wasm32-unknown-unknown"],
                ".",
            )?;
            println!("✅ wasm32-unknown-unknown target installed");
        }

        Ok(())
    }

    /// Build standard WASM without wasm-bindgen
    fn build_standard_wasm(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        // Ensure wasm32 target is installed
        self.ensure_wasm32_target()?;

        // Determine build args based on optimization level
        let mut args = vec!["build", "--target", "wasm32-unknown-unknown"];

        match config.optimization_level {
            OptimizationLevel::Release => args.push("--release"),
            OptimizationLevel::Size => {
                args.push("--release");
                // TODO: Add size optimization flags
            }
            OptimizationLevel::Debug => {} // Default debug build
        }

        // Execute cargo build
        if config.verbose {
            self.execute_command_with_output("cargo", &args, &config.project_path)?;
        } else {
            let output = self.execute_command("cargo", &args, &config.project_path, false)?;
            if !output.status.success() {
                return Err(format!(
                    "Cargo build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Find the generated WASM file
        let build_type = match config.optimization_level {
            OptimizationLevel::Debug => "debug",
            _ => "release",
        };

        let target_dir = PathResolver::join_paths(
            &config.project_path,
            &format!("target/wasm32-unknown-unknown/{}", build_type),
        );

        let wasm_files = PathResolver::find_files_with_extension(&target_dir, "wasm")?;

        if wasm_files.is_empty() {
            return Err("No WASM file found in target directory".to_string());
        }

        let wasm_file = &wasm_files[0]; // Take the first one
        let output_path = self.copy_to_output(wasm_file, &config.output_dir)?;

        Ok(BuildResult {
            wasm_path: output_path,
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    /// Build wasm-bindgen project
    fn build_wasm_bindgen(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        // Check if wasm-pack is installed
        if !self.is_tool_installed("wasm-pack") {
            return Err(
                "wasm-pack is not installed. Install with: cargo install wasm-pack".to_string(),
            );
        }

        // Ensure wasm32 target is installed
        self.ensure_wasm32_target()?;

        // Determine wasm-pack args
        let mut args = vec!["build", "--target", "web"];

        match config.optimization_level {
            OptimizationLevel::Debug => args.push("--dev"),
            OptimizationLevel::Release => {} // Default is release
            OptimizationLevel::Size => {
                // TODO: Add size optimization flags for wasm-pack
            }
        }

        // Add no-typescript flag to simplify output
        args.push("--no-typescript");

        // Execute wasm-pack build
        if config.verbose {
            self.execute_command_with_output("wasm-pack", &args, &config.project_path)?;
        } else {
            let output = self.execute_command("wasm-pack", &args, &config.project_path, false)?;
            if !output.status.success() {
                return Err(format!(
                    "wasm-pack build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Find generated files in pkg directory
        let pkg_dir = PathResolver::join_paths(&config.project_path, "pkg");

        let wasm_files = PathResolver::find_files_with_extension(&pkg_dir, "wasm")?;
        let js_files = PathResolver::find_files_with_extension(&pkg_dir, "js")?;

        if wasm_files.is_empty() {
            return Err("No WASM file found in pkg directory".to_string());
        }

        // Find the main JS file (not .d.js files)
        let main_js_file = js_files
            .iter()
            .find(|path| !path.contains(".d.js"))
            .ok_or("No main JS file found in pkg directory")?;

        // Copy files to output directory
        let wasm_output = self.copy_to_output(&wasm_files[0], &config.output_dir)?;
        let js_output = self.copy_to_output(main_js_file, &config.output_dir)?;

        // Copy any additional files (.d.ts, etc.)
        let mut additional_files = Vec::new();
        let all_pkg_files =
            fs::read_dir(&pkg_dir).map_err(|e| format!("Failed to read pkg directory: {}", e))?;

        for entry in all_pkg_files.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "ts" || ext_str == "json" {
                    let copied_file =
                        self.copy_to_output(&path.to_string_lossy(), &config.output_dir)?;
                    additional_files.push(copied_file);
                }
            }
        }

        Ok(BuildResult {
            wasm_path: wasm_output,
            js_path: Some(js_output),
            additional_files,
            is_wasm_bindgen: true,
        })
    }

    /// Build Rust web application (like Yew, Leptos, etc.)
    fn build_web_application(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        // Check if this project uses Trunk
        let uses_trunk = Path::new(&config.project_path).join("Trunk.toml").exists()
            || Path::new(&config.project_path).join("trunk.toml").exists();

        if uses_trunk {
            self.build_with_trunk(config)
        } else {
            // Fall back to wasm-pack
            self.build_wasm_bindgen(config)
        }
    }

    /// Build with Trunk (for web frameworks like Yew)
    fn build_with_trunk(&self, config: &BuildConfig) -> Result<BuildResult, String> {
        if !self.is_tool_installed("trunk") {
            return Err("Trunk is not installed. Install with: cargo install trunk".to_string());
        }

        // Determine trunk args
        let mut args = vec!["build"];

        match config.optimization_level {
            OptimizationLevel::Release => args.push("--release"),
            OptimizationLevel::Debug => {} // Default debug build
            OptimizationLevel::Size => {
                args.push("--release");
                // TODO: Add size optimization flags
            }
        }

        // Execute trunk build
        if config.verbose {
            self.execute_command_with_output("trunk", &args, &config.project_path)?;
        } else {
            let output = self.execute_command("trunk", &args, &config.project_path, false)?;
            if !output.status.success() {
                return Err(format!(
                    "Trunk build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Copy the dist directory to output
        let trunk_dist = PathResolver::join_paths(&config.project_path, "dist");
        if !Path::new(&trunk_dist).exists() {
            return Err("Trunk build completed but dist directory was not created".to_string());
        }

        // Find the main files
        let wasm_files = PathResolver::find_files_with_extension(&trunk_dist, "wasm")?;
        let js_files = PathResolver::find_files_with_extension(&trunk_dist, "js")?;

        if wasm_files.is_empty() {
            return Err("No WASM file found in trunk dist directory".to_string());
        }

        // Copy dist directory contents to output
        copy_directory_recursively(&trunk_dist, &config.output_dir)?;

        // Return relative paths within the output directory
        let wasm_filename = PathResolver::get_filename(&wasm_files[0])?;
        let js_filename = if !js_files.is_empty() {
            Some(PathResolver::get_filename(&js_files[0])?)
        } else {
            None
        };

        Ok(BuildResult {
            wasm_path: PathResolver::join_paths(&config.output_dir, &wasm_filename),
            js_path: js_filename.map(|name| PathResolver::join_paths(&config.output_dir, &name)),
            additional_files: vec![], // TODO: Track all copied files
            is_wasm_bindgen: true,
        })
    }
}

/// Copy directory recursively (standalone function to avoid clippy warning)
fn copy_directory_recursively(source: &str, destination: &str) -> Result<(), String> {
    PathResolver::ensure_output_directory(destination)?;

    let entries = fs::read_dir(source)
        .map_err(|e| format!("Failed to read source directory {}: {}", source, e))?;

    for entry in entries.flatten() {
        let source_path = entry.path();
        let file_name = source_path
            .file_name()
            .ok_or_else(|| "Invalid file name".to_string())?;
        let destination_path = Path::new(destination).join(file_name);

        if source_path.is_dir() {
            copy_directory_recursively(
                &source_path.to_string_lossy(),
                &destination_path.to_string_lossy(),
            )?;
        } else {
            fs::copy(&source_path, &destination_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}

/// Check if a Rust project uses wasm-bindgen
pub fn uses_wasm_bindgen(project_path: &str) -> bool {
    RustBuilder::new().uses_wasm_bindgen(project_path)
}

/// Check if a project is a Rust web application
pub fn is_rust_web_application(project_path: &str) -> bool {
    RustBuilder::new().is_rust_web_application(project_path)
}

/// Build a web application from a Rust project
pub fn build_rust_web_application(project_path: &str, output_dir: &str) -> Result<String, String> {
    let config = BuildConfig {
        project_path: project_path.to_string(),
        output_dir: output_dir.to_string(),
        verbose: true,
        optimization_level: OptimizationLevel::Release,
        target_type: TargetType::WebApp,
    };

    let builder = RustBuilder::new();
    let result = builder.build(&config)?;

    // Return the JS file path for web applications, or WASM path if no JS
    Ok(result.js_path.unwrap_or(result.wasm_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_rust_builder_language_name() {
        let builder = RustBuilder::new();
        assert_eq!(builder.language_name(), "Rust");
    }

    #[test]
    fn test_rust_builder_entry_candidates() {
        let builder = RustBuilder::new();
        assert!(builder.entry_file_candidates().contains(&"Cargo.toml"));
    }

    #[test]
    fn test_validate_project_success() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create Cargo.toml
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        let builder = RustBuilder::new();
        assert!(builder.validate_project(project_path).is_ok());
    }

    #[test]
    fn test_validate_project_missing_cargo_toml() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        let builder = RustBuilder::new();
        let result = builder.validate_project(project_path);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No Cargo.toml found"));
    }

    #[test]
    fn test_uses_wasm_bindgen_detection() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create Cargo.toml with wasm-bindgen dependency
        let cargo_toml = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
wasm-bindgen = "0.2"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml).unwrap();

        let builder = RustBuilder::new();
        assert!(builder.uses_wasm_bindgen(project_path));
    }

    #[test]
    fn test_web_application_detection() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().to_str().unwrap();

        // Create Cargo.toml with yew dependency
        let cargo_toml = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
yew = "0.20"
wasm-bindgen = "0.2"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml).unwrap();

        let builder = RustBuilder::new();
        assert!(builder.is_rust_web_application(project_path));
    }
}
