use crate::error::{Result, WasmrunError};
use crate::plugin::{PluginCapabilities, PluginInfo, PluginSource, PluginType};
use crate::utils::SystemUtils;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub extensions: Vec<String>,
    pub entry_files: Vec<String>,
    pub capabilities: MetadataCapabilities,
    pub dependencies: MetadataDependencies,
    pub exports: Option<MetadataExports>,
    pub frameworks: Option<MetadataFrameworks>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataCapabilities {
    pub compile_wasm: bool,
    pub compile_webapp: bool,
    pub live_reload: bool,
    pub optimization: bool,
    pub custom_targets: Vec<String>,
    pub supported_languages: Option<Vec<String>>, // Add this field
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataDependencies {
    pub tools: Vec<String>,
    pub optional_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataExports {
    pub create_wasm_builder: String,
    pub can_handle_project: String,
    pub build: String,
    pub clean: String,
    pub clone_box: String,
    pub drop: String,
    pub plugin_create: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFrameworks {
    pub supported: Vec<String>,
    pub auto_detect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CargoToml {
    package: CargoPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    description: Option<String>,
    authors: Option<Vec<String>>,
    metadata: Option<CargoMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CargoMetadata {
    wasm_plugin: Option<PluginMetadata>,
}

impl PluginMetadata {
    pub fn from_installed_plugin(plugin_dir: &Path) -> Result<Self> {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return Err(WasmrunError::from(
                "Cargo.toml not found in plugin directory",
            ));
        }

        let content = std::fs::read_to_string(&cargo_toml_path)
            .map_err(|e| WasmrunError::from(format!("Failed to read Cargo.toml: {e}")))?;

        Self::from_cargo_toml_content(&content)
    }

    pub fn from_cargo_toml_content(content: &str) -> Result<Self> {
        let cargo_toml: CargoToml = toml::from_str(content)
            .map_err(|e| WasmrunError::from(format!("Failed to parse Cargo.toml: {e}")))?;

        if let Some(metadata) = cargo_toml.package.metadata.and_then(|m| m.wasm_plugin) {
            Ok(metadata)
        } else {
            let name = cargo_toml.package.name;
            let version = cargo_toml.package.version;
            let description = cargo_toml.package.description.unwrap_or_default();
            let author = cargo_toml
                .package
                .authors
                .and_then(|authors| authors.first().cloned())
                .unwrap_or_default();

            Ok(Self::create_fallback_metadata(
                name,
                version,
                description,
                author,
            ))
        }
    }

    pub fn from_crates_io(crate_name: &str) -> Result<Self> {
        // First try to get the plugin metadata from a locally cached Cargo.toml if available
        if let Ok(metadata) = Self::from_cached_cargo_toml(crate_name) {
            return Ok(metadata);
        }

        // Try to download Cargo.toml from crates.io API (future implementation)
        if let Ok(metadata) = Self::from_crates_io_api(crate_name) {
            return Ok(metadata);
        }

        // Basic search
        let output = std::process::Command::new("cargo")
            .args(["search", crate_name, "--limit", "1"])
            .output()
            .map_err(|e| WasmrunError::from(format!("Failed to search crates.io: {e}")))?;

        if !output.status.success() {
            return Err(WasmrunError::from(format!(
                "Plugin '{crate_name}' not found on crates.io"
            )));
        }

        let search_output = String::from_utf8_lossy(&output.stdout);
        if search_output.trim().is_empty() {
            return Err(WasmrunError::from(format!(
                "Plugin '{crate_name}' not found on crates.io"
            )));
        }

        let version = SystemUtils::get_latest_crates_version(crate_name)
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self::create_fallback_metadata(
            crate_name.to_string(),
            version,
            format!("{crate_name} WebAssembly plugin for wasmrun"),
            "Unknown".to_string(),
        ))
    }

    /// Try to get metadata from a locally cached Cargo.toml (e.g., if plugin was downloaded before)
    fn from_cached_cargo_toml(crate_name: &str) -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not find home directory"))?
            .join(".wasmrun")
            .join("cache")
            .join(crate_name);

        let cargo_toml_path = cache_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            let content = std::fs::read_to_string(&cargo_toml_path).map_err(|e| {
                WasmrunError::from(format!("Failed to read cached Cargo.toml: {e}"))
            })?;
            return Self::from_cargo_toml_content(&content);
        }

        Err(WasmrunError::from("No cached Cargo.toml found"))
    }

    /// Future implementation: download Cargo.toml from crates.io API
    fn from_crates_io_api(crate_name: &str) -> Result<Self> {
        // For now, we'll try to use `cargo show` if available
        let output = std::process::Command::new("cargo")
            .args(["show", crate_name])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let show_output = String::from_utf8_lossy(&output.stdout);
                // Parse the cargo show output for metadata
                return Self::parse_cargo_show_output(crate_name, &show_output);
            }
        }

        // Download crate metadata from crates.io API
        download_crate_metadata_from_api(crate_name)
    }

    /// Parse cargo show output to extract metadata
    fn parse_cargo_show_output(crate_name: &str, output: &str) -> Result<Self> {
        let mut version = "unknown".to_string();
        let mut description = format!("{crate_name} WebAssembly plugin");
        let mut author = "Unknown".to_string();

        for line in output.lines() {
            if let Some(v) = line.strip_prefix("version: ") {
                version = v.trim().to_string();
            } else if let Some(d) = line.strip_prefix("description: ") {
                description = d.trim().to_string();
            } else if let Some(a) = line.strip_prefix("authors: ") {
                author = a.trim().to_string();
            }
        }

        Ok(Self::create_fallback_metadata(
            crate_name.to_string(),
            version,
            description,
            author,
        ))
    }

    fn create_fallback_metadata(
        name: String,
        version: String,
        description: String,
        author: String,
    ) -> Self {
        let (extensions, entry_files, dependencies) = Self::infer_plugin_details(&name);

        Self {
            name: name.clone(),
            version,
            description,
            author,
            extensions,
            entry_files,
            capabilities: MetadataCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32-unknown-unknown".to_string()],
                supported_languages: Some(vec![name.clone()]), // Use plugin name as fallback
            },
            dependencies: MetadataDependencies {
                tools: dependencies,
                optional_tools: None,
            },
            exports: Some(Self::create_default_exports(&name)),
            frameworks: None,
        }
    }

    fn infer_plugin_details(plugin_name: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
        match plugin_name {
            name if name.contains("rust") => (
                vec!["rs".to_string(), "toml".to_string()],
                vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                vec!["cargo".to_string(), "rustc".to_string()],
            ),
            name if name.contains("go") => (
                vec!["go".to_string(), "mod".to_string()],
                vec!["go.mod".to_string(), "main.go".to_string()],
                vec!["tinygo".to_string()],
            ),
            name if name.contains("zig") => (
                vec!["zig".to_string()],
                vec!["build.zig".to_string(), "src/main.zig".to_string()],
                vec!["zig".to_string()],
            ),
            name if name.contains("cpp") || name.contains("cxx") => (
                vec!["cpp".to_string(), "cxx".to_string(), "hpp".to_string()],
                vec!["CMakeLists.txt".to_string(), "Makefile".to_string()],
                vec!["emcc".to_string()],
            ),
            _ => (
                vec!["wasm".to_string()],
                vec!["main.wasm".to_string()],
                vec![],
            ),
        }
    }

    fn create_default_exports(plugin_name: &str) -> MetadataExports {
        let prefix = plugin_name.replace('-', "_");
        MetadataExports {
            create_wasm_builder: "create_wasm_builder".to_string(),
            can_handle_project: format!("{prefix}_can_handle_project"),
            build: format!("{prefix}_build"),
            clean: format!("{prefix}_clean"),
            clone_box: format!("{prefix}_clone_box"),
            drop: format!("{prefix}_drop"),
            plugin_create: "wasmrun_plugin_create".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn to_plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            author: self.author.clone(),
            extensions: self.extensions.clone(),
            entry_files: self.entry_files.clone(),
            plugin_type: PluginType::External,
            source: Some(PluginSource::CratesIo {
                name: self.name.clone(),
                version: self.version.clone(),
            }),
            dependencies: self.dependencies.tools.clone(),
            capabilities: PluginCapabilities {
                compile_wasm: self.capabilities.compile_wasm,
                compile_webapp: self.capabilities.compile_webapp,
                live_reload: self.capabilities.live_reload,
                optimization: self.capabilities.optimization,
                custom_targets: self.capabilities.custom_targets.clone(),
                supported_languages: self.capabilities.supported_languages.clone(),
            },
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(WasmrunError::from("Plugin name cannot be empty"));
        }

        if self.extensions.is_empty() {
            return Err(WasmrunError::from(
                "Plugin must support at least one file extension",
            ));
        }

        if self.entry_files.is_empty() {
            return Err(WasmrunError::from(
                "Plugin must specify at least one entry file",
            ));
        }

        Ok(())
    }
}

