//! Agent mode: Source code execution pipeline.
//!
//! Routes source code to language runtimes fetched from wasmhub.
//! Supports JavaScript via QuickJS and TypeScript via an in-sandbox
//! swc transpile stage that emits JS for the same runtime.

use crate::agent::api::ApiError;
use crate::agent::limits::{dir_size, ResourceLimits};
use crate::runtime::core::native_executor::{execute_wasm_bytes_with_env, ExecLimits};
use crate::runtime::runtime_cache::RuntimeCache;
use crate::runtime::wasi::WasiEnv;
use std::collections::HashMap;
use std::path::{Component, Path};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

const JS_SCRIPT_NAME: &str = "_run_.js";
const TS_SCRIPT_NAME: &str = "_run_.ts";
const TSX_SCRIPT_NAME: &str = "_run_.tsx";
/// wasmhub artifact name of the TypeScript transpiler (an swc-based WASI CLI).
const TS_TRANSPILER: &str = "swc";
/// The runtime that executes the (possibly transpiled) JavaScript.
const JS_RUNTIME: &str = "nodejs";

/// A source language accepted by the exec endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLanguage {
    JavaScript,
    /// TypeScript; `tsx` selects the `.tsx` extension (JSX-enabled parse) for
    /// single-`source` execution. In `files` mode each file's own extension
    /// decides, so the flag only affects the generated script name.
    TypeScript {
        tsx: bool,
    },
}

impl SourceLanguage {
    fn is_typescript(self) -> bool {
        matches!(self, SourceLanguage::TypeScript { .. })
    }
}

/// Resolves the language string to a supported source language.
/// Returns `BadRequest` for languages not yet supported (e.g. Python).
pub fn resolve_language(language: &str) -> std::result::Result<SourceLanguage, ApiError> {
    match language {
        "javascript" | "js" | "nodejs" => Ok(SourceLanguage::JavaScript),
        "typescript" | "ts" => Ok(SourceLanguage::TypeScript { tsx: false }),
        "tsx" => Ok(SourceLanguage::TypeScript { tsx: true }),
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported language: '{language}'. Supported languages: javascript, js, nodejs, typescript, ts, tsx",
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
    cancel: Option<Arc<AtomicBool>>,
) -> std::result::Result<i32, ApiError> {
    let lang = resolve_language(language)?;

    let script_name = match lang {
        SourceLanguage::JavaScript => JS_SCRIPT_NAME,
        SourceLanguage::TypeScript { tsx: false } => TS_SCRIPT_NAME,
        SourceLanguage::TypeScript { tsx: true } => TSX_SCRIPT_NAME,
    };
    write_checked(
        &work_dir.join(script_name),
        source.as_bytes(),
        limits,
        work_dir,
    )?;

    let run_target = if lang.is_typescript() {
        transpile_in_session(
            &[script_name.to_string()],
            wasi_env.clone(),
            limits,
            cancel.clone(),
        )?;
        JS_SCRIPT_NAME
    } else {
        script_name
    };

    let wasm_bytes = fetch_runtime_bytes(JS_RUNTIME)?;

    // nodejs-runtime dispatch: "run <file>" reads the file and evals it
    let args = vec![
        format!("{JS_RUNTIME}-runtime"),
        "run".to_string(),
        run_target.to_string(),
    ];
    execute_wasm_bytes_with_env(
        &wasm_bytes,
        wasi_env,
        None,
        args,
        exec_limits(limits),
        cancel,
    )
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
    cancel: Option<Arc<AtomicBool>>,
) -> std::result::Result<i32, ApiError> {
    let lang = resolve_language(language)?;

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

    let entry_to_run = if lang.is_typescript() {
        let mut ts_files: Vec<String> = files.keys().filter(|p| is_ts_path(p)).cloned().collect();
        ts_files.sort(); // deterministic transpile order
        if !ts_files.is_empty() {
            transpile_in_session(&ts_files, wasi_env.clone(), limits, cancel.clone())?;
        }
        js_output_path(entry)
    } else {
        entry.to_string()
    };

    let args = vec![
        format!("{JS_RUNTIME}-runtime"),
        "run".to_string(),
        entry_to_run,
    ];
    execute_wasm_bytes_with_env(
        &fetch_runtime_bytes(JS_RUNTIME)?,
        wasi_env,
        None,
        args,
        exec_limits(limits),
        cancel,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))
}

/// True for paths the transpile stage should convert to JavaScript.
fn is_ts_path(path: &str) -> bool {
    path.ends_with(".ts") || path.ends_with(".tsx")
}

