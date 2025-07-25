//! External plugin management

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compiler::builder::{BuildConfig, BuildResult, OptimizationLevel, WasmBuilder};
use crate::error::{CompilationResult, Result, WasmrunError};
use crate::plugin::config::ExternalPluginEntry;
use crate::plugin::{Plugin, PluginCapabilities, PluginInfo, PluginSource, PluginType};

// Only import libloading on non-Windows systems where it's used
#[cfg(not(target_os = "windows"))]
use libloading::{Library, Symbol};

/// EXTERNAL PLUGIN WRAPPER - Handles both dynamic and binary loading
pub struct ExternalPluginWrapper {
    info: PluginInfo,
    plugin_name: String,
    #[cfg(not(target_os = "windows"))]
    _library: Option<Library>,
    #[allow(dead_code)]
    builder: Box<dyn WasmBuilder>,
}

impl ExternalPluginWrapper {
    pub fn new(_plugin_path: PathBuf, entry: ExternalPluginEntry) -> Result<Self> {
        println!("🔍 Debug: ExternalPluginWrapper::new called");
        println!("🔍 Debug: Plugin name: {}", entry.info.name);
        
        // Plugin type and availability
        let plugin_name = entry.info.name.clone();
        let plugin_available = Self::is_plugin_available(&plugin_name);
        println!("🔍 Debug: Plugin available: {}", plugin_available);
        
        if !plugin_available {
            let error = WasmrunError::from(format!("Plugin '{}' not available", plugin_name));
            println!("🔍 Debug: Plugin not available, returning error");
            return Err(error);
        }
        
        // Create builder for this plugin
        let builder = Self::create_builder(&plugin_name, &entry)?;
        
        println!("🔍 Debug: ExternalPluginWrapper created successfully");
        Ok(Self {
            info: entry.info,
            plugin_name,
            #[cfg(not(target_os = "windows"))]
            _library: None,
            builder,
        })
    }

    fn create_builder(plugin_name: &str, entry: &ExternalPluginEntry) -> Result<Box<dyn WasmBuilder>> {
        // TODO: Implement dynamic loading for library plugins
        if !Self::is_plugin_available(plugin_name) {
            return Err(WasmrunError::from(format!(
                "Plugin '{}' is not available",
                plugin_name
            )));
        }

        Ok(Box::new(ExternalPluginBuilder {
            plugin_name: plugin_name.to_string(),
            info: entry.info.clone(),
        }))
    }

    pub fn is_plugin_available(plugin_name: &str) -> bool {
        println!("🔍 Debug: Plugin detection: '{}'", plugin_name);
        
        // For library-based plugins (like wasmrust), check if the plugin directory exists
        let home_dir = std::env::var("HOME").unwrap_or_default();
        let plugin_dir = format!("{}/.wasmrun/plugins/{}", home_dir, plugin_name);
        
        if std::path::Path::new(&plugin_dir).exists() {
            println!("🔍 Debug: Plugin directory exists: {}", plugin_dir);
            
            // Check for metadata files that indicate proper installation
            let crates_toml = format!("{}/.crates.toml", plugin_dir);
            let crates2_json = format!("{}/.crates2.json", plugin_dir);
            
            if std::path::Path::new(&crates_toml).exists() || std::path::Path::new(&crates2_json).exists() {
                println!("🔍 Debug: Found metadata files, treating as library-based plugin");
                
                // For library-based plugins like wasmrust, check if required tools are available
                let required_deps = match plugin_name {
                    "wasmrust" => vec!["cargo", "rustc"],
                    "wasmgo" => vec!["tinygo"],
                    _ => vec![],
                };
                
                for dep in &required_deps {
                    if !Self::is_tool_available(dep) {
                        println!("🔍 Debug: Missing dependency: {}", dep);
                        return false;
                    }
                }
                
                println!("🔍 Debug: All dependencies satisfied for library plugin: {}", plugin_name);
                return true;
            }
        }
        
        Self::is_binary_available(plugin_name)
    }
    
