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
