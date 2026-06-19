//! Agent mode: CLI command handler for `wasmrun agent`.

use crate::agent::auth::{self, AuthConfig};
use crate::agent::limits::ResourceLimits;
use crate::agent::server::{AgentConfig, AgentServer};
use crate::agent::session::SessionConfig;
use crate::error::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[allow(clippy::too_many_arguments)]
pub fn handle_agent_command(
    port: u16,
    timeout: u64,
    max_sessions: usize,
    max_memory: u32,
    max_fuel: u64,
    max_output: u32,
    max_file_size: u32,
    max_disk: u32,
    max_body: u32,
    max_concurrent_exec: usize,
    allow_cors: bool,
    verbose: bool,
    auth_config: Option<&str>,
    hash_key: Option<&str>,
) -> Result<()> {
    // `--hash-key` is a standalone helper: print sha256(key) and exit without
    // starting the server, so operators can populate the auth config.
    if let Some(key) = hash_key {
        println!("{}", auth::hash_key(key));
        return Ok(());
    }

    // Load the auth config when requested. Abort startup on any error rather than
    // silently running open when auth was asked for. The path is retained so the
    // server can watch it for live reloads.
    let auth = match auth_config {
        Some(path) => Some(Arc::new(AuthConfig::load(Path::new(path))?)),
        None => None,
    };
    let auth_path = auth_config.map(PathBuf::from);

    let limits =
        ResourceLimits::from_cli(max_memory, max_fuel, max_output, max_file_size, max_disk);

    // 0 = unlimited, matching the resource-limit flag convention.
    let max_body_bytes = (max_body != 0).then(|| max_body as usize * 1024 * 1024);

    let config = AgentConfig {
        port,
        session_config: SessionConfig {
            default_timeout: Duration::from_secs(timeout),
            max_sessions,
            cleanup_interval: Duration::from_secs(30),
            limits,
        },
        allow_cors,
        verbose,
        max_body_bytes,
        max_concurrent_exec,
        auth,
        auth_path,
    };

    let server = AgentServer::new(config);
    server.start()
}
