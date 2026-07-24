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
        let tsconfig = TsConfig::load(files, work_dir)?;
        let mut ts_files: Vec<String> = files.keys().filter(|p| is_ts_path(p)).cloned().collect();
        ts_files.sort(); // deterministic transpile order
        if !ts_files.is_empty() {
            transpile_in_session(&ts_files, wasi_env.clone(), limits, cancel.clone())?;
        }
        tsconfig.write_path_aliases(files, work_dir, limits)?;
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

/// The parts of a project's `tsconfig.json` that reach the sandbox.
///
/// The transpiler takes file paths and no options, so most compiler options
/// cannot be honored here. The ones whose absence breaks the output are
/// refused by name; `paths` is the one this side can deliver.
#[derive(Debug, Default)]
struct TsConfig {
    /// Alias pattern → first target, both as written.
    paths: Vec<(String, String)>,
    /// `baseUrl`, normalized to a project-relative prefix ("" for the root).
    base_url: String,
}

impl TsConfig {
    fn load(
        files: &HashMap<String, String>,
        work_dir: &Path,
    ) -> std::result::Result<Self, ApiError> {
        let raw = match files.get("tsconfig.json") {
            Some(uploaded) => uploaded.clone(),
            None => match std::fs::read_to_string(work_dir.join("tsconfig.json")) {
                Ok(s) => s,
                Err(_) => return Ok(Self::default()),
            },
        };
        let parsed: serde_json::Value = serde_json::from_str(&strip_jsonc(&raw))
            .map_err(|e| ApiError::BadRequest(format!("Invalid tsconfig.json: {e}")))?;
        let Some(opts) = parsed.get("compilerOptions").and_then(|o| o.as_object()) else {
            return Ok(Self::default());
        };

        // Refused rather than dropped: the failure otherwise (decorator syntax
        // QuickJS rejects, JSX for the wrong runtime) is far harder to read.
        let unsupported = |opt: &str, why: &str| {
            Err(ApiError::BadRequest(format!(
                "tsconfig.json sets '{opt}', which the sandbox transpiler cannot apply yet: {why}"
            )))
        };
        if opts.get("experimentalDecorators") == Some(&serde_json::Value::Bool(true)) {
            return unsupported(
                "experimentalDecorators",
                "decorators would be emitted as-is and the runtime cannot execute them",
            );
        }
        if opts.get("emitDecoratorMetadata") == Some(&serde_json::Value::Bool(true)) {
            return unsupported(
                "emitDecoratorMetadata",
                "no decorator transform is available",
            );
        }
        if let Some(jsx) = opts.get("jsx").and_then(|j| j.as_str()) {
            if jsx != "react" {
                return unsupported(
                    "jsx",
                    &format!(
                        "only the classic 'react' runtime is available, not '{jsx}'; \
                         JSX compiles to React.createElement calls"
                    ),
                );
            }
        }
        // `target` is not refused: with no down-level pass the source syntax
        // reaches the runtime either way, so it is a no-op, not a wrong answer.

        let base_url = opts
            .get("baseUrl")
            .and_then(|b| b.as_str())
            .map(|b| {
                let b = b.trim_start_matches("./").trim_matches('/');
                if b.is_empty() || b == "." {
                    String::new()
                } else {
                    format!("{b}/")
                }
            })
            .unwrap_or_default();

        let mut paths = Vec::new();
        if let Some(map) = opts.get("paths").and_then(|p| p.as_object()) {
            for (pattern, targets) in map {
                // tsc tries each in order; there is no ambiguity here to resolve.
                let Some(target) = targets
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|t| t.as_str())
                else {
                    return Err(ApiError::BadRequest(format!(
                        "tsconfig.json path alias '{pattern}' must map to a non-empty array of strings"
                    )));
                };
                if pattern.contains('*') != target.contains('*') {
                    return Err(ApiError::BadRequest(format!(
                        "tsconfig.json path alias '{pattern}' -> '{target}': either both sides must use '*' or neither"
                    )));
                }
                paths.push((pattern.clone(), target.to_string()));
            }
            paths.sort();
        }
        Ok(Self { paths, base_url })
    }

    /// Materialize `paths` aliases as CommonJS shims under `node_modules`, so
    /// the runtime's stock bare-specifier walk-up resolves them with no
    /// specifier rewriting and no transpiler involvement.
    fn write_path_aliases(
        &self,
        files: &HashMap<String, String>,
        work_dir: &Path,
        limits: &ResourceLimits,
    ) -> std::result::Result<(), ApiError> {
        for (pattern, target) in &self.paths {
            let (alias, module_path) = (pattern.as_str(), target.as_str());
            match (alias.split_once('*'), module_path.split_once('*')) {
                // Wildcard alias: shim every project file under the target.
                (Some((a_pre, a_post)), Some((t_pre, t_post))) => {
                    let t_pre = format!("{}{}", self.base_url, t_pre.trim_start_matches("./"));
                    for source in files.keys() {
                        let Some(rest) = source.strip_prefix(&t_pre) else {
                            continue;
                        };
                        let Some(stem) = rest.strip_suffix(t_post).or_else(|| {
                            // "src/*" with no suffix matches any extension.
                            t_post.is_empty().then(|| strip_module_ext(rest))
                        }) else {
                            continue;
                        };
                        let stem = strip_module_ext(stem);
                        let shim = format!("node_modules/{a_pre}{stem}{a_post}.js");
                        write_alias_shim(&shim, &js_output_path(source), work_dir, limits)?;
                    }
                }
                // Exposed through package.json `main` so the alias resolves
                // as a directory.
                (None, None) => {
                    let target =
                        format!("{}{}", self.base_url, module_path.trim_start_matches("./"));
                    if !files.contains_key(&target) {
                        return Err(ApiError::BadRequest(format!(
                            "tsconfig.json path alias '{alias}' points at '{target}', which is not in the project"
                        )));
                    }
                    write_checked(
                        &work_dir.join(format!("node_modules/{alias}/package.json")),
                        br#"{"main":"index.js"}"#,
                        limits,
                        work_dir,
                    )?;
                    write_alias_shim(
                        &format!("node_modules/{alias}/index.js"),
                        &js_output_path(&target),
                        work_dir,
                        limits,
                    )?;
                }
                _ => unreachable!("mixed wildcards are rejected at load"),
            }
        }
        Ok(())
    }
}

