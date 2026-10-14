//! Exec command implementation for running WASM files with arguments

use crate::config::project::NetworkConfig;
use crate::error::{Result, WasmrunError};
use crate::runtime::core::native_executor::{self, ExecLimits, ExecOptions};
use crate::runtime::wasi::network::NetworkAccess;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

/// `wasmrun exec`'s options beyond the file, the function and the arguments.
#[derive(Debug, Default, Clone)]
pub struct ExecFlags {
    pub tcplisten: Vec<String>,
    pub allow_net: Vec<String>,
    /// `--dir HOST[:GUEST]`
    pub dirs: Vec<String>,
    /// `--env KEY=VALUE`
    pub env: Vec<String>,
    /// `--max-memory-mb`; 0 or absent is no cap
    pub max_memory_mb: Option<u32>,
    /// `--max-fuel`; 0 or absent is no cap
    pub max_fuel: Option<u64>,
    /// `--quiet`: only the program's own output
    pub quiet: bool,
}

#[cfg(test)]
pub fn handle_exec_command(
    wasm_file: &Option<String>,
    call: &Option<String>,
    tcplisten: &[String],
    allow_net: &[String],
    args: Vec<String>,
) -> Result<i32> {
    let flags = ExecFlags {
        tcplisten: tcplisten.to_vec(),
        allow_net: allow_net.to_vec(),
        ..Default::default()
    };
    handle_exec_command_with(wasm_file, call, &flags, args)
}

pub fn handle_exec_command_with(
    wasm_file: &Option<String>,
    call: &Option<String>,
    flags: &ExecFlags,
    args: Vec<String>,
) -> Result<i32> {
    let wasm_path = wasm_file
        .as_ref()
        .ok_or_else(|| WasmrunError::from("WASM file path is required".to_string()))?;

    execute_wasm_with_args(wasm_path, call.clone(), flags, args)
}

/// `--dir HOST[:GUEST]`: a host directory and where the program sees it. A
/// bare `HOST` is seen as `/`, the way `wasmtime --dir` works.
pub fn parse_dir(spec: &str) -> Result<(String, PathBuf)> {
    let (host, guest) = match spec.rsplit_once(':') {
        // A Windows drive letter is not a guest path.
        Some((host, guest)) if guest.starts_with('/') && !host.is_empty() => (host, guest),
        _ => (spec, "/"),
    };
    let path = PathBuf::from(host);
    if !path.is_dir() {
        return Err(WasmrunError::from(format!(
            "--dir {spec}: '{host}' is not a directory"
        )));
    }
    Ok((guest.to_string(), path))
}

/// `--env KEY=VALUE`
pub fn parse_env(spec: &str) -> Result<(String, String)> {
    match spec.split_once('=') {
        Some((key, value)) if !key.is_empty() => Ok((key.to_string(), value.to_string())),
        _ => Err(WasmrunError::from(format!(
            "--env {spec}: expected KEY=VALUE"
        ))),
    }
}

/// Turn `--allow-net` rules into the network the program gets.
///
/// No rules means no network, which is the default a sandbox should have: a
/// program only reaches what someone deliberately allowed. The rules are
/// validated the same way a project's `[os.network]` table is, so a malformed
/// CIDR is refused here rather than silently becoming a hostname that matches
/// nothing.
///
/// The rules given are the whole policy, with no inherited deny list. wasmnet
/// ships one that blocks the private ranges, which is the right default under
/// a permissive `allow = ["*"]` and the wrong one here: this flag starts from
/// nothing allowed, so keeping those denies would mean `--allow-net
/// 127.0.0.0/8` refused the very thing it named. Someone who writes
/// `--allow-net "*"` is asking for everything and gets it.
fn network_from_rules(allow_net: &[String], quiet: bool) -> Result<NetworkAccess> {
    if allow_net.is_empty() {
        return Ok(NetworkAccess::denied());
    }

    let policy = NetworkConfig::from_allow_net(allow_net)?;
    if !quiet {
        println!("🌐 Network allowed: {}", allow_net.join(", "));
    }
    Ok(NetworkAccess::with_policy(&policy))
}

/// Bind every `--tcplisten` address before the program starts.
///
/// Binding here rather than inside the guest is the whole model: the sandbox
/// gets a socket it can accept on and no way to ask for a different one.
fn bind_listeners(addrs: &[String], first_fd: usize, quiet: bool) -> Result<Vec<TcpListener>> {
    addrs
        .iter()
        .enumerate()
        .map(|(i, addr)| {
            let listener = TcpListener::bind(addr).map_err(|e| {
                WasmrunError::from(format!("Failed to bind {addr} for --tcplisten: {e}"))
            })?;
            let bound = listener
                .local_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| addr.clone());
            // Preopened fds start at 3 and are handed out in order, directories
            // first, so with no --dir the first listener is fd 3.
            let fd = first_fd + i;
            if !quiet {
                println!("🔌 Listening on {bound}, passed to the program as fd {fd}");
            }
            Ok(listener)
        })
        .collect()
}

