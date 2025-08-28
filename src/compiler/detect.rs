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
    if tool_name == "wasm-pack" {
        let check_command = if cfg!(target_os = "windows") {
            "where wasm-pack"
        } else {
            "which wasm-pack"
        };

        let wasm_pack_installed = std::process::Command::new(if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        })
        .args(if cfg!(target_os = "windows") {
            ["/c", check_command]
        } else {
            ["-c", check_command]
        })
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

        if !wasm_pack_installed {
            println!("⚠️ wasm-pack is not installed. It's required for wasm-bindgen projects.");
            println!("  To install wasm-pack, run: cargo install wasm-pack");
        }

        return wasm_pack_installed;
    }

    let command = if cfg!(target_os = "windows") {
        format!("where {tool_name}")
    } else {
        format!("which {tool_name}")
    };

    std::process::Command::new(if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    })
    .args(if cfg!(target_os = "windows") {
        ["/c", &command]
    } else {
        ["-c", &command]
    })
    .output()
    .map(|output| output.status.success())
    .unwrap_or(false)
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