/// Download crate metadata from crates.io API
fn download_crate_metadata_from_api(crate_name: &str) -> Result<PluginMetadata> {
    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg(format!("https://crates.io/api/v1/crates/{crate_name}"))
        .output()
        .map_err(|e| {
            WasmrunError::from(format!("Failed to download metadata from crates.io: {e}"))
        })?;

    if !output.status.success() {
        return Err(WasmrunError::from(format!(
            "Failed to query crates.io API for {crate_name}"
        )));
    }

    let response = String::from_utf8_lossy(&output.stdout);
    parse_crates_io_metadata_response(crate_name, &response)
}

/// Parse crates.io API response to extract metadata
fn parse_crates_io_metadata_response(crate_name: &str, response: &str) -> Result<PluginMetadata> {
    use serde_json::Value;

    let json: Value = serde_json::from_str(response)
        .map_err(|e| WasmrunError::from(format!("Failed to parse crates.io response: {e}")))?;

    let crate_info = json["crate"]
        .as_object()
        .ok_or_else(|| WasmrunError::from("Invalid crates.io response format".to_string()))?;

    let version = crate_info["max_version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let description = crate_info["description"]
        .as_str()
        .unwrap_or(&format!("{crate_name} WebAssembly plugin"))
        .to_string();

    // Try to extract author information
    let mut author = "Unknown".to_string();
    if let Some(versions) = json["versions"].as_array() {
        if let Some(latest_version) = versions.first() {
            if let Some(published_by) = latest_version["published_by"]["name"].as_str() {
                author = published_by.to_string();
            }
        }
    }

    // Get language support from plugin metadata
    let languages = infer_supported_languages_from_name(crate_name);

    Ok(PluginMetadata {
        name: crate_name.to_string(),
        version,
        description,
        author,
        extensions: languages.clone(), // Map languages to extensions
        entry_files: infer_entry_files_from_name(crate_name), // Infer based on plugin name
        capabilities: MetadataCapabilities {
            compile_wasm: true,
            compile_webapp: false,
            live_reload: false,
            optimization: true,
            custom_targets: vec![],
            supported_languages: Some(languages),
        },
        dependencies: MetadataDependencies {
            tools: vec![], // Dependencies will be fetched separately if needed
            optional_tools: None,
        },
        exports: None,
        frameworks: None,
    })
}

fn infer_entry_files_from_name(plugin_name: &str) -> Vec<String> {
    match plugin_name {
        name if name.contains("rust") => vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()],
        name if name.contains("go") => vec!["go.mod".to_string(), "main.go".to_string()],
        name if name.contains("zig") => vec!["build.zig".to_string(), "src/main.zig".to_string()],
        name if name.contains("cpp") || name.contains("cxx") => {
            vec!["CMakeLists.txt".to_string(), "Makefile".to_string()]
        }
        name if name.contains("py") || name.contains("python") => {
            vec!["main.py".to_string(), "app.py".to_string()]
        }
        _ => vec!["main.wasm".to_string()],
    }
}

