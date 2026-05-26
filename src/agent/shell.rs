//! Agent mode: Basic shell emulation for sandboxed sessions.
//!
//! Lets LLM agents use familiar `command arg1 arg2` patterns operating on the
//! session's WASI filesystem. All commands are in-process built-ins — there is
//! no subprocess execution and no access to the host shell.
//!
//! Supported features:
//! - Built-ins: `echo`, `cat`, `ls`, `pwd`, `cd`, `mkdir`, `rm`, `cp`, `mv`,
//!   `env`, `export`
//! - Pipes: `echo hello | cat`
//! - Redirection: `>`, `>>`, `<`
//! - Sequencing: `&&` (short-circuit on failure), `;`
//! - Single and double quoted strings
//!
//! CWD is scoped to a single shell invocation (each `command` request starts at
//! `/`). `export KEY=value` writes to the session's `WasiEnv` so the variable
//! is visible to subsequent WASM executions in the same session.

use crate::runtime::wasi::WasiEnv;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum ShellError {
    Parse(String),
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellError::Parse(m) => write!(f, "Parse error: {m}"),
        }
    }
}

/// Run a shell command line in the given session work_dir, capturing output
/// into the session's `WasiEnv` stdout/stderr buffers. Returns the exit code
/// of the final pipeline.
pub fn run_command(
    line: &str,
    work_dir: &Path,
    wasi_env: Arc<Mutex<WasiEnv>>,
) -> Result<i32, ShellError> {
    let statement = parse(line)?;
    let mut shell = Shell::new(work_dir.to_path_buf(), wasi_env);
    shell.execute_statement(&statement)
}

// ── Tokens & AST ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Pipe,
    RedirOut,
    RedirAppend,
    RedirIn,
    AndAnd,
    Semi,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RedirKind {
    Out,
    Append,
    In,
}

#[derive(Debug, Clone)]
struct Redirect {
    kind: RedirKind,
    target: String,
}

#[derive(Debug, Clone)]
struct Command {
    args: Vec<String>,
    redirects: Vec<Redirect>,
}

#[derive(Debug, Clone)]
struct Pipeline {
    commands: Vec<Command>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Connector {
    And,
    Semi,
}

/// A list of pipelines joined by sequencing operators. The Vec entries are
/// `(pipeline, connector_to_next)`, with the last entry's connector ignored.
#[derive(Debug, Clone)]
struct Statement {
    pipelines: Vec<(Pipeline, Connector)>,
}

// ── Tokenizer ─────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Result<Vec<Token>, ShellError> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '|' => {
                chars.next();
                tokens.push(Token::Pipe);
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RedirAppend);
                } else {
                    tokens.push(Token::RedirOut);
                }
            }
            '<' => {
                chars.next();
                tokens.push(Token::RedirIn);
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::AndAnd);
                } else {
                    return Err(ShellError::Parse(
                        "Background '&' is not supported; use '&&' for sequencing".into(),
                    ));
                }
            }
            ';' => {
                chars.next();
                tokens.push(Token::Semi);
            }
            '"' | '\'' => {
                let quote = c;
                chars.next();
                let mut word = String::new();
                let mut closed = false;
                while let Some(&ch) = chars.peek() {
                    if ch == quote {
                        chars.next();
                        closed = true;
                        break;
                    }
                    word.push(ch);
                    chars.next();
                }
                if !closed {
                    return Err(ShellError::Parse(format!("Unclosed {quote} quote")));
                }
                tokens.push(Token::Word(word));
            }
            _ => {
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_whitespace() || matches!(ch, '|' | '>' | '<' | '&' | ';') {
                        break;
                    }
                    word.push(ch);
                    chars.next();
                }
                tokens.push(Token::Word(word));
            }
        }
    }
    Ok(tokens)
}

// ── Parser ────────────────────────────────────────────────────────────

