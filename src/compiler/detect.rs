use crate::{debug_enter, debug_exit, debug_println};
use std::fmt;
use std::fs;
use std::path::Path;

/// Supported project languages
#[derive(Debug, PartialEq)]
pub enum ProjectLanguage {
    Rust,
    Go,
    C,
    Asc,
    Python,
    Unknown,
}

/// Supported OS
#[derive(Debug, PartialEq)]
#[allow(dead_code)] // TODO: Future OS-specific compilation features
pub enum OperatingSystem {
    Windows,
    MacOS,
    Linux,
    Other,
}

/// Detect project language
pub fn detect_project_language(project_path: &str) -> ProjectLanguage {
    debug_enter!("detect_project_language", "project_path={}", project_path);
    let path = Path::new(project_path);

    if !path.exists() || !path.is_dir() {
        debug_println!(
            "Project path validation failed: exists={}, is_dir={}",
            path.exists(),
            path.is_dir()
        );
        eprintln!("❌ Project path does not exist or is not a directory: {project_path}");
        debug_exit!("detect_project_language", ProjectLanguage::Unknown);
        return ProjectLanguage::Unknown;
    }

    debug_println!("Checking for language-specific configuration files");
    if path.join("Cargo.toml").exists() {
        debug_println!("Found Cargo.toml - detected Rust project");
        debug_exit!("detect_project_language", ProjectLanguage::Rust);
        return ProjectLanguage::Rust;
    }

    if path.join("go.mod").exists() {
        debug_println!("Found go.mod - detected Go project");
        debug_exit!("detect_project_language", ProjectLanguage::Go);
        return ProjectLanguage::Go;
    } else if let Ok(entries) = fs::read_dir(path) {
        debug_println!("Scanning directory for Go source files");
        for entry in entries.flatten() {
            if let Some(extension) = entry.path().extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "go" {
                    debug_println!("Found .go file - detected Go project");
                    debug_exit!("detect_project_language", ProjectLanguage::Go);
                    return ProjectLanguage::Go;
                }
            }
        }
    }

    if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        return ProjectLanguage::Python;
    } else if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(extension) = entry.path().extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "py" {
                    return ProjectLanguage::Python;
                }
            }
        }
    }

    if let Ok(package_json) = fs::read_to_string(path.join("package.json")) {
        if package_json.contains("\"asc\"") {
            return ProjectLanguage::Asc;
        }
    }

    let mut has_c_files = false;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(extension) = entry.path().extension() {
                let ext = extension.to_string_lossy().to_lowercase();

                if ext == "c" {
                    has_c_files = true;
                }
            }
        }
    }

    if has_c_files {
        return ProjectLanguage::C;
    }

    ProjectLanguage::Unknown
}

/// Detect the OS Wasmrun is running on
pub fn detect_operating_system() -> OperatingSystem {
    #[cfg(target_os = "windows")]
    {
        return OperatingSystem::Windows;
    }

    #[cfg(target_os = "macos")]
    {
        return OperatingSystem::MacOS;
    }

    #[cfg(target_os = "linux")]
    {
        return OperatingSystem::Linux;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        OperatingSystem::Other
    }

    #[allow(unreachable_code)]
    OperatingSystem::Other
}

/// Get recommended compilation tools based on OS and language.
pub fn get_recommended_tools(language: &ProjectLanguage, os: &OperatingSystem) -> Vec<String> {
    let recommended_tools = match (language, os) {
        (ProjectLanguage::Rust, _) => {
            vec![
                "rustup".to_string(),
                "cargo".to_string(),
                "External Rust plugin (install with: wasmrun plugin install wasmrust)".to_string(),
            ]
        }
        (ProjectLanguage::Go, _) => {
            vec![
                "External Go plugin (install with: wasmrun plugin install wasmgo)".to_string(),
                "tinygo".to_string(),
            ]
        }
        (ProjectLanguage::C, OperatingSystem::Windows) => {
            vec![
                "emscripten".to_string(),
                "mingw-w64".to_string(),
                "msvc".to_string(),
            ]
        }
        (ProjectLanguage::C, _) => {
            vec![
                "emscripten".to_string(),
                "clang".to_string(),
                "gcc".to_string(),
            ]
        }
        (ProjectLanguage::Asc, _) => {
            vec!["node.js".to_string(), "npm".to_string(), "asc".to_string()]
        }
        (ProjectLanguage::Python, _) => Vec::new(),
        (ProjectLanguage::Unknown, _) => Vec::new(),
    };

    recommended_tools
        .into_iter()
        .filter(|tool| !is_tool_installed(tool))
        .collect()
}

/// Check if a tool is installed and available in the system path
pub fn is_tool_installed(tool_name: &str) -> bool {
    use crate::utils::CommandExecutor;

    let is_installed = CommandExecutor::is_tool_installed(tool_name);

    if tool_name == "wasm-pack" && !is_installed {
        println!("⚠️ wasm-pack is not installed. It's required for wasm-bindgen projects.");
        println!("  To install wasm-pack, run: cargo install wasm-pack");
    }

    is_installed
}

/// Get missing tools for a given language
pub fn get_missing_tools(language: &ProjectLanguage, os: &OperatingSystem) -> Vec<String> {
    get_recommended_tools(language, os)
}

/// Print system information
pub fn print_system_info() {
    let os = detect_operating_system();
    println!("💻 System: {os:?}");
}