    pub fn is_tool_available(tool: &str) -> bool {
        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        
        match Command::new(which_cmd).arg(tool).output() {
            Ok(output) => {
                let available = output.status.success();
                println!("🔍 Debug: Tool '{}' available: {}", tool, available);
                available
            }
            Err(_) => {
                println!("🔍 Debug: Failed to check tool: {}", tool);
                false
            }
        }
    }
    
    pub fn is_binary_available(plugin_name: &str) -> bool {
        println!("🔍 Debug: Checking for binary plugin: {}", plugin_name);

        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        
        match Command::new(which_cmd).arg(plugin_name).output() {
            Ok(output) => {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    println!("🔍 Debug: Binary plugin found at: {}", path.trim());
                    return true;
                }
            }
            Err(_) => {}
        }

        match Command::new(plugin_name).arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    println!("🔍 Debug: Binary plugin executable and responsive");
                    return true;
                }
            }
            Err(_) => {}
        }
        
        println!("🔍 Debug: Binary plugin '{}' not found", plugin_name);
        false
    }

    fn can_handle_rust_project(&self, project_path: &str) -> bool {
        if self.plugin_name == "wasmrust" || self.info.name == "wasmrust" {
            let cargo_toml = Path::new(project_path).join("Cargo.toml");
            return cargo_toml.exists();
        }
        false
    }
}

impl Plugin for ExternalPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check rust projects specifically
        if self.can_handle_rust_project(project_path) {
            return true;
        }

        // Check by extensions
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if self.info.extensions.contains(&ext) {
                        return true;
                    }
                }
            }
        }

        // Check by entry files
        for entry_file in &self.info.entry_files {
            let file_path = Path::new(project_path).join(entry_file);
            if file_path.exists() {
                return true;
            }
        }

        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(ExternalPluginBuilder {
            plugin_name: self.plugin_name.clone(),
            info: self.info.clone(),
        })
    }
}

/// EXTERNAL PLUGIN BUILDER - Handles compilation via external binaries
#[derive(Clone)]
pub struct ExternalPluginBuilder {
    plugin_name: String,
    info: PluginInfo,
}

impl ExternalPluginBuilder {
    fn build_impl(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        match self.plugin_name.as_str() {
            "wasmrust" => self.build_with_wasmrust(config),
            "wasmgo" => self.build_with_wasmgo(config),
            _ => Err(crate::error::CompilationError::BuildFailed {
                language: self.plugin_name.clone(),
                reason: format!("Unsupported plugin: {}", self.plugin_name),
            }),
        }
    }