/// Drop a module extension so the stem can be re-suffixed with `.js`.
fn strip_module_ext(path: &str) -> &str {
    for ext in [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs"] {
        if let Some(stem) = path.strip_suffix(ext) {
            return stem;
        }
    }
    path
}

/// Write a shim at `shim_path` re-exporting the project-relative `target`.
fn write_alias_shim(
    shim_path: &str,
    target: &str,
    work_dir: &Path,
    limits: &ResourceLimits,
) -> std::result::Result<(), ApiError> {
    // The session root is preopened at `/`, so an absolute specifier is stable
    // however deeply the shim is nested.
    let body = format!("module.exports = require('/{target}');\n");
    write_checked(&work_dir.join(shim_path), body.as_bytes(), limits, work_dir)
}

/// Strip `//` and `/* */` comments and trailing commas from JSONC. Real
/// tsconfigs have comments, so refusing them would make this useless.
fn strip_jsonc(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            _ => out.push(c),
        }
    }
    // Trailing commas: drop any comma followed only by whitespace and a close.
    let mut cleaned = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == ',' {
            let next = bytes[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                i += 1;
                continue;
            }
        }
        cleaned.push(bytes[i]);
        i += 1;
    }
    cleaned
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

    // ── tsconfig.json ─────────────────────────────────────────────

    fn tsconfig_from(json: &str) -> std::result::Result<TsConfig, ApiError> {
        let files = HashMap::from([("tsconfig.json".to_string(), json.to_string())]);
        TsConfig::load(&files, Path::new("/nonexistent"))
    }

    #[test]
    fn test_tsconfig_absent_is_a_no_op() {
        let files = HashMap::from([("main.ts".to_string(), "export const x = 1".to_string())]);
        let cfg = TsConfig::load(&files, Path::new("/nonexistent")).unwrap();
        assert!(cfg.paths.is_empty());
        assert_eq!(cfg.base_url, "");
    }

    #[test]
    fn test_tsconfig_accepts_comments_and_trailing_commas() {
        let cfg = tsconfig_from(
            r#"{
                // a real-world tsconfig is JSON with comments
                "compilerOptions": {
                    /* block comment */
                    "target": "ES2022",
                    "baseUrl": "./src",
                },
            }"#,
        )
        .unwrap();
        assert_eq!(cfg.base_url, "src/");
    }

    #[test]
    fn test_tsconfig_refuses_options_it_cannot_apply() {
        for (json, named) in [
            (
                r#"{"compilerOptions":{"experimentalDecorators":true}}"#,
                "experimentalDecorators",
            ),
            (
                r#"{"compilerOptions":{"emitDecoratorMetadata":true}}"#,
                "emitDecoratorMetadata",
            ),
            (r#"{"compilerOptions":{"jsx":"react-jsx"}}"#, "jsx"),
        ] {
            let err = tsconfig_from(json).unwrap_err();
            assert_eq!(err.status_code(), 400);
            assert!(
                err.to_string().contains(named),
                "error should name '{named}': {err}"
            );
        }

        // `target` is a no-op rather than a failure: with no down-level pass
        // the source syntax reaches the runtime either way.
        assert!(tsconfig_from(r#"{"compilerOptions":{"target":"ES5"}}"#).is_ok());
        assert!(tsconfig_from(r#"{"compilerOptions":{"jsx":"react"}}"#).is_ok());
    }

    #[test]
    fn test_tsconfig_rejects_malformed_path_aliases() {
        let err = tsconfig_from(r#"{"compilerOptions":{"paths":{"@app/*":["src"]}}}"#).unwrap_err();
        assert!(err.to_string().contains("both sides must use"), "{err}");

        let err = tsconfig_from(r#"{"compilerOptions":{"paths":{"@app":[]}}}"#).unwrap_err();
        assert!(err.to_string().contains("non-empty array"), "{err}");

        assert!(tsconfig_from("{not json").is_err());
    }

    #[test]
    fn test_tsconfig_wildcard_aliases_become_resolvable_shims() {
        let tmp = std::env::temp_dir().join(format!("wasmrun-tscfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let files = HashMap::from([
            (
                "tsconfig.json".to_string(),
                r#"{"compilerOptions":{"paths":{"@app/*":["src/*"]}}}"#.to_string(),
            ),
            ("src/util.ts".to_string(), "export const x = 1".to_string()),
            (
                "src/deep/thing.ts".to_string(),
                "export const y = 2".to_string(),
            ),
            (
                "main.ts".to_string(),
                "import {x} from '@app/util'".to_string(),
            ),
        ]);
        let cfg = TsConfig::load(&files, &tmp).unwrap();
        cfg.write_path_aliases(&files, &tmp, &ResourceLimits::default())
            .unwrap();

        // require('@app/util') resolves to node_modules/@app/util.js.
        let shim = std::fs::read_to_string(tmp.join("node_modules/@app/util.js")).unwrap();
        assert_eq!(shim.trim(), "module.exports = require('/src/util.js');");
        let deep = std::fs::read_to_string(tmp.join("node_modules/@app/deep/thing.js")).unwrap();
        assert_eq!(
            deep.trim(),
            "module.exports = require('/src/deep/thing.js');"
        );
        // Files outside the aliased directory are not shimmed.
        assert!(!tmp.join("node_modules/@app/main.js").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_tsconfig_exact_alias_uses_package_main() {
        let tmp = std::env::temp_dir().join(format!("wasmrun-tscfg-exact-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let files = HashMap::from([
            (
                "tsconfig.json".to_string(),
                r#"{"compilerOptions":{"paths":{"@config":["src/config.ts"]}}}"#.to_string(),
            ),
            (
                "src/config.ts".to_string(),
                "export const c = 1".to_string(),
            ),
        ]);
        let cfg = TsConfig::load(&files, &tmp).unwrap();
        cfg.write_path_aliases(&files, &tmp, &ResourceLimits::default())
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(tmp.join("node_modules/@config/package.json")).unwrap(),
            r#"{"main":"index.js"}"#
        );
        let shim = std::fs::read_to_string(tmp.join("node_modules/@config/index.js")).unwrap();
        assert_eq!(shim.trim(), "module.exports = require('/src/config.js');");

        // Outside the project: a clear 400, not a shim failing at require time.
        let files = HashMap::from([(
            "tsconfig.json".to_string(),
            r#"{"compilerOptions":{"paths":{"@gone":["src/missing.ts"]}}}"#.to_string(),
        )]);
        let cfg = TsConfig::load(&files, &tmp).unwrap();
        let err = cfg
            .write_path_aliases(&files, &tmp, &ResourceLimits::default())
            .unwrap_err();
        assert!(err.to_string().contains("not in the project"), "{err}");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
