//! [Process Manager] `processes.toml`: the programs a supervisor keeps running.
//!
//! ```toml
//! [[process]]
//! name = "api"
//! script = "server.js"          # JavaScript, on wasmhub's nodejs runtime
//! listen = "127.0.0.1:8080"     # bound by the host, handed to the program
//! restart = "on-failure"        # always | on-failure | never
//!
//! [[process]]
//! name = "worker"
//! wasm = "target/wasm32-wasip1/release/worker.wasm"
//! args = ["--queue", "jobs"]
//! env = { RUST_LOG = "info" }
//! allow_net = ["api.example.com:443"]
//! max_memory_mb = 128
//! ```
//!
//! Relative paths are relative to the file. Everything is checked when the
//! file is read, so a mistake is reported before anything starts rather than
//! as a program that keeps crashing.

use crate::config::project::NetworkConfig;
use crate::error::{ConfigError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// The file `wasmrun ps start` reads when none is named.
pub const DEFAULT_CONFIG_FILE: &str = "processes.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    process: Vec<RawProcess>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProcess {
    name: String,
    wasm: Option<String>,
    script: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
    #[serde(default)]
    dirs: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    listen: Option<String>,
    #[serde(default)]
    allow_net: Vec<String>,
    #[serde(default)]
    restart: RestartPolicy,
    max_restarts: Option<u32>,
    restart_window_secs: Option<u64>,
    backoff_ms: Option<u64>,
    max_backoff_ms: Option<u64>,
    max_memory_mb: Option<u32>,
    max_fuel: Option<u64>,
    log_max_mb: Option<u64>,
    log_files: Option<u32>,
    autostart: Option<bool>,
}

/// When a program that ended is started again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    /// Whenever it ends, until it is stopped.
    Always,
    /// When it ends with a non-zero code or a signal. The default.
    #[default]
    OnFailure,
    /// Never: an ended program stays ended.
    Never,
}

/// What a supervised program is, as the supervisor runs it.
#[derive(Debug, Clone, Serialize)]
pub enum Program {
    /// A WASI module, run with `wasmrun exec`.
    Wasm(PathBuf),
    /// A JavaScript file under `cwd`, run on wasmhub's nodejs runtime.
    Script(String),
}

/// One `[[process]]`, checked and with every default applied.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessSpec {
    pub name: String,
    pub program: Program,
    pub args: Vec<String>,
    /// The directory the program sees as `/`, when it has one.
    pub cwd: Option<PathBuf>,
    /// More directories, as `HOST[:GUEST]`.
    pub dirs: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub listen: Option<String>,
    pub allow_net: Vec<String>,
    pub restart: RestartPolicy,
    /// Restarts allowed within `restart_window_secs` before giving up.
    pub max_restarts: u32,
    pub restart_window_secs: u64,
    /// The first restart's delay, doubled for each quick failure after it.
    pub backoff_ms: u64,
    pub max_backoff_ms: u64,
    /// Agent mode's limits, in agent mode's units. 0 is no cap.
    pub max_memory_mb: u32,
    pub max_fuel: u64,
    /// The log file's size before it is rotated, and how many rotated files
    /// are kept.
    pub log_max_mb: u64,
    pub log_files: u32,
    pub autostart: bool,
}

/// Read and check a `processes.toml`.
pub fn load(path: &Path) -> Result<Vec<ProcessSpec>> {
    let text = std::fs::read_to_string(path).map_err(|e| ConfigError::ParseError {
        message: format!("could not read {}: {e}", path.display()),
    })?;
    let base = path
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    parse(&text, &base).map_err(|message| {
        ConfigError::ParseError {
            message: format!("{}: {message}", path.display()),
        }
        .into()
    })
}

/// Check a `processes.toml`'s text, resolving relative paths against `base`.
pub fn parse(text: &str, base: &Path) -> std::result::Result<Vec<ProcessSpec>, String> {
    let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
    if raw.process.is_empty() {
        return Err("no [[process]] entries".into());
    }
    let mut seen = HashSet::new();
    raw.process
        .into_iter()
        .map(|p| {
            if !seen.insert(p.name.clone()) {
                return Err(format!("two processes are named '{}'", p.name));
            }
            check(p, base)
        })
        .collect()
}