fn parse(line: &str) -> Result<Statement, ShellError> {
    let tokens = tokenize(line)?;
    if tokens.is_empty() {
        return Ok(Statement { pipelines: vec![] });
    }

    // Split by sequencing operators (&& / ;) into pipelines.
    let mut pipelines: Vec<(Pipeline, Connector)> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    for tok in tokens {
        match tok {
            Token::AndAnd | Token::Semi => {
                let conn = if matches!(tok, Token::AndAnd) {
                    Connector::And
                } else {
                    Connector::Semi
                };
                if current.is_empty() {
                    return Err(ShellError::Parse(
                        "Empty command before sequencing operator".into(),
                    ));
                }
                pipelines.push((parse_pipeline(&current)?, conn));
                current.clear();
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        pipelines.push((parse_pipeline(&current)?, Connector::Semi));
    }

    Ok(Statement { pipelines })
}

fn parse_pipeline(tokens: &[Token]) -> Result<Pipeline, ShellError> {
    let mut commands = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    for tok in tokens {
        match tok {
            Token::Pipe => {
                if current.is_empty() {
                    return Err(ShellError::Parse("Empty command before '|'".into()));
                }
                commands.push(parse_command(&current)?);
                current.clear();
            }
            other => current.push(other.clone()),
        }
    }
    if current.is_empty() {
        return Err(ShellError::Parse("Empty command after '|'".into()));
    }
    commands.push(parse_command(&current)?);
    Ok(Pipeline { commands })
}

fn parse_command(tokens: &[Token]) -> Result<Command, ShellError> {
    let mut args = Vec::new();
    let mut redirects = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(w) => {
                args.push(w.clone());
                i += 1;
            }
            Token::RedirOut | Token::RedirAppend | Token::RedirIn => {
                let kind = match &tokens[i] {
                    Token::RedirOut => RedirKind::Out,
                    Token::RedirAppend => RedirKind::Append,
                    Token::RedirIn => RedirKind::In,
                    _ => unreachable!(),
                };
                let target = match tokens.get(i + 1) {
                    Some(Token::Word(w)) => w.clone(),
                    _ => {
                        return Err(ShellError::Parse(
                            "Redirection requires a target filename".into(),
                        ));
                    }
                };
                redirects.push(Redirect { kind, target });
                i += 2;
            }
            _ => {
                return Err(ShellError::Parse(format!(
                    "Unexpected token in command: {:?}",
                    tokens[i]
                )));
            }
        }
    }
    if args.is_empty() {
        return Err(ShellError::Parse("Empty command".into()));
    }
    Ok(Command { args, redirects })
}

// ── Shell execution state ─────────────────────────────────────────────

struct Shell {
    work_dir: PathBuf,
    /// Guest-visible CWD, always starts with `/` and is normalized.
    cwd: String,
    wasi_env: Arc<Mutex<WasiEnv>>,
}

/// Per-command result. Captured stdout/stderr are returned so pipelines can
/// stitch them together before flushing to the session buffers.
struct CmdOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
}

impl Shell {
    fn new(work_dir: PathBuf, wasi_env: Arc<Mutex<WasiEnv>>) -> Self {
        Self {
            work_dir,
            cwd: "/".to_string(),
            wasi_env,
        }
    }

    fn execute_statement(&mut self, stmt: &Statement) -> Result<i32, ShellError> {
        let mut last_code = 0;
        for (i, (pipeline, conn)) in stmt.pipelines.iter().enumerate() {
            last_code = self.execute_pipeline(pipeline)?;
            // Short-circuit on `&&` if previous failed and there is a next one.
            let is_last = i + 1 == stmt.pipelines.len();
            if !is_last && *conn == Connector::And && last_code != 0 {
                break;
            }
        }
        Ok(last_code)
    }