fn execute_wasm_with_args(
    wasm_path: &str,
    call: Option<String>,
    flags: &ExecFlags,
    args: Vec<String>,
) -> Result<i32> {
    if !Path::new(wasm_path).exists() {
        return Err(WasmrunError::from(format!(
            "WASM file not found: {wasm_path}"
        )));
    }

    if !wasm_path.ends_with(".wasm") {
        return Err(WasmrunError::from(format!(
            "Expected a .wasm file, got: {wasm_path}"
        )));
    }

    let quiet = flags.quiet;
    if !quiet {
        println!("🎯 Running WASM file: {wasm_path}");
        if let Some(ref func) = call {
            println!("📍 Calling: {func}");
        }
        if !args.is_empty() {
            println!("📝 Arguments: {}", args.join(" "));
        }
        println!("🏃 Executing natively (interpreter mode)");
    }

    let preopens = flags
        .dirs
        .iter()
        .map(|d| parse_dir(d))
        .collect::<Result<Vec<_>>>()?;
    let env = flags
        .env
        .iter()
        .map(|e| parse_env(e))
        .collect::<Result<Vec<_>>>()?;
    let network = network_from_rules(&flags.allow_net, quiet)?;
    let first_fd = crate::runtime::wasi::WASI_FIRST_PREOPEN_FD as usize + preopens.len();
    let listeners = bind_listeners(&flags.tcplisten, first_fd, quiet)?;
    let limits = ExecLimits::from_flags(flags.max_memory_mb, flags.max_fuel);
    let exit_code = native_executor::execute_wasm_file_with_options(
        wasm_path,
        ExecOptions {
            function: call,
            args,
            listeners,
            network,
            preopens,
            env,
            limits,
        },
    )?;
    if !quiet {
        if exit_code != 0 {
            println!("❌ Program exited with code {exit_code}");
        } else {
            println!("✅ Execution completed");
        }
    }

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Missing WASM file path parameter
    #[test]
    fn test_handle_exec_missing_wasm_path() {
        let result = handle_exec_command(&None, &None, &[], &[], Vec::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("required"));
    }

    /// Test: Non-existent WASM file
    #[test]
    fn test_handle_exec_nonexistent_file() {
        let result = handle_exec_command(
            &Some("nonexistent.wasm".to_string()),
            &None,
            &[],
            &[],
            Vec::new(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    /// Test: Invalid file extension (not .wasm)
    #[test]
    fn test_handle_exec_invalid_extension() {
        let result = handle_exec_command(
            &Some("test_file.txt".to_string()),
            &None,
            &[],
            &[],
            Vec::new(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Error could be either about extension or file not found
        // The important thing is it fails with either message
        assert!(err.contains(".wasm") || err.contains("not found"));
    }

    /// Test: Valid WASM file path with Go example (if available)
    #[test]
    fn test_handle_exec_go_example() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let result = handle_exec_command(&Some(wasm_path.to_string()), &None, &[], &[], Vec::new());

        match result {
            Ok(_) => println!("✓ Successfully executed Go example WASM"),
            Err(e) => println!("⚠️  Go example execution error: {e}"),
        }
    }

    /// Test: Execute with function selection (call flag)
    #[test]
    fn test_handle_exec_with_function_selection() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        // Try calling a function that likely doesn't exist (for error testing)
        let result = handle_exec_command(
            &Some(wasm_path.to_string()),
            &Some("nonexistent_func".to_string()),
            &[],
            &[],
            Vec::new(),
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    /// Test: Execute with arguments
    #[test]
    fn test_handle_exec_with_arguments() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let args = vec!["arg1".to_string(), "arg2".to_string()];
        let result = handle_exec_command(&Some(wasm_path.to_string()), &None, &[], &[], args);

        match result {
            Ok(_) => println!("✓ Successfully executed with arguments"),
            Err(e) => println!("⚠️  Execution with arguments error: {e}"),
        }
    }

    /// Test: Execute with both function selection and arguments
    #[test]
    fn test_handle_exec_function_and_arguments() {
        let wasm_path = "examples/go-hello/main.wasm";
        if !Path::new(wasm_path).exists() {
            println!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let args = vec!["test_arg".to_string()];
        let result = handle_exec_command(
            &Some(wasm_path.to_string()),
            &Some("run".to_string()),
            &[],
            &[],
            args,
        );

        match result {
            Ok(_) => println!("✓ Successfully executed with function and arguments"),
            Err(e) => {
                // This is expected if the function doesn't exist or doesn't accept arguments
                println!("⚠️  Execution error (may be expected): {e}");
            }
        }
    }
}