    fn build_with_wasmrust(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let mut cmd = Command::new("wasmrust");
        cmd.arg("build");
        cmd.arg("--project").arg(&config.project_path);
        cmd.arg("--output").arg(&config.output_dir);

        // Optimization levels
        match config.optimization_level {
            OptimizationLevel::Debug => cmd.arg("--opt-level").arg("0"),
            OptimizationLevel::Release => cmd.arg("--opt-level").arg("3"),
            OptimizationLevel::Size => cmd.arg("--opt-level").arg("s"),
        };

        if config.verbose {
            cmd.arg("--verbose");
        }

        let output = cmd.output().map_err(|e| {
            crate::error::CompilationError::BuildFailed {
                language: "wasmrust".to_string(),
                reason: format!("Failed to execute wasmrust command: {}", e),
            }
        })?;

        if !output.status.success() {
            return Err(crate::error::CompilationError::BuildFailed {
                language: "wasmrust".to_string(),
                reason: format!(
                    "wasmrust build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        Ok(BuildResult {
            wasm_path: format!("{}/main.wasm", config.output_dir),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    fn build_with_wasmgo(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let mut cmd = Command::new("wasmgo");
        cmd.arg("build");
        cmd.arg("--project").arg(&config.project_path);
        cmd.arg("--output").arg(&config.output_dir);

        // Optimization level
        match config.optimization_level {
            OptimizationLevel::Debug => cmd.arg("--opt-level").arg("0"),
            OptimizationLevel::Release => cmd.arg("--opt-level").arg("3"),
            OptimizationLevel::Size => cmd.arg("--opt-level").arg("s"),
        };

        if config.verbose {
            cmd.arg("--verbose");
        }

        let output = cmd.output().map_err(|e| {
            crate::error::CompilationError::BuildFailed {
                language: "wasmgo".to_string(),
                reason: format!("Failed to execute wasmgo command: {}", e),
            }
        })?;

        if !output.status.success() {
            return Err(crate::error::CompilationError::BuildFailed {
                language: "wasmgo".to_string(),
                reason: format!(
                    "wasmgo build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        Ok(BuildResult {
            wasm_path: format!("{}/main.wasm", config.output_dir),
            js_path: None,
            additional_files: vec![],
            is_wasm_bindgen: false,
        })
    }

    fn build_verbose_impl(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        println!("🔧 Building with {} plugin...", self.plugin_name);
        println!("📂 Project path: {}", config.project_path);
        println!("📦 Output dir: {}", config.output_dir);
        
        let result = self.build_impl(config);
        
        match &result {
            Ok(build_result) => {
                println!("✅ Build successful!");
                println!("📄 WASM file: {}", build_result.wasm_path);
                if let Some(js_path) = &build_result.js_path {
                    println!("📄 JS file: {}", js_path);
                }
            }
            Err(e) => {
                println!("❌ Build failed: {}", e);
            }
        }
        
        result
    }
}

impl WasmBuilder for ExternalPluginBuilder {
    fn language_name(&self) -> &str {
        &self.plugin_name
    }

    fn can_handle_project(&self, project_path: &str) -> bool {
        // Check by extensions
        if let Ok(entries) = std::fs::read_dir(project_path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if self.info.extensions.contains(&ext) {
                        return true;
                    }
                }
            }
        }

        // Check by entry files
        for entry_file in &self.info.entry_files {
            let file_path = Path::new(project_path).join(entry_file);
            if file_path.exists() {
                return true;
            }
        }

        false
    }

    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        if config.verbose {
            self.build_verbose_impl(config)
        } else {
            self.build_impl(config)
        }
    }

    fn clean(&self, project_path: &str) -> Result<()> {
        let output = Command::new(&self.plugin_name)
            .args(&["clean", project_path])
            .output()
            .map_err(|e| {
                WasmrunError::from(format!(
                    "Failed to execute {} clean: {}",
                    self.plugin_name, e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "{} clean failed: {}",
                self.plugin_name, stderr
            )));
        }

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn WasmBuilder> {
        Box::new(self.clone())
    }

    fn entry_file_candidates(&self) -> &[&str] {
        match self.plugin_name.as_str() {
            "wasmrust" => &["Cargo.toml", "main.rs", "lib.rs"],
            "wasmgo" => &["go.mod", "main.go"],
            _ => &[],
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        match self.plugin_name.as_str() {
            "wasmrust" => &["rs"],
            "wasmgo" => &["go"],
            _ => &[],
        }
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !ExternalPluginWrapper::is_plugin_available(&self.plugin_name) {
            missing.push(format!("{} plugin not available", self.plugin_name));
            missing.push(format!(
                "Install {} plugin with: wasmrun plugin install {}",
                self.plugin_name, self.plugin_name
            ));
        }

        missing
    }

    fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
        if !std::path::Path::new(project_path).exists() {
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!("Project path does not exist: {}", project_path),
            });
        }

        if !self.can_handle_project(project_path) {
            return Err(crate::error::CompilationError::BuildFailed {
                language: self.language_name().to_string(),
                reason: format!(
                    "Project at {} is not compatible with {} plugin",
                    project_path, self.plugin_name
                ),
            });
        }

        Ok(())
    }

    fn build_verbose(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        self.build_verbose_impl(config)
    }
}

/// EXTERNAL PLUGIN LOADER - Main interface for loading external plugins
pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    pub fn load(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        println!("🔍 Debug: ExternalPluginLoader::load entry point");
        println!("🔍 Debug: Plugin name: {}", entry.info.name);
        println!("🔍 Debug: Plugin enabled: {}", entry.enabled);
        
        if !entry.enabled {
            let error = WasmrunError::from(format!("Plugin '{}' is disabled", entry.info.name));
            println!("🔍 Debug: Plugin disabled, returning error");
            return Err(error);
        }

        println!("🔍 Debug: Calling load_binary");
        let result = Self::load_binary(entry);
        
        match &result {
            Ok(_) => println!("🔍 Debug: load_binary succeeded"),
            Err(e) => println!("🔍 Debug: load_binary failed: {}", e),
        }
        
        result
    }

    pub fn load_binary(entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        println!("🔍 Debug: load_binary called");
        println!("🔍 Debug: Creating ExternalPluginWrapper");
        
        let wrapper = ExternalPluginWrapper::new(PathBuf::new(), entry.clone())?;
        println!("🔍 Debug: ExternalPluginWrapper created successfully");
        
        Ok(Box::new(wrapper))
    }

    /// TODO: Load plugin via dynamic library
    #[cfg(not(target_os = "windows"))]
    #[allow(dead_code)]
    pub fn load_dynamic(_entry: &ExternalPluginEntry) -> Result<Box<dyn Plugin>> {
        // TODO: Implement actual dynamic loading
        Err(WasmrunError::from("Dynamic loading not yet implemented"))
    }

    /// Create a wasmrust plugin entry for registration
    #[allow(dead_code)]
    pub fn create_wasmrust_entry() -> ExternalPluginEntry {
        ExternalPluginEntry {
            info: PluginInfo {
                name: "wasmrust".to_string(),
                version: Self::get_wasmrust_version(),
                description: "Rust WebAssembly plugin for Wasmrun".to_string(),
                author: "Kumar Anirudha".to_string(),
                extensions: vec!["rs".to_string()],
                entry_files: vec!["Cargo.toml".to_string()],
                plugin_type: PluginType::External,
                source: Some(PluginSource::CratesIo {
                    name: "wasmrust".to_string(),
                    version: "latest".to_string(),
                }),
                dependencies: vec!["cargo".to_string(), "rustc".to_string()],
                capabilities: PluginCapabilities {
                    compile_wasm: true,
                    compile_webapp: true,
                    live_reload: true,
                    optimization: true,
                    custom_targets: vec![
                        "wasm32-unknown-unknown".to_string(),
                        "web".to_string(),
                    ],
                },
            },
            enabled: true,
            install_path: "~/.wasmrun/plugins/wasmrust".to_string(),
            source: PluginSource::CratesIo {
                name: "wasmrust".to_string(),
                version: "latest".to_string(),
            },
            installed_at: chrono::Utc::now().to_rfc3339(),
            executable_path: Some("wasmrust".to_string()),
        }
    }

    fn get_wasmrust_version() -> String {
        if let Ok(output) = Command::new("wasmrust").arg("--version").output() {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = version_output.split_whitespace().nth(1) {
                    return version.to_string();
                }
            }
        }
        "unknown".to_string()
    }
}

/// DYNAMIC PLUGIN WRAPPER - For future dynamic library loading
#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
struct DynamicPluginWrapper {
    _library: Library,
    info: PluginInfo,
}

#[cfg(not(target_os = "windows"))]
impl DynamicPluginWrapper {
    #[allow(dead_code)]
    pub fn new(library_path: &Path, entry: &ExternalPluginEntry) -> Result<Self> {
        use std::ffi::c_char;
        
        unsafe {
            let library = Library::new(library_path).map_err(|e| {
                WasmrunError::from(format!(
                    "Failed to load library '{}': {}",
                    library_path.display(),
                    e
                ))
            })?;

            let _get_name: Symbol<unsafe extern "C" fn() -> *const c_char> = library
                .get(b"wasmrust_get_name")
                .map_err(|e| {
                    WasmrunError::from(format!(
                        "Plugin missing required symbol 'wasmrust_get_name': {}",
                        e
                    ))
                })?;

            // TODO: Load other required symbols
            Ok(Self {
                _library: library,
                info: entry.info.clone(),
            })
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl Plugin for DynamicPluginWrapper {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, _project_path: &str) -> bool {
        // TODO: Call dynamic function
        false
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        // TODO: Create builder from dynamic library
        Box::new(ExternalPluginBuilder {
            plugin_name: self.info.name.clone(),
            info: self.info.clone(),
        })
    }
}
