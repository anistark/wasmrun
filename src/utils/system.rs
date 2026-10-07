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

    /// Get the latest version of a crate from crates.io, or `None` when no crate has that name.
    /// `cargo search` matches loosely and lists whatever ranks first, so the name is compared
    /// rather than trusting the first line; crates.io treats `-` and `_` as the same. Colour is
    /// forced off: under `CARGO_TERM_COLOR=always`, as CI sets it, the matched name comes back
    /// wrapped in escape codes and would never compare equal.
    pub fn get_latest_crates_version(crate_name: &str) -> Option<String> {
        let output = Command::new("cargo")
            .args(["search", crate_name, "--limit", "10", "--color", "never"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        version_from_search(&String::from_utf8_lossy(&output.stdout), crate_name)
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

/// The version `cargo search` lists for exactly `crate_name`, from lines like
/// `waspy = "0.17.0"    # A Python to WebAssembly compiler`.
fn version_from_search(stdout: &str, crate_name: &str) -> Option<String> {
    let wanted = crate_name.replace('_', "-");
    stdout.lines().find_map(|line| {
        let (name, rest) = line.split_once(" = \"")?;
        (name.trim().replace('_', "-") == wanted)
            .then(|| rest.split('"').next().map(str::to_string))?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_from_search_wants_the_exact_name() {
        let out = "waspy = \"0.17.0\"      # A Python to WebAssembly compiler\nwasmrun = \"0.23.0\"    # A WebAssembly Runtime\n";
        assert_eq!(version_from_search(out, "waspy").as_deref(), Some("0.17.0"));
        assert_eq!(
            version_from_search(out, "wasmrun").as_deref(),
            Some("0.23.0")
        );
        // A fuzzy hit is not the crate that was asked for
        assert_eq!(version_from_search(out, "wasp"), None);
        assert_eq!(
            version_from_search("ignition-cli = \"1.4.1\"\n", "invalid_plugin_name_1"),
            None
        );
        // crates.io treats - and _ as the same name
        assert_eq!(
            version_from_search("wasm-rust = \"1.0.0\"\n", "wasm_rust").as_deref(),
            Some("1.0.0")
        );
        // Coloured output would never match, which is why the search runs with colour off
        assert_eq!(
            version_from_search("\x1b[1mwaspy\x1b[0m = \"0.17.0\"\n", "waspy"),
            None
        );
    }
}