fn check(p: RawProcess, base: &Path) -> std::result::Result<ProcessSpec, String> {
    let name = p.name;
    let valid_name = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_name {
        return Err(format!(
            "process name '{name}' must be letters, digits, '-' and '_' (it names the log file)"
        ));
    }
    let at = |msg: String| format!("[[process]] '{name}': {msg}");
    let resolve = |rel: &str| {
        let path = Path::new(rel);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        }
    };

    let cwd = p.cwd.as_deref().map(resolve);
    if let Some(dir) = &cwd {
        if !dir.is_dir() {
            return Err(at(format!("cwd '{}' is not a directory", dir.display())));
        }
    }

    let (program, cwd) = match (p.wasm, p.script) {
        (Some(wasm), None) => {
            let path = resolve(&wasm);
            if !path.is_file() {
                return Err(at(format!("wasm '{}' does not exist", path.display())));
            }
            (Program::Wasm(path), cwd)
        }
        (None, Some(script)) => {
            // A script runs from a directory: `cwd`, or the file's own one.
            let dir = cwd.unwrap_or_else(|| base.to_path_buf());
            let file = dir.join(&script);
            if !file.is_file() {
                return Err(at(format!("script '{}' does not exist", file.display())));
            }
            let inside = Path::new(&script).components().all(|c| {
                matches!(
                    c,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            });
            if !inside {
                return Err(at(format!(
                    "script '{script}' must be a path inside cwd, which is all the program can see"
                )));
            }
            (
                Program::Script(script.trim_start_matches("./").to_string()),
                Some(dir),
            )
        }
        (Some(_), Some(_)) => return Err(at("give either wasm or script, not both".into())),
        (None, None) => return Err(at("needs wasm (a .wasm file) or script (a .js file)".into())),
    };

    if let Some(listen) = &p.listen {
        listen.parse::<SocketAddr>().map_err(|_| {
            at(format!(
                "listen '{listen}' is not an address like 127.0.0.1:8080"
            ))
        })?;
    }
    if !p.allow_net.is_empty() {
        NetworkConfig::from_allow_net(&p.allow_net).map_err(|e| at(e.to_string()))?;
    }
    for key in p.env.keys() {
        if key.is_empty() || key.contains('=') {
            return Err(at(format!("env key '{key}' is not a variable name")));
        }
    }

    Ok(ProcessSpec {
        name,
        program,
        args: p.args,
        cwd,
        dirs: p.dirs,
        env: p.env,
        listen: p.listen,
        allow_net: p.allow_net,
        restart: p.restart,
        max_restarts: p.max_restarts.unwrap_or(10),
        restart_window_secs: p.restart_window_secs.unwrap_or(60),
        backoff_ms: p.backoff_ms.unwrap_or(500),
        max_backoff_ms: p.max_backoff_ms.unwrap_or(30_000),
        max_memory_mb: p.max_memory_mb.unwrap_or(256),
        max_fuel: p.max_fuel.unwrap_or(0),
        log_max_mb: p.log_max_mb.unwrap_or(10).max(1),
        log_files: p.log_files.unwrap_or(3),
        autostart: p.autostart.unwrap_or(true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            let path = dir.path().join(f);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        dir
    }

    #[test]
    fn a_script_and_a_wasm_process_are_read_with_defaults() {
        let dir = dir_with(&["server.js", "bin/worker.wasm"]);
        let specs = parse(
            r#"
[[process]]
name = "api"
script = "server.js"
listen = "127.0.0.1:8080"

[[process]]
name = "worker"
wasm = "bin/worker.wasm"
args = ["--once"]
env = { MODE = "batch" }
restart = "always"
max_memory_mb = 64
"#,
            dir.path(),
        )
        .unwrap();
        assert_eq!(specs.len(), 2);
        assert!(matches!(&specs[0].program, Program::Script(s) if s == "server.js"));
        assert_eq!(specs[0].cwd.as_deref(), Some(dir.path()));
        assert_eq!(specs[0].restart, RestartPolicy::OnFailure);
        assert_eq!(specs[0].max_memory_mb, 256);
        assert!(matches!(&specs[1].program, Program::Wasm(p) if p.ends_with("bin/worker.wasm")));
        assert_eq!(specs[1].restart, RestartPolicy::Always);
        assert_eq!(specs[1].env["MODE"], "batch");
        assert_eq!(specs[1].max_memory_mb, 64);
    }

    #[test]
    fn mistakes_are_reported_before_anything_runs() {
        let dir = dir_with(&["a.js", "x.wasm"]);
        let cases = [
            ("[[process]]\nname = \"a\"\nscript = \"missing.js\"\n", "does not exist"),
            ("[[process]]\nname = \"a\"\n", "needs wasm"),
            ("[[process]]\nname = \"a\"\nscript = \"a.js\"\nwasm = \"x.wasm\"\n", "not both"),
            ("[[process]]\nname = \"a b\"\nscript = \"a.js\"\n", "process name"),
            ("[[process]]\nname = \"a\"\nscript = \"../a.js\"\ncwd = \".\"\n", "does not exist"),
            ("[[process]]\nname = \"a\"\nscript = \"a.js\"\nlisten = \"8080\"\n", "not an address"),
            ("[[process]]\nname = \"a\"\nscript = \"a.js\"\nallow_net = [\"10.0.0/8\"]\n", "--allow-net"),
            ("[[process]]\nname = \"a\"\nscript = \"a.js\"\nrestart = \"sometimes\"\n", "unknown variant"),
            ("[[process]]\nname = \"a\"\nscript = \"a.js\"\nmax_memroy_mb = 1\n", "unknown field"),
            (
                "[[process]]\nname = \"a\"\nscript = \"a.js\"\n[[process]]\nname = \"a\"\nscript = \"a.js\"\n",
                "two processes",
            ),
            ("", "no [[process]]"),
        ];
        for (text, want) in cases {
            let err = parse(text, dir.path()).unwrap_err();
            assert!(err.contains(want), "{text:?}: {err}");
        }
    }
}