    fn execute_pipeline(&mut self, pipeline: &Pipeline) -> Result<i32, ShellError> {
        // Determine pipeline stdin from any leading `<` redirect on the first
        // command, and the final stdout destination from the last command's
        // `>`/`>>` redirect. Stderr from every command is flushed to the
        // session's WASI buffer regardless.
        let mut stdin_bytes: Vec<u8> = Vec::new();
        let first = &pipeline.commands[0];
        for r in &first.redirects {
            if r.kind == RedirKind::In {
                let host = self.resolve_host(&r.target)?;
                match std::fs::read(&host) {
                    Ok(b) => stdin_bytes = b,
                    Err(e) => {
                        self.append_stderr(&format!("{}: {e}\n", r.target));
                        return Ok(1);
                    }
                }
            }
        }

        let last_idx = pipeline.commands.len() - 1;
        let mut current_stdin = stdin_bytes;
        let mut exit_code = 0;

        for (i, cmd) in pipeline.commands.iter().enumerate() {
            let out = self.run_builtin(cmd, &current_stdin)?;
            exit_code = out.exit_code;
            if !out.stderr.is_empty() {
                self.append_stderr_bytes(&out.stderr);
            }

            let is_last = i == last_idx;
            if is_last {
                // Apply the last command's output redirection, or flush.
                let mut wrote_to_file = false;
                for r in &cmd.redirects {
                    if matches!(r.kind, RedirKind::Out | RedirKind::Append) {
                        let host = self.resolve_host(&r.target)?;
                        if let Some(parent) = host.parent() {
                            if let Err(e) = std::fs::create_dir_all(parent) {
                                self.append_stderr(&format!("{}: {e}\n", r.target));
                                return Ok(1);
                            }
                        }
                        let write_res = match r.kind {
                            RedirKind::Append => std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&host)
                                .and_then(|mut f| {
                                    use std::io::Write;
                                    f.write_all(&out.stdout)
                                }),
                            RedirKind::Out => std::fs::write(&host, &out.stdout),
                            RedirKind::In => unreachable!(),
                        };
                        if let Err(e) = write_res {
                            self.append_stderr(&format!("{}: {e}\n", r.target));
                            return Ok(1);
                        }
                        wrote_to_file = true;
                    }
                }
                if !wrote_to_file {
                    self.append_stdout_bytes(&out.stdout);
                }
            } else {
                current_stdin = out.stdout;
            }
        }