impl fmt::Display for ProjectLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lang_str = match self {
            ProjectLanguage::Rust => "Rust",
            ProjectLanguage::Go => "Go",
            ProjectLanguage::C => "C",
            ProjectLanguage::Asc => "Asc",
            ProjectLanguage::Python => "Python",
            ProjectLanguage::Unknown => "Unknown",
        };
        write!(f, "{lang_str}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_test_file(dir: &std::path::Path, filename: &str, content: &str) {
        let file_path = dir.join(filename);
        let mut file = File::create(file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_detect_rust_project_with_cargo_toml() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "Cargo.toml", "[package]\nname = \"test\"");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Rust);
    }

    #[test]
    fn test_detect_go_project_with_go_mod() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "go.mod", "module test");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Go);
    }

    #[test]
    fn test_detect_go_project_with_go_files() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "main.go", "package main\n\nfunc main() {}");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Go);
    }

    #[test]
    fn test_detect_python_project_with_pyproject_toml() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "pyproject.toml", "[build-system]");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Python);
    }

    #[test]
    fn test_detect_python_project_with_setup_py() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "setup.py", "from setuptools import setup");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Python);
    }

    #[test]
    fn test_detect_python_project_with_py_files() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "main.py", "print('hello')");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Python);
    }

    #[test]
    fn test_detect_asc_project_with_package_json() {
        let temp_dir = tempdir().unwrap();
        create_test_file(
            temp_dir.path(),
            "package.json",
            r#"{"scripts": {"asc": "asc"}}"#,
        );

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Asc);
    }

    #[test]
    fn test_detect_c_project_with_c_files() {
        let temp_dir = tempdir().unwrap();
        create_test_file(
            temp_dir.path(),
            "main.c",
            "#include <stdio.h>\nint main() { return 0; }",
        );

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::C);
    }

    #[test]
    fn test_detect_unknown_project() {
        let temp_dir = tempdir().unwrap();
        create_test_file(temp_dir.path(), "readme.txt", "This is a readme");

        let result = detect_project_language(temp_dir.path().to_str().unwrap());
        assert_eq!(result, ProjectLanguage::Unknown);
    }

    #[test]
    fn test_detect_nonexistent_directory() {
        let result = detect_project_language("/nonexistent/directory");
        assert_eq!(result, ProjectLanguage::Unknown);
    }

    #[test]
    fn test_detect_operating_system() {
        let os = detect_operating_system();
        // We can't test specific OS values since tests run on different platforms
        // But we can ensure it returns a valid enum variant
        match os {
            OperatingSystem::Windows
            | OperatingSystem::MacOS
            | OperatingSystem::Linux
            | OperatingSystem::Other => {
                // Valid OS detected
            }
        }
    }

    #[test]
    fn test_get_recommended_tools_rust() {
        let tools = get_recommended_tools(&ProjectLanguage::Rust, &OperatingSystem::Linux);
        // Since tool installation depends on the system, we just check structure
        assert!(tools
            .iter()
            .any(|t| t.contains("cargo") || t.contains("rustup") || t.contains("Rust")));
    }

    #[test]
    fn test_get_recommended_tools_go() {
        let tools = get_recommended_tools(&ProjectLanguage::Go, &OperatingSystem::Linux);
        assert!(tools
            .iter()
            .any(|t| t.contains("tinygo") || t.contains("Go")));
    }

    #[test]
    fn test_get_recommended_tools_c_linux() {
        let tools = get_recommended_tools(&ProjectLanguage::C, &OperatingSystem::Linux);
        assert!(tools
            .iter()
            .any(|t| t.contains("emscripten") || t.contains("clang") || t.contains("gcc")));
    }

    #[test]
    fn test_get_recommended_tools_c_windows() {
        let tools = get_recommended_tools(&ProjectLanguage::C, &OperatingSystem::Windows);
        assert!(tools
            .iter()
            .any(|t| t.contains("emscripten") || t.contains("mingw") || t.contains("msvc")));
    }

    #[test]
    fn test_get_recommended_tools_asc() {
        let tools = get_recommended_tools(&ProjectLanguage::Asc, &OperatingSystem::Linux);
        assert!(tools
            .iter()
            .any(|t| t.contains("node") || t.contains("npm") || t.contains("asc")));
    }

    #[test]
    fn test_get_recommended_tools_unknown() {
        let tools = get_recommended_tools(&ProjectLanguage::Unknown, &OperatingSystem::Linux);
        assert!(tools.is_empty());
    }

    #[test]
    fn test_is_tool_installed() {
        // Test with a tool that should exist on most systems
        assert!(is_tool_installed("echo"));

        // Test with a tool that shouldn't exist
        assert!(!is_tool_installed("nonexistent_tool_12345"));
    }

    #[test]
    fn test_get_missing_tools() {
        let missing = get_missing_tools(&ProjectLanguage::Unknown, &OperatingSystem::Linux);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_project_language_display() {
        assert_eq!(format!("{}", ProjectLanguage::Rust), "Rust");
        assert_eq!(format!("{}", ProjectLanguage::Go), "Go");
        assert_eq!(format!("{}", ProjectLanguage::C), "C");
        assert_eq!(format!("{}", ProjectLanguage::Asc), "Asc");
        assert_eq!(format!("{}", ProjectLanguage::Python), "Python");
        assert_eq!(format!("{}", ProjectLanguage::Unknown), "Unknown");
    }

    #[test]
    fn test_project_language_partial_eq() {
        assert_eq!(ProjectLanguage::Rust, ProjectLanguage::Rust);
        assert_ne!(ProjectLanguage::Rust, ProjectLanguage::Go);
    }

    #[test]
    fn test_operating_system_partial_eq() {
        assert_eq!(OperatingSystem::Linux, OperatingSystem::Linux);
        assert_ne!(OperatingSystem::Linux, OperatingSystem::Windows);
    }
}
