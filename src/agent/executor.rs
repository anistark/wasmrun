//! Agent mode: Source code execution pipeline.
//!
//! Routes source code to language runtimes fetched from wasmhub.
//! Currently supports JavaScript via QuickJS.

use crate::agent::api::ApiError;
use crate::runtime::core::native_executor::execute_wasm_bytes_with_env;
use crate::runtime::runtime_cache::{wasmhub_language, RuntimeCache};
use crate::runtime::wasi::WasiEnv;
use std::path::Path;
use std::sync::{Arc, Mutex};

const JS_SCRIPT_NAME: &str = "_run_.js";

/// Resolves the language string to a supported wasmhub runtime name.
/// Returns `BadRequest` for languages not yet supported (e.g. Python).
pub fn resolve_runtime(language: &str) -> std::result::Result<&'static str, ApiError> {
    match wasmhub_language(language) {
        "nodejs" => Ok("nodejs"),
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported language: '{}'. Supported languages: javascript, js, nodejs",
            language
        ))),
    }
}

/// Execute source code in a session sandbox using a wasmhub language runtime.
///
/// Writes `source` to `{work_dir}/_run_.js`, fetches the QuickJS runtime
/// from wasmhub (cached after first download), and runs it with the session's
/// WASI environment so output is captured to the session's stdout/stderr buffers.
pub fn execute_source(
    source: &str,
    language: &str,
    wasi_env: Arc<Mutex<WasiEnv>>,
    work_dir: &Path,
    max_memory_pages: Option<u32>,
) -> std::result::Result<i32, ApiError> {
    let runtime_name = resolve_runtime(language)?;

    std::fs::write(work_dir.join(JS_SCRIPT_NAME), source)
        .map_err(|e| ApiError::Internal(format!("Failed to write script: {e}")))?;

    let cache = RuntimeCache::new()
        .map_err(|e| ApiError::Internal(format!("Runtime cache unavailable: {e}")))?;
    let wasm_bytes = cache
        .get_runtime(runtime_name)
        .map_err(|e| ApiError::Internal(format!("Failed to fetch {runtime_name} runtime: {e}")))?;

    // nodejs-runtime dispatch: "run <file>" reads the file and evals it
    let args = vec![
        "nodejs-runtime".to_string(),
        "run".to_string(),
        JS_SCRIPT_NAME.to_string(),
    ];
    execute_wasm_bytes_with_env(&wasm_bytes, wasi_env, None, args, max_memory_pages)
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_runtime_js_aliases() {
        assert_eq!(resolve_runtime("javascript").unwrap(), "nodejs");
        assert_eq!(resolve_runtime("js").unwrap(), "nodejs");
        assert_eq!(resolve_runtime("nodejs").unwrap(), "nodejs");
    }

    #[test]
    fn test_resolve_runtime_python_unsupported() {
        let err = resolve_runtime("python").unwrap_err();
        assert_eq!(err.status_code(), 400);
        let msg = err.to_string();
        assert!(msg.contains("python"), "expected 'python' in: {msg}");
        assert!(
            msg.contains("javascript"),
            "expected 'javascript' in: {msg}"
        );
    }

    #[test]
    fn test_resolve_runtime_unknown_unsupported() {
        let err = resolve_runtime("ruby").unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("ruby"));
    }

    #[test]
    fn test_resolve_runtime_case_sensitive() {
        assert!(resolve_runtime("JavaScript").is_err());
        assert!(resolve_runtime("JS").is_err());
        assert!(resolve_runtime("NodeJS").is_err());
    }
}
