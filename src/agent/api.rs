//! Agent mode: REST API request/response types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Requests ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExecRequest {
    pub wasm_path: Option<String>,
    pub function: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout: Option<u64>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

// ── Responses ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct SessionStatusResponse {
    pub session_id: String,
    pub state: String,
    pub created_at_elapsed_ms: u64,
    pub last_accessed_elapsed_ms: u64,
    pub timeout_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadFileResponse {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ListFilesResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct EnvVarsResponse {
    pub env: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

// ── API Error ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ApiError {
    SessionNotFound(String),
    SessionExpired(String),
    MaxSessions(usize),
    BadRequest(String),
    NotFound(String),
    #[allow(dead_code)] // TODO: Used when exec timeout triggers API-level error
    Timeout,
    Internal(String),
}

impl ApiError {
    pub fn status_code(&self) -> u16 {
        match self {
            ApiError::SessionNotFound(_) | ApiError::NotFound(_) => 404,
            ApiError::SessionExpired(_) => 410,
            ApiError::MaxSessions(_) => 429,
            ApiError::BadRequest(_) => 400,
            ApiError::Timeout => 408,
            ApiError::Internal(_) => 500,
        }
    }

    pub fn to_error_response(&self) -> ErrorResponse {
        ErrorResponse {
            error: self.to_string(),
            code: self.status_code(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::SessionNotFound(id) => write!(f, "Session not found: {id}"),
            ApiError::SessionExpired(id) => write!(f, "Session expired: {id}"),
            ApiError::MaxSessions(max) => write!(f, "Maximum sessions reached: {max}"),
            ApiError::BadRequest(msg) => write!(f, "Bad request: {msg}"),
            ApiError::NotFound(msg) => write!(f, "Not found: {msg}"),
            ApiError::Timeout => write!(f, "Execution timed out"),
            ApiError::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}