/// Infer supported languages from plugin name and try to read Cargo.toml if available
fn infer_supported_languages_from_name(plugin_name: &str) -> Vec<String> {
    // Read from plugin's Cargo.toml
    if let Ok(plugin_dir) = crate::utils::PluginUtils::get_plugin_directory(plugin_name) {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        if cargo_toml_path.exists() {
            if let Ok(languages) = extract_languages_from_cargo_toml(&cargo_toml_path) {
                if !languages.is_empty() {
                    return languages;
                }
            }
        }
    }

    match plugin_name {
        name if name.contains("rust") || name.contains("rs") => vec!["rust".to_string()],
        name if name.contains("go") => vec!["go".to_string()],
        name if name.contains("zig") => vec!["zig".to_string()],
        name if name.contains("cpp") || name.contains("cxx") || name.contains("c++") => {
            vec!["cpp".to_string(), "c".to_string()]
        }
        name if name.contains("py") || name.contains("python") => vec!["python".to_string()],
        name if name.contains("js") || name.contains("javascript") => {
            vec!["javascript".to_string()]
        }
        name if name.contains("ts") || name.contains("typescript") => {
            vec!["typescript".to_string()]
        }
        name if name.contains("asc") || name.contains("assemblyscript") => {
            vec!["assemblyscript".to_string()]
        }
        name if name.contains("wat") || name.contains("wasm") => {
            vec!["wat".to_string(), "wasm".to_string()]
        }
        _ => {
            if plugin_name.ends_with("-rust")
                || plugin_name.starts_with("wasm") && plugin_name.contains("rust")
            {
                vec!["rust".to_string()]
            } else {
                vec!["unknown".to_string()]
            }
        }
    }
}

