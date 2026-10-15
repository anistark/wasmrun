//! OS mode command implementation

use crate::error::{Result, WasmrunError};
use crate::runtime::multilang_kernel::{MultiLanguageKernel, OsRunConfig};
use crate::runtime::os_server::OsServer;
use crate::runtime::tunnel::bore::BoreServer;
use crate::utils::PathResolver;
use std::fmt;
use std::path::Path;

/// Supported languages in OS mode
#[derive(Debug, Clone, PartialEq)]
pub enum OsLanguage {
    NodeJs,
    Python,
}

impl OsLanguage {
    /// Parse a string into an OS language
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "nodejs" | "node" | "js" | "javascript" => Ok(OsLanguage::NodeJs),
            "python" | "py" => Ok(OsLanguage::Python),
            _ => Err(WasmrunError::from(format!(
                "Unsupported OS mode language: '{s}'. Supported languages: nodejs, python"
            ))),
        }
    }
}

/// `--expose`: where the public tunnel goes. Absent when not exposing.
#[derive(Debug, Clone, Default)]
pub struct OsTunnel {
    pub server: Option<String>,
    pub secret: Option<String>,
}

/// Handle the OS mode command
#[allow(clippy::too_many_arguments)]
pub fn handle_os_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    port: u16,
    language: &Option<String>,
    watch: bool,
    verbose: bool,
    allow_cors: bool,
    tunnel: Option<OsTunnel>,
) -> Result<()> {
    let resolved_path = PathResolver::resolve_input_path(positional_path.clone(), path.clone());

    // `--language`, else `[os] language` in the project's wasmrun.toml
    let validated_language = if let Some(lang) = language {
        Some(os_validate_language(lang)?)
    } else {
        match crate::config::ProjectConfig::load(&resolved_path)?
            .os
            .language
        {
            Some(lang) => Some(
                os_validate_language(&lang)
                    .map_err(|e| WasmrunError::from(format!("wasmrun.toml [os] language: {e}")))?,
            ),
            None => None,
        }
    };

    // A bad address should stop the command, not surface later as a tunnel
    // that never connects
    if let Some(server) = tunnel.as_ref().and_then(|t| t.server.as_deref()) {
        BoreServer::parse(server)
            .map_err(|e| WasmrunError::from(format!("--tunnel-server: {e}")))?;
    }

    os_run_project(
        resolved_path,
        port,
        validated_language,
        watch,
        verbose,
        allow_cors,
        tunnel,
    )
}

/// Validate OS mode language
fn os_validate_language(language: &str) -> Result<OsLanguage> {
    OsLanguage::from_str(language)
}

/// Run a project in OS mode with browser-based multi-language kernel
pub fn os_run_project(
    path: String,
    port: u16,
    language: Option<OsLanguage>,
    watch: bool,
    verbose: bool,
    allow_cors: bool,
    tunnel: Option<OsTunnel>,
) -> Result<()> {
    if verbose {
        println!("🔍 OS Mode: Analyzing project path: {path}");
    }

    if !Path::new(&path).exists() {
        return Err(WasmrunError::from(format!(
            "Project path does not exist: {path}"
        )));
    }

    if !Path::new(&path).is_dir() {
        return Err(WasmrunError::from(format!(
            "OS mode requires a project directory, not a file: {path}"
        )));
    }

    os_start_kernel_and_server(path, port, language, watch, verbose, allow_cors, tunnel)
}

/// Start the OS mode kernel and server
fn os_start_kernel_and_server(
    path: String,
    port: u16,
    language: Option<OsLanguage>,
    watch: bool,
    verbose: bool,
    allow_cors: bool,
    tunnel: Option<OsTunnel>,
) -> Result<()> {
    println!("🚀 Starting wasmrun in OS mode for project: {path}");

    if let Some(ref lang) = language {
        println!("🏷️  Forced language: {lang}");
    }

    if watch {
        println!("👀 Watch mode enabled");
    }

    if verbose {
        println!("🔍 Verbose output enabled");
    }

    let config = os_create_config(path, language, watch, verbose, allow_cors, tunnel)?;
    let kernel = os_initialize_kernel(config.clone())?;
    let server = os_create_server(kernel, config)?;
    os_start_server(server, port)
}

/// Create OS mode configuration
fn os_create_config(
    project_path: String,
    language: Option<OsLanguage>,
    watch: bool,
    _verbose: bool,
    allow_cors: bool,
    tunnel: Option<OsTunnel>,
) -> Result<OsRunConfig> {
    Ok(OsRunConfig {
        project_path,
        language: language.map(|l| l.to_string()),
        port: None,
        hot_reload: watch,
        expose: tunnel.is_some(),
        tunnel_server: tunnel.as_ref().and_then(|t| t.server.clone()),
        tunnel_secret: tunnel.and_then(|t| t.secret),
        allow_cors,
    })
}

fn os_initialize_kernel(_config: OsRunConfig) -> Result<MultiLanguageKernel> {
    let kernel = MultiLanguageKernel::new();
    kernel
        .start()
        .map_err(|e| WasmrunError::from(format!("Failed to start the OS kernel: {e}")))?;
    println!("✅ Multi-language kernel started");
    Ok(kernel)
}

/// Create the OS mode server
fn os_create_server(kernel: MultiLanguageKernel, config: OsRunConfig) -> Result<OsServer> {
    let server = OsServer::new(kernel, config)?;
    Ok(server)
}

/// Start the OS mode server
fn os_start_server(server: OsServer, port: u16) -> Result<()> {
    println!("🌐 OS Mode interface starting on http://localhost:{port}");
    println!("📱 Open your browser to access the development environment");

    server.start(port)
}

impl fmt::Display for OsLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lang_str = match self {
            OsLanguage::NodeJs => "nodejs",
            OsLanguage::Python => "python",
        };
        write!(f, "{lang_str}")
    }
}