        Ok(exit_code)
    }

    fn run_builtin(&mut self, cmd: &Command, stdin: &[u8]) -> Result<CmdOutput, ShellError> {
        let name = cmd.args[0].as_str();
        let args = &cmd.args[1..];
        let out = match name {
            "echo" => builtin_echo(args),
            "cat" => self.builtin_cat(args, stdin),
            "ls" => self.builtin_ls(args),
            "pwd" => self.builtin_pwd(),
            "cd" => self.builtin_cd(args),
            "mkdir" => self.builtin_mkdir(args),
            "rm" => self.builtin_rm(args),
            "cp" => self.builtin_cp(args),
            "mv" => self.builtin_mv(args),
            "env" => self.builtin_env(),
            "export" => self.builtin_export(args),
            other => CmdOutput {
                stdout: Vec::new(),
                stderr: format!("{other}: command not found\n").into_bytes(),
                exit_code: 127,
            },
        };
        Ok(out)
    }

    // ── Path helpers ──────────────────────────────────────────────

    /// Normalize a guest path (absolute or relative to cwd) to an absolute
    /// guest path that stays within `/`.
    fn resolve_guest(&self, path: &str) -> Result<String, ShellError> {
        let combined = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            PathBuf::from(&self.cwd).join(path)
        };
        let mut parts: Vec<String> = Vec::new();
        for c in combined.components() {
            match c {
                Component::RootDir => parts.clear(),
                Component::CurDir => {}
                Component::ParentDir => {
                    if parts.is_empty() {
                        return Err(ShellError::Parse(format!(
                            "Path escapes session root: {path}"
                        )));
                    }
                    parts.pop();
                }
                Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
                Component::Prefix(_) => {
                    return Err(ShellError::Parse(format!(
                        "Path prefix not supported: {path}"
                    )));
                }
            }
        }
        let mut out = String::from("/");
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                out.push('/');
            }
            out.push_str(p);
        }
        Ok(out)
    }

    fn resolve_host(&self, path: &str) -> Result<PathBuf, ShellError> {
        let guest = self.resolve_guest(path)?;
        let stripped = guest.trim_start_matches('/');
        Ok(if stripped.is_empty() {
            self.work_dir.clone()
        } else {
            self.work_dir.join(stripped)
        })
    }

    // ── stdout/stderr plumbing ────────────────────────────────────

    fn append_stdout_bytes(&self, bytes: &[u8]) {
        if let Ok(mut env) = self.wasi_env.lock() {
            env.stdout_mut().extend_from_slice(bytes);
        }
    }

    fn append_stderr_bytes(&self, bytes: &[u8]) {
        if let Ok(mut env) = self.wasi_env.lock() {
            env.stderr_mut().extend_from_slice(bytes);
        }
    }

    fn append_stderr(&self, s: &str) {
        self.append_stderr_bytes(s.as_bytes());
    }

    // ── Builtins ──────────────────────────────────────────────────

    fn builtin_cat(&self, args: &[String], stdin: &[u8]) -> CmdOutput {
        if args.is_empty() {
            return CmdOutput {
                stdout: stdin.to_vec(),
                stderr: Vec::new(),
                exit_code: 0,
            };
        }
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let mut code = 0;
        for path in args {
            let host = match self.resolve_host(path) {
                Ok(p) => p,
                Err(e) => {
                    err.extend_from_slice(format!("cat: {path}: {e}\n").as_bytes());
                    code = 1;
                    continue;
                }
            };
            match std::fs::read(&host) {
                Ok(b) => out.extend_from_slice(&b),
                Err(e) => {
                    err.extend_from_slice(format!("cat: {path}: {e}\n").as_bytes());
                    code = 1;
                }
            }
        }
        CmdOutput {
            stdout: out,
            stderr: err,
            exit_code: code,
        }
    }

    fn builtin_ls(&self, args: &[String]) -> CmdOutput {
        let target = args.first().map(|s| s.as_str()).unwrap_or(".");
        let host = match self.resolve_host(target) {
            Ok(p) => p,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("ls: {target}: {e}\n").into_bytes(),
                    exit_code: 1,
                };
            }
        };
        let entries = match std::fs::read_dir(&host) {
            Ok(e) => e,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("ls: {target}: {e}\n").into_bytes(),
                    exit_code: 1,
                };
            }
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        let mut out = String::new();
        for n in &names {
            out.push_str(n);
            out.push('\n');
        }
        CmdOutput {
            stdout: out.into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
        }
    }

    fn builtin_pwd(&self) -> CmdOutput {
        let mut s = self.cwd.clone();
        s.push('\n');
        CmdOutput {
            stdout: s.into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
        }
    }

    fn builtin_cd(&mut self, args: &[String]) -> CmdOutput {
        let target = args.first().map(|s| s.as_str()).unwrap_or("/");
        let resolved = match self.resolve_guest(target) {
            Ok(p) => p,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("cd: {target}: {e}\n").into_bytes(),
                    exit_code: 1,
                };
            }
        };
        let host = if resolved == "/" {
            self.work_dir.clone()
        } else {
            self.work_dir.join(resolved.trim_start_matches('/'))
        };
        if !host.is_dir() {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: format!("cd: {target}: Not a directory\n").into_bytes(),
                exit_code: 1,
            };
        }
        self.cwd = resolved;
        CmdOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: 0,
        }
    }

    fn builtin_mkdir(&self, args: &[String]) -> CmdOutput {
        let mut err = Vec::new();
        let mut code = 0;
        // Support `-p` flag to silently succeed when the directory exists.
        let (recursive, paths): (bool, Vec<&String>) =
            if args.first().map(|s| s.as_str()) == Some("-p") {
                (true, args[1..].iter().collect())
            } else {
                (false, args.iter().collect())
            };
        if paths.is_empty() {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: b"mkdir: missing operand\n".to_vec(),
                exit_code: 1,
            };
        }
        for p in paths {
            let host = match self.resolve_host(p) {
                Ok(h) => h,
                Err(e) => {
                    err.extend_from_slice(format!("mkdir: {p}: {e}\n").as_bytes());
                    code = 1;
                    continue;
                }
            };
            let res = if recursive {
                std::fs::create_dir_all(&host)
            } else {
                std::fs::create_dir(&host)
            };
            if let Err(e) = res {
                err.extend_from_slice(format!("mkdir: {p}: {e}\n").as_bytes());
                code = 1;
            }
        }
        CmdOutput {
            stdout: Vec::new(),
            stderr: err,
            exit_code: code,
        }
    }

    fn builtin_rm(&self, args: &[String]) -> CmdOutput {
        let (recursive, paths): (bool, Vec<&String>) = match args.first().map(|s| s.as_str()) {
            Some("-r") | Some("-rf") | Some("-fr") => (true, args[1..].iter().collect()),
            _ => (false, args.iter().collect()),
        };
        if paths.is_empty() {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: b"rm: missing operand\n".to_vec(),
                exit_code: 1,
            };
        }
        let mut err = Vec::new();
        let mut code = 0;
        for p in paths {
            let host = match self.resolve_host(p) {
                Ok(h) => h,
                Err(e) => {
                    err.extend_from_slice(format!("rm: {p}: {e}\n").as_bytes());
                    code = 1;
                    continue;
                }
            };
            let res = if host.is_dir() {
                if recursive {
                    std::fs::remove_dir_all(&host)
                } else {
                    err.extend_from_slice(format!("rm: {p}: is a directory\n").as_bytes());
                    code = 1;
                    continue;
                }
            } else {
                std::fs::remove_file(&host)
            };
            if let Err(e) = res {
                err.extend_from_slice(format!("rm: {p}: {e}\n").as_bytes());
                code = 1;
            }
        }
        CmdOutput {
            stdout: Vec::new(),
            stderr: err,
            exit_code: code,
        }
    }

    fn builtin_cp(&self, args: &[String]) -> CmdOutput {
        if args.len() != 2 {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: b"cp: expected exactly 2 arguments (source, dest)\n".to_vec(),
                exit_code: 1,
            };
        }
        let src = match self.resolve_host(&args[0]) {
            Ok(p) => p,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("cp: {}: {e}\n", args[0]).into_bytes(),
                    exit_code: 1,
                };
            }
        };
        let dst = match self.resolve_host(&args[1]) {
            Ok(p) => p,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("cp: {}: {e}\n", args[1]).into_bytes(),
                    exit_code: 1,
                };
            }
        };
        if let Err(e) = std::fs::copy(&src, &dst) {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: format!("cp: {} -> {}: {e}\n", args[0], args[1]).into_bytes(),
                exit_code: 1,
            };
        }
        CmdOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: 0,
        }
    }

    fn builtin_mv(&self, args: &[String]) -> CmdOutput {
        if args.len() != 2 {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: b"mv: expected exactly 2 arguments (source, dest)\n".to_vec(),
                exit_code: 1,
            };
        }
        let src = match self.resolve_host(&args[0]) {
            Ok(p) => p,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("mv: {}: {e}\n", args[0]).into_bytes(),
                    exit_code: 1,
                };
            }
        };
        let dst = match self.resolve_host(&args[1]) {
            Ok(p) => p,
            Err(e) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: format!("mv: {}: {e}\n", args[1]).into_bytes(),
                    exit_code: 1,
                };
            }
        };
        if let Err(e) = std::fs::rename(&src, &dst) {
            return CmdOutput {
                stdout: Vec::new(),
                stderr: format!("mv: {} -> {}: {e}\n", args[0], args[1]).into_bytes(),
                exit_code: 1,
            };
        }
        CmdOutput {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: 0,
        }
    }

    fn builtin_env(&self) -> CmdOutput {
        let env = match self.wasi_env.lock() {
            Ok(e) => e,
            Err(_) => {
                return CmdOutput {
                    stdout: Vec::new(),
                    stderr: b"env: lock error\n".to_vec(),
                    exit_code: 1,
                };
            }
        };
        let mut pairs: Vec<(String, String)> = env
            .env_vars()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        drop(env);
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = String::new();
        for (k, v) in pairs {
            out.push_str(&k);
            out.push('=');
            out.push_str(&v);
            out.push('\n');
        }
        CmdOutput {
            stdout: out.into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
        }
    }

    fn builtin_export(&mut self, args: &[String]) -> CmdOutput {
        if args.is_empty() {
            // Same behavior as `env` when called with no arguments.
            return self.builtin_env();
        }
        let mut err = Vec::new();
        let mut code = 0;
        for a in args {
            match a.split_once('=') {
                Some((k, v)) if !k.is_empty() => {
                    if let Ok(mut env) = self.wasi_env.lock() {
                        env.add_env(k.to_string(), v.to_string());
                    } else {
                        return CmdOutput {
                            stdout: Vec::new(),
                            stderr: b"export: lock error\n".to_vec(),
                            exit_code: 1,
                        };
                    }
                }
                _ => {
                    err.extend_from_slice(
                        format!("export: '{a}': not a valid KEY=value assignment\n").as_bytes(),
                    );
                    code = 1;
                }
            }
        }
        CmdOutput {
            stdout: Vec::new(),
            stderr: err,
            exit_code: code,
        }
    }
}