/// Extract supported languages from Cargo.toml metadata
fn extract_languages_from_cargo_toml(cargo_toml_path: &std::path::Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(cargo_toml_path)
        .map_err(|e| WasmrunError::Config(crate::error::ConfigError::ParseError {
            message: format!("Failed to read Cargo.toml: {e}"),
        }))?;

    let mut languages = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        if line.contains("[package.metadata.wasmrun]") {
            continue;
        }

        if line.starts_with("languages") && line.contains('=') {
            if let Some(langs_part) = line.split('=').nth(1) {
                let langs_str = langs_part
                    .trim()
                    .trim_matches('"')
                    .trim_matches('[')
                    .trim_matches(']');
                for lang in langs_str.split(',') {
                    let lang = lang.trim().trim_matches('"').trim_matches('\'');
                    if !lang.is_empty() {
                        languages.push(lang.to_string());
                    }
                }
            }
        }

        if line.starts_with("keywords") && line.contains('=') {
            if let Some(keywords_part) = line.split('=').nth(1) {
                let keywords_str = keywords_part
                    .trim()
                    .trim_matches('"')
                    .trim_matches('[')
                    .trim_matches(']');
                for keyword in keywords_str.split(',') {
                    let keyword = keyword
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_lowercase();
                    match keyword.as_str() {
                        "rust" | "rustlang" => languages.push("rust".to_string()),
                        "go" | "golang" => languages.push("go".to_string()),
                        "c" => languages.push("c".to_string()),
                        "cpp" | "c++" => languages.push("cpp".to_string()),
                        "python" | "py" => languages.push("python".to_string()),
                        "javascript" | "js" => languages.push("javascript".to_string()),
                        "typescript" | "ts" => languages.push("typescript".to_string()),
                        "zig" => languages.push("zig".to_string()),
                        "assemblyscript" | "asc" => languages.push("assemblyscript".to_string()),
                        _ => {}
                    }
                }
            }
        }
    }

    languages.sort();
    languages.dedup();

    if languages.is_empty() && content.contains("[dependencies]") {
        languages.push("rust".to_string());
    }

    Ok(languages)
}
