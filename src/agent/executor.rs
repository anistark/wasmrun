//! Agent mode: Source code execution pipeline.
//!
//! Routes source code to language runtimes fetched from wasmhub.
//! Currently supports JavaScript via QuickJS.

use crate::agent::api::ApiError;
use crate::agent::limits::{dir_size, ResourceLimits};
use crate::runtime::core::native_executor::{execute_wasm_bytes_with_env, ExecLimits};
use crate::runtime::runtime_cache::{wasmhub_language, RuntimeCache};
use crate::runtime::wasi::WasiEnv;
use std::collections::HashMap;
use std::path::{Component, Path};
use std::sync::{Arc, Mutex};

const JS_SCRIPT_NAME: &str = "_run_.js";

/// Resolves the language string to a supported wasmhub runtime name.
/// Returns `BadRequest` for languages not yet supported (e.g. Python).
pub fn resolve_runtime(language: &str) -> std::result::Result<&'static str, ApiError> {
    match wasmhub_language(language) {
        "nodejs" => Ok("nodejs"),
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported language: '{language}'. Supported languages: javascript, js, nodejs",
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
    limits: &ResourceLimits,
) -> std::result::Result<i32, ApiError> {
    let runtime_name = resolve_runtime(language)?;

    write_checked(
        &work_dir.join(JS_SCRIPT_NAME),
        source.as_bytes(),
        limits,
        work_dir,
    )?;

    let wasm_bytes = fetch_runtime_bytes(runtime_name)?;

    // nodejs-runtime dispatch: "run <file>" reads the file and evals it
    let args = vec![
        format!("{runtime_name}-runtime"),
        "run".to_string(),
        JS_SCRIPT_NAME.to_string(),
    ];
    execute_wasm_bytes_with_env(&wasm_bytes, wasi_env, None, args, exec_limits(limits))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// Execute a multi-file source project in a session sandbox.
///
/// Writes every entry in `files` (path → content) into the session work_dir,
/// then runs the language runtime with `entry` as the script argument.
/// Sibling files are visible to the runtime via the session's preopened
/// WASI directory, enabling relative `require()` between project files.
pub fn execute_source_project(
    files: &HashMap<String, String>,
    entry: &str,
    language: &str,
    wasi_env: Arc<Mutex<WasiEnv>>,
    work_dir: &Path,
    limits: &ResourceLimits,
) -> std::result::Result<i32, ApiError> {
    let runtime_name = resolve_runtime(language)?;

    if files.is_empty() {
        return Err(ApiError::BadRequest("'files' map is empty".into()));
    }
    if !files.contains_key(entry) {
        return Err(ApiError::BadRequest(format!(
            "Entry '{entry}' not found in 'files' map",
        )));
    }

    for path in files.keys() {
        validate_project_filename(path)?;
    }

    for (rel_path, content) in files {
        write_checked(
            &work_dir.join(rel_path),
            content.as_bytes(),
            limits,
            work_dir,
        )?;
    }

    let args = vec![
        format!("{runtime_name}-runtime"),
        "run".to_string(),
        entry.to_string(),
    ];
    execute_wasm_bytes_with_env(
        &fetch_runtime_bytes(runtime_name)?,
        wasi_env,
        None,
        args,
        exec_limits(limits),
    )
    .map_err(|e| ApiError::Internal(e.to_string()))
}

/// Build executor-level limits (memory + fuel) from the session's full limits.
fn exec_limits(limits: &ResourceLimits) -> ExecLimits {
    ExecLimits {
        max_memory_pages: limits.max_memory_pages,
        max_fuel: limits.max_fuel,
    }
}

/// Write `content` to `resolved`, first enforcing the per-file size and total
/// disk-usage caps against the session's current footprint at `work_dir`.
fn write_checked(
    resolved: &Path,
    content: &[u8],
    limits: &ResourceLimits,
    work_dir: &Path,
) -> std::result::Result<(), ApiError> {
    let existing_len = std::fs::metadata(resolved).map(|m| m.len()).unwrap_or(0);
    let current_disk = dir_size(work_dir);
    limits
        .check_write(content.len() as u64, existing_len, current_disk)
        .map_err(ApiError::BadRequest)?;
    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ApiError::Internal(format!("Failed to create dir: {e}")))?;
    }
    std::fs::write(resolved, content)
        .map_err(|e| ApiError::Internal(format!("Failed to write file: {e}")))?;
    Ok(())
}