fn builtin_echo(args: &[String]) -> CmdOutput {
    let mut out = args.join(" ");
    out.push('\n');
    CmdOutput {
        stdout: out.into_bytes(),
        stderr: Vec::new(),
        exit_code: 0,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_env() -> Arc<Mutex<WasiEnv>> {
        Arc::new(Mutex::new(WasiEnv::new()))
    }

    fn make_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wasmrun_shell_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn stdout(env: &Arc<Mutex<WasiEnv>>) -> String {
        String::from_utf8(env.lock().unwrap().get_stdout()).unwrap()
    }

    fn stderr(env: &Arc<Mutex<WasiEnv>>) -> String {
        String::from_utf8(env.lock().unwrap().get_stderr()).unwrap()
    }

    // ── Tokenizer ────────────────────────────────────────────────

    #[test]
    fn tokenize_simple() {
        let toks = tokenize("echo hello world").unwrap();
        assert_eq!(
            toks,
            vec![
                Token::Word("echo".into()),
                Token::Word("hello".into()),
                Token::Word("world".into()),
            ]
        );
    }

    #[test]
    fn tokenize_double_quotes() {
        let toks = tokenize(r#"echo "hello world""#).unwrap();
        assert_eq!(
            toks,
            vec![
                Token::Word("echo".into()),
                Token::Word("hello world".into())
            ]
        );
    }

    #[test]
    fn tokenize_single_quotes() {
        let toks = tokenize("echo 'a b'").unwrap();
        assert_eq!(
            toks,
            vec![Token::Word("echo".into()), Token::Word("a b".into())]
        );
    }

    #[test]
    fn tokenize_pipe_and_redirect() {
        let toks = tokenize("echo a | cat > out.txt").unwrap();
        assert_eq!(
            toks,
            vec![
                Token::Word("echo".into()),
                Token::Word("a".into()),
                Token::Pipe,
                Token::Word("cat".into()),
                Token::RedirOut,
                Token::Word("out.txt".into()),
            ]
        );
    }

    #[test]
    fn tokenize_append_and_and() {
        let toks = tokenize("a >> b && c").unwrap();
        assert_eq!(
            toks,
            vec![
                Token::Word("a".into()),
                Token::RedirAppend,
                Token::Word("b".into()),
                Token::AndAnd,
                Token::Word("c".into()),
            ]
        );
    }

    #[test]
    fn tokenize_unclosed_quote() {
        assert!(tokenize(r#"echo "oops"#).is_err());
    }

    #[test]
    fn tokenize_lone_ampersand_rejected() {
        assert!(tokenize("foo &").is_err());
    }

    // ── Builtins (plan-spec tests) ───────────────────────────────

    #[test]
    fn plan_test_echo_hello() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("echo hello", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "hello\n");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn plan_test_ls_root_lists_dirs() {
        let work = make_dir();
        std::fs::create_dir(work.join("sub1")).unwrap();
        std::fs::create_dir(work.join("sub2")).unwrap();
        let env = fresh_env();
        let code = run_command("ls /", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        let out = stdout(&env);
        assert!(out.contains("sub1"), "missing sub1 in {out:?}");
        assert!(out.contains("sub2"), "missing sub2 in {out:?}");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn plan_test_echo_redirect_then_cat() {
        let work = make_dir();
        let env = fresh_env();
        let code =
            run_command("echo hello > test.txt && cat test.txt", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "hello\n");
        let on_disk = std::fs::read_to_string(work.join("test.txt")).unwrap();
        assert_eq!(on_disk, "hello\n");
        let _ = std::fs::remove_dir_all(&work);
    }

    // ── Additional builtin behavior ──────────────────────────────

    #[test]
    fn pwd_is_root_by_default() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("pwd", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "/\n");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn cd_then_pwd() {
        let work = make_dir();
        std::fs::create_dir(work.join("sub")).unwrap();
        let env = fresh_env();
        let code = run_command("cd sub && pwd", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "/sub\n");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn cd_into_nonexistent_fails() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("cd nowhere", &work, env.clone()).unwrap();
        assert_eq!(code, 1);
        assert!(stderr(&env).contains("Not a directory"));
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn cd_escape_is_rejected() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("cd ..", &work, env.clone()).unwrap();
        assert_eq!(code, 1);
        assert!(stderr(&env).contains("escapes"));
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn cat_pipe_echo() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("echo hello | cat", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "hello\n");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn cat_with_redirect_in() {
        let work = make_dir();
        std::fs::write(work.join("in.txt"), "from-file").unwrap();
        let env = fresh_env();
        let code = run_command("cat < in.txt", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "from-file");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn redirect_append() {
        let work = make_dir();
        let env = fresh_env();
        run_command("echo one > log.txt", &work, env.clone()).unwrap();
        run_command("echo two >> log.txt", &work, env.clone()).unwrap();
        let on_disk = std::fs::read_to_string(work.join("log.txt")).unwrap();
        assert_eq!(on_disk, "one\ntwo\n");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn mkdir_and_ls() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("mkdir foo && ls", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert!(stdout(&env).contains("foo"));
        assert!(work.join("foo").is_dir());
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn mkdir_p_idempotent() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("mkdir -p a/b/c", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert!(work.join("a/b/c").is_dir());
        // Second invocation succeeds with -p
        let env2 = fresh_env();
        let code2 = run_command("mkdir -p a/b/c", &work, env2).unwrap();
        assert_eq!(code2, 0);
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn rm_file() {
        let work = make_dir();
        std::fs::write(work.join("x.txt"), "x").unwrap();
        let env = fresh_env();
        let code = run_command("rm x.txt", &work, env).unwrap();
        assert_eq!(code, 0);
        assert!(!work.join("x.txt").exists());
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn rm_dir_requires_recursive() {
        let work = make_dir();
        std::fs::create_dir(work.join("d")).unwrap();
        let env = fresh_env();
        let code = run_command("rm d", &work, env.clone()).unwrap();
        assert_eq!(code, 1);
        assert!(stderr(&env).contains("is a directory"));

        let env2 = fresh_env();
        let code2 = run_command("rm -r d", &work, env2).unwrap();
        assert_eq!(code2, 0);
        assert!(!work.join("d").exists());
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn cp_and_mv() {
        let work = make_dir();
        std::fs::write(work.join("a.txt"), "hello").unwrap();
        let env = fresh_env();
        let code = run_command("cp a.txt b.txt && mv b.txt c.txt", &work, env).unwrap();
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(work.join("a.txt")).unwrap(),
            "hello"
        );
        assert!(!work.join("b.txt").exists());
        assert_eq!(
            std::fs::read_to_string(work.join("c.txt")).unwrap(),
            "hello"
        );
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn export_and_env() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("export FOO=bar && env", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert!(stdout(&env).contains("FOO=bar"));
        // Also verify it's actually in the WasiEnv (visible to subsequent execs)
        let pairs = env.lock().unwrap().env_vars().to_vec();
        assert!(pairs.iter().any(|(k, v)| k == "FOO" && v == "bar"));
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn export_rejects_bare_name() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("export NOEQUALS", &work, env.clone()).unwrap();
        assert_eq!(code, 1);
        assert!(stderr(&env).contains("not a valid KEY=value"));
        let _ = std::fs::remove_dir_all(&work);
    }

    // ── Chaining & short-circuit ─────────────────────────────────

    #[test]
    fn and_short_circuits_on_failure() {
        let work = make_dir();
        let env = fresh_env();
        // First command fails (no such file), second must not run.
        let code =
            run_command("cat missing.txt && echo should-not-run", &work, env.clone()).unwrap();
        assert_eq!(code, 1);
        assert!(!stdout(&env).contains("should-not-run"));
        assert!(stderr(&env).contains("missing.txt"));
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn semi_runs_both_regardless() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("cat missing.txt ; echo after", &work, env.clone()).unwrap();
        // Final command's exit code wins
        assert_eq!(code, 0);
        assert!(stdout(&env).contains("after"));
        let _ = std::fs::remove_dir_all(&work);
    }

    // ── Unknown command ──────────────────────────────────────────

    #[test]
    fn unknown_command_returns_127() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("does-not-exist", &work, env.clone()).unwrap();
        assert_eq!(code, 127);
        assert!(stderr(&env).contains("command not found"));
        let _ = std::fs::remove_dir_all(&work);
    }

    // ── Path resolution ──────────────────────────────────────────

    #[test]
    fn resolve_guest_normalizes() {
        let work = make_dir();
        let s = Shell::new(work.clone(), fresh_env());
        assert_eq!(s.resolve_guest("/").unwrap(), "/");
        assert_eq!(s.resolve_guest("/a/b").unwrap(), "/a/b");
        assert_eq!(s.resolve_guest("/a/./b").unwrap(), "/a/b");
        assert_eq!(s.resolve_guest("/a/b/../c").unwrap(), "/a/c");
        assert!(s.resolve_guest("/..").is_err());
        let _ = std::fs::remove_dir_all(&work);
    }

    // ── Empty input ──────────────────────────────────────────────

    #[test]
    fn empty_input_is_no_op() {
        let work = make_dir();
        let env = fresh_env();
        let code = run_command("", &work, env.clone()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(stdout(&env), "");
        let _ = std::fs::remove_dir_all(&work);
    }
}
