//! Plugin protocol for communication between wasmrun and external plugins

use crate::compiler::builder::{BuildConfig, BuildResult, OptimizationLevel, TargetType};
use crate::error::{Result, WasmrunError};
use crate::plugin::PluginInfo;
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

/// Protocol version for plugin communication
pub const PLUGIN_PROTOCOL_VERSION: &str = "1.0";

/// Request types that wasmrun can send to external plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginRequest {
    Info,
    CanHandle { project_path: String },
    Validate { project_path: String },
    CheckDependencies,
    Build { config: ExternalBuildConfig },
}

/// Response types that external plugins send back to wasmrun
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginResponse {
    Info {
        info: PluginInfo,
    },
    CanHandle {
        can_handle: bool,
    },
    Validate {
        valid: bool,
        errors: Vec<String>,
    },
    Dependencies {
        missing: Vec<String>,
    },
    Build {
        result: std::result::Result<ExternalBuildResult, String>,
    },
    Error {
        message: String,
    },
}

/// Build configuration for external plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalBuildConfig {
    pub project_path: String,
    pub output_dir: String,
    pub verbose: bool,
    pub optimization_level: String,
    pub target_type: String,
}

impl From<&BuildConfig> for ExternalBuildConfig {
    fn from(config: &BuildConfig) -> Self {
        Self {
            project_path: config.project_path.clone(),
            output_dir: config.output_dir.clone(),
            verbose: config.verbose,
            optimization_level: match config.optimization_level {
                OptimizationLevel::Debug => "debug".to_string(),
                OptimizationLevel::Release => "release".to_string(),
                OptimizationLevel::Size => "size".to_string(),
            },
            target_type: match config.target_type {
                TargetType::Standard => "standard".to_string(),
                TargetType::WasmBindgen => "wasm_bindgen".to_string(),
            },
        }
    }
}

/// Build result from external plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalBuildResult {
    pub wasm_path: String,
    pub js_path: Option<String>,
    pub additional_files: Vec<String>,
    pub is_wasm_bindgen: bool,
}

impl From<ExternalBuildResult> for BuildResult {
    fn from(result: ExternalBuildResult) -> Self {
        Self {
            wasm_path: result.wasm_path,
            js_path: result.js_path,
            additional_files: result.additional_files,
            is_wasm_bindgen: result.is_wasm_bindgen,
        }
    }
}

/// Plugin communication protocol
pub struct PluginProtocol {
    pub executable_path: String,
}

impl PluginProtocol {
    pub fn new(executable_path: String) -> Self {
        Self { executable_path }
    }

    /// Send a request to the plugin and get response
    pub fn send_request(&self, request: PluginRequest) -> Result<PluginResponse> {
        let request_json = serde_json::to_string(&request)
            .map_err(|e| WasmrunError::from(format!("Failed to serialize request: {}", e)))?;

        let mut cmd = Command::new(&self.executable_path);
        cmd.arg("--wasmrun-plugin-protocol")
            .arg(PLUGIN_PROTOCOL_VERSION)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            WasmrunError::from(format!(
                "Failed to start plugin {}: {}",
                self.executable_path, e
            ))
        })?;

        if let Some(stdin) = child.stdin.take() {
            use std::io::Write;
            if let Err(e) = std::thread::spawn(move || -> std::io::Result<()> {
                let mut stdin = stdin;
                stdin.write_all(request_json.as_bytes())?;
                stdin.write_all(b"\n")?;
                stdin.flush()
            })
            .join()
            {
                return Err(WasmrunError::from(format!(
                    "Failed to send request to plugin: {:?}",
                    e
                )));
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|e| WasmrunError::from(format!("Failed to get plugin response: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmrunError::from(format!(
                "Plugin {} failed: {}",
                self.executable_path, stderr
            )));
        }

        let response_json = String::from_utf8(output.stdout).map_err(|e| {
            WasmrunError::from(format!("Invalid UTF-8 response from plugin: {}", e))
        })?;

        serde_json::from_str(&response_json)
            .map_err(|e| WasmrunError::from(format!("Failed to parse plugin response: {}", e)))
    }

    /// Check if the executable supports the wasmrun plugin protocol
    pub fn is_wasmrun_plugin(&self) -> bool {
        let output = Command::new(&self.executable_path)
            .arg("--wasmrun-plugin-check")
            .output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Get plugin information
    pub fn get_info(&self) -> Result<PluginInfo> {
        match self.send_request(PluginRequest::Info)? {
            PluginResponse::Info { info } => Ok(info),
            PluginResponse::Error { message } => {
                Err(WasmrunError::from(format!("Plugin error: {}", message)))
            }
            _ => Err(WasmrunError::from(
                "Unexpected response type for Info request",
            )),
        }
    }
}
