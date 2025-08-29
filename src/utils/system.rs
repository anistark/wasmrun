use std::process::Command;

/// System utilities for tool detection and version checking
pub struct SystemUtils;

impl SystemUtils {
    /// Check if a command/tool is available in the system PATH
    pub fn is_tool_available(tool: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };

        Command::new(which_cmd)
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Check if Rust wasm32-unknown-unknown target is installed
    #[allow(dead_code)]
    pub fn is_wasm_target_installed() -> bool {
        Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("wasm32-unknown-unknown")
            })
            .unwrap_or(false)
    }

    /// Get the latest version of a crate from crates.io
    pub fn get_latest_crates_version(crate_name: &str) -> Option<String> {
        if let Ok(output) = Command::new("cargo")
            .args(["search", crate_name, "--limit", "1"])
            .output()
        {
            if output.status.success() {
                let search_output = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = search_output.lines().next() {
                    if let Some(start) = line.find(" = \"") {
                        if let Some(end) = line[start + 4..].find('"') {
                            return Some(line[start + 4..start + 4 + end].to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Detect version from Cargo.toml content
    #[allow(dead_code)]
    pub fn detect_version_from_cargo_toml(content: &str) -> Option<String> {
        // Try parsing with toml first
        if let Ok(parsed) = toml::from_str::<toml::Value>(content) {
            if let Some(package) = parsed.get("package") {
                if let Some(version) = package.get("version") {
                    if let Some(version_str) = version.as_str() {
                        return Some(version_str.to_string());
                    }
                }
            }
        }

        // Line-by-line parsing
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("version") && line.contains('=') {
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let version = &line[start + 1..start + 1 + end];
                        return Some(version.to_string());
                    }
                }
            }
        }

        None
    }

    /// Check if project has wasm-bindgen dependency
    #[allow(dead_code)]
    pub fn has_wasm_bindgen_dependency(cargo_toml_path: &std::path::Path) -> bool {
        if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
            content.contains("wasm-bindgen")
        } else {
            false
        }
    }
}