/// The `.js` path the transpiler emits for a TypeScript input (non-TS paths
/// pass through unchanged, so a `.js` entry in a TS project is allowed).
fn js_output_path(path: &str) -> String {
    if let Some(stem) = path.strip_suffix(".tsx") {
        format!("{stem}.js")
    } else if let Some(stem) = path.strip_suffix(".ts") {
        format!("{stem}.js")
    } else {
        path.to_string()
    }
}

/// Run the swc transpiler WASM inside the session sandbox over `ts_files`
/// (paths relative to the session root). Each input emits a sibling `.js`.
///
/// The transpiler shares the session's WASI environment, so its output lands
/// in the session buffers; those are snapshotted and cleared here so
/// transpiler noise never leaks into the program's captured output. A
/// non-zero exit is reported with the transpiler's stderr (which references
/// the original `.ts` file names).
fn transpile_in_session(
    ts_files: &[String],
    wasi_env: Arc<Mutex<WasiEnv>>,
    limits: &ResourceLimits,
    cancel: Option<Arc<AtomicBool>>,
) -> std::result::Result<(), ApiError> {
    let transpiler = fetch_runtime_bytes(TS_TRANSPILER)?;

    let mut args = vec![TS_TRANSPILER.to_string()];
    args.extend(ts_files.iter().cloned());

    let exit = execute_wasm_bytes_with_env(
        &transpiler,
        wasi_env.clone(),
        None,
        args,
        exec_limits(limits),
        cancel,
    )
    .map_err(|e| ApiError::Internal(format!("TypeScript transpiler failed: {e}")))?;

    let stderr = {
        let mut env = wasi_env
            .lock()
            .map_err(|_| ApiError::Internal("Lock".into()))?;
        let captured = String::from_utf8_lossy(&env.get_stderr()).into_owned();
        env.clear_stdout();
        env.clear_stderr();
        captured
    };

    if exit != 0 {
        let detail = stderr.trim();
        return Err(ApiError::BadRequest(if detail.is_empty() {
            format!("TypeScript transpilation failed (exit code {exit})")
        } else {
            format!("TypeScript transpilation failed: {detail}")
        }));
    }
    Ok(())
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
    fn test_resolve_language_js_aliases() {
        assert_eq!(
            resolve_language("javascript").unwrap(),
            SourceLanguage::JavaScript
        );
        assert_eq!(resolve_language("js").unwrap(), SourceLanguage::JavaScript);
        assert_eq!(
            resolve_language("nodejs").unwrap(),
            SourceLanguage::JavaScript
        );
    }

    #[test]
    fn test_resolve_language_ts_aliases() {
        assert_eq!(
            resolve_language("typescript").unwrap(),
            SourceLanguage::TypeScript { tsx: false }
        );
        assert_eq!(
            resolve_language("ts").unwrap(),
            SourceLanguage::TypeScript { tsx: false }
        );
        assert_eq!(
            resolve_language("tsx").unwrap(),
            SourceLanguage::TypeScript { tsx: true }
        );
    }

    #[test]
    fn test_resolve_language_python_unsupported() {
        let err = resolve_language("python").unwrap_err();
        assert_eq!(err.status_code(), 400);
        let msg = err.to_string();
        assert!(msg.contains("python"), "expected 'python' in: {msg}");
        assert!(
            msg.contains("javascript"),
            "expected 'javascript' in: {msg}"
        );
        assert!(
            msg.contains("typescript"),
            "expected 'typescript' in: {msg}"
        );
    }

    #[test]
    fn test_resolve_language_unknown_unsupported() {
        let err = resolve_language("ruby").unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("ruby"));
    }

    #[test]
    fn test_resolve_language_case_sensitive() {
        assert!(resolve_language("JavaScript").is_err());
        assert!(resolve_language("JS").is_err());
        assert!(resolve_language("TypeScript").is_err());
        assert!(resolve_language("TS").is_err());
    }

    #[test]
    fn test_is_ts_path() {
        assert!(is_ts_path("main.ts"));
        assert!(is_ts_path("components/app.tsx"));
        assert!(!is_ts_path("main.js"));
        assert!(!is_ts_path("data.json"));
        assert!(!is_ts_path("notes.txt"));
    }

    #[test]
    fn test_js_output_path() {
        assert_eq!(js_output_path("main.ts"), "main.js");
        assert_eq!(js_output_path("components/app.tsx"), "components/app.js");
        assert_eq!(js_output_path("already.js"), "already.js");
        assert_eq!(js_output_path("data.json"), "data.json");
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
            None,
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
            None,
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
            None,
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
            None,
        )
        .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("traversal"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