fn fetch_runtime_bytes(runtime_name: &str) -> std::result::Result<Vec<u8>, ApiError> {
    let cache = RuntimeCache::new()
        .map_err(|e| ApiError::Internal(format!("Runtime cache unavailable: {e}")))?;
    cache
        .get_runtime(runtime_name)
        .map_err(|e| ApiError::Internal(format!("Failed to fetch {runtime_name} runtime: {e}")))
}

/// Reject filenames that would escape the session work_dir or are unusable
/// in the runtime's WASI view (absolute paths, parent traversal, empty).
fn validate_project_filename(name: &str) -> std::result::Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::BadRequest("Empty filename in 'files'".into()));
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return Err(ApiError::BadRequest(format!(
            "Absolute path not allowed in 'files': {name}",
        )));
    }
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(ApiError::BadRequest(format!(
                    "Path traversal not allowed in 'files': {name}",
                )));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(ApiError::BadRequest(format!(
                    "Absolute path not allowed in 'files': {name}",
                )));
            }
            _ => {}
        }
    }
    Ok(())
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

    #[test]
    fn test_validate_project_filename_accepts_plain() {
        assert!(validate_project_filename("main.js").is_ok());
        assert!(validate_project_filename("utils.js").is_ok());
        assert!(validate_project_filename("lib/helper.js").is_ok());
        assert!(validate_project_filename("a/b/c/d.js").is_ok());
    }

    #[test]
    fn test_validate_project_filename_rejects_empty() {
        let err = validate_project_filename("").unwrap_err();
        assert_eq!(err.status_code(), 400);
    }

    #[test]
    fn test_validate_project_filename_rejects_absolute() {
        let err = validate_project_filename("/etc/passwd").unwrap_err();
        assert_eq!(err.status_code(), 400);
    }

    #[test]
    fn test_validate_project_filename_rejects_traversal() {
        assert!(validate_project_filename("../escape.js").is_err());
        assert!(validate_project_filename("sub/../../escape.js").is_err());
        assert!(validate_project_filename("a/b/../../../escape.js").is_err());
    }

    #[test]
    fn test_execute_source_project_rejects_empty_files() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let tmp = std::env::temp_dir();
        let err = execute_source_project(
            &HashMap::new(),
            "main.js",
            "javascript",
            env,
            &tmp,
            &ResourceLimits::default(),
        )
        .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_execute_source_project_rejects_missing_entry() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let tmp = std::env::temp_dir();
        let mut files = HashMap::new();
        files.insert("a.js".to_string(), "1".to_string());
        let err = execute_source_project(
            &files,
            "main.js",
            "javascript",
            env,
            &tmp,
            &ResourceLimits::default(),
        )
        .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Entry"));
    }

    #[test]
    fn test_execute_source_project_rejects_unsupported_language() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let tmp = std::env::temp_dir();
        let mut files = HashMap::new();
        files.insert("main.py".to_string(), "print(1)".to_string());
        let err = execute_source_project(
            &files,
            "main.py",
            "python",
            env,
            &tmp,
            &ResourceLimits::default(),
        )
        .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("python"));
    }

    #[test]
    fn test_execute_source_project_rejects_path_traversal_in_files() {
        // Create an isolated work_dir so the test never touches real files
        let tmp = std::env::temp_dir().join(format!(
            "wasmrun_proj_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let mut files = HashMap::new();
        files.insert("main.js".to_string(), "console.log(1)".to_string());
        files.insert("../evil.js".to_string(), "pwned".to_string());

        let err = execute_source_project(
            &files,
            "main.js",
            "javascript",
            env,
            &tmp,
            &ResourceLimits::default(),
        )
        .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("traversal"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
