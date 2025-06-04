use std::fs;
use std::path::Path;

/// Enum for supported project languages
#[derive(Debug, PartialEq)]
pub enum ProjectLanguage {
    Rust,
    Go,
    C,
    AssemblyScript,
    Python,
    Unknown,
}

/// Enum for supported operating systems
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub enum OperatingSystem {
    Windows,
    MacOS,
    Linux,
    Other,
}

/// Detect the project language
pub fn detect_project_language(project_path: &str) -> ProjectLanguage {
    let path = Path::new(project_path);

    // Check if the path exists and is a directory
    if !path.exists() || !path.is_dir() {
        eprintln!(
            "❌ Project path does not exist or is not a directory: {}",
            project_path
        );
        return ProjectLanguage::Unknown;
    }

    // Check for Rust project (Cargo.toml)
    if path.join("Cargo.toml").exists() {
        return ProjectLanguage::Rust;
    }

    // Check for Go project (go.mod or .go files)
    if path.join("go.mod").exists() {
        return ProjectLanguage::Go;
    } else {
        // Look for .go files
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "go" {
                        return ProjectLanguage::Go;
                    }
                }
            }
        }
    }

    // Check for Python project (pyproject.toml, setup.py, or .py files)
    if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        return ProjectLanguage::Python;
    } else {
        // Look for .py files
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "py" {
                        return ProjectLanguage::Python;
                    }
                }
            }
        }
    }

    // Check for AssemblyScript project (package.json with assemblyscript dependency)
    if let Ok(package_json) = fs::read_to_string(path.join("package.json")) {
        if package_json.contains("\"assemblyscript\"") {
            return ProjectLanguage::AssemblyScript;
        }
    }

    // Check for C project (.c files)
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

    // Default to unknown
    ProjectLanguage::Unknown
}

/// Detect the OS Chakra is running on
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

    // Fallback to Other if no specific OS is detected
    #[allow(unreachable_code)]
    OperatingSystem::Other
}

/// Get recommended compilation tools based on OS and language.
pub fn get_recommended_tools(language: &ProjectLanguage, os: &OperatingSystem) -> Vec<String> {
    let recommended_tools = match (language, os) {
        (ProjectLanguage::Rust, _) => {
            // Start with basic Rust tools
            let mut tools = vec!["rustup".to_string(), "cargo".to_string()];

            // If this is a Rust project path that exists, check for wasm-bindgen
            if let Ok(current_dir) = std::env::current_dir() {
                let current_dir_str = current_dir.to_str().unwrap_or(".");
                let builder = crate::plugin::languages::rust_plugin::RustBuilder::new();
                if builder.uses_wasm_bindgen(current_dir_str) {
                    // Add wasm-pack for wasm-bindgen projects
                    tools.push("wasm-pack".to_string());
                }
            }

            tools
        }
        // Other language cases remain unchanged
        (ProjectLanguage::Go, _) => {
            vec!["tinygo".to_string(), "go".to_string()]
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
        (ProjectLanguage::AssemblyScript, _) => {
            vec!["node.js".to_string(), "npm".to_string(), "asc".to_string()]
        }
        (ProjectLanguage::Python, _) => Vec::new(),
        (ProjectLanguage::Unknown, _) => Vec::new(),
    };

    // Filter installed tools
    recommended_tools
        .into_iter()
        .filter(|tool| !is_tool_installed(tool))
        .collect()
}

/// Check if a tool is installed and available in the system path
pub fn is_tool_installed(tool_name: &str) -> bool {
    // Special handling for wasm-pack
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

        // If wasm-pack is not found, provide installation instructions
        if !wasm_pack_installed {
            println!("⚠️ wasm-pack is not installed. It's required for wasm-bindgen projects.");
            println!("  To install wasm-pack, run: cargo install wasm-pack");
        }

        return wasm_pack_installed;
    }

    // Original implementation for other tools
    let command = if cfg!(target_os = "windows") {
        format!("where {}", tool_name)
    } else {
        format!("which {}", tool_name)
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

/// Get missing tools required for compilation
pub fn get_missing_tools(language: &ProjectLanguage, os: &OperatingSystem) -> Vec<String> {
    let recommended_tools = get_recommended_tools(language, os);

    recommended_tools
        .into_iter()
        .filter(|tool| !is_tool_installed(tool))
        .collect()
}

/// Print system info for compilation
pub fn print_system_info() {
    let os = detect_operating_system();

    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🖥️  \x1b[1;36mSystem Information\x1b[0m");
    println!();
    println!(
        "  \x1b[1;34mOperating System:\x1b[0m \x1b[1;33m{:?}\x1b[0m",
        os
    );

    // Print additional system info
    #[cfg(target_os = "windows")]
    {
        // Check if we're running in MSYS, Cygwin, or WSL
        let is_msys = std::env::var("MSYSTEM").is_ok();
        let is_cygwin = std::env::var("CYGWIN").is_ok();
        let is_wsl = std::fs::read_to_string("/proc/version")
            .map(|v| v.contains("Microsoft") || v.contains("WSL"))
            .unwrap_or(false);

        if is_msys {
            println!("  \x1b[1;34mEnvironment:\x1b[0m \x1b[1;33mMSYS/MinGW\x1b[0m");
        } else if is_cygwin {
            println!("  \x1b[1;34mEnvironment:\x1b[0m \x1b[1;33mCygwin\x1b[0m");
        } else if is_wsl {
            println!(
                "  \x1b[1;34mEnvironment:\x1b[0m \x1b[1;33mWindows Subsystem for Linux\x1b[0m"
            );
        } else {
            println!("  \x1b[1;34mEnvironment:\x1b[0m \x1b[1;33mNative Windows\x1b[0m");
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Get macOS version
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                println!(
                    "  \x1b[1;34mmacOS Version:\x1b[0m \x1b[1;33m{}\x1b[0m",
                    version.trim()
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Try to get Linux distribution info
        if let Ok(output) = std::fs::read_to_string("/etc/os-release") {
            if let Some(name_line) = output.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
                if let Some(name) = name_line.strip_prefix("PRETTY_NAME=") {
                    let name = name.trim_matches('"');
                    println!("  \x1b[1;34mDistribution:\x1b[0m \x1b[1;33m{}\x1b[0m", name);
                }
            }
        }
    }

    println!("\x1b[1;34m╰\x1b[0m");
}
