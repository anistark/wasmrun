//! Agent mode: CLI command handler for `wasmrun agent`.

use crate::agent::limits::ResourceLimits;
use crate::agent::server::{AgentConfig, AgentServer};
use crate::agent::session::SessionConfig;
use crate::error::Result;
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
    allow_cors: bool,
    verbose: bool,
) -> Result<()> {
    let limits =
        ResourceLimits::from_cli(max_memory, max_fuel, max_output, max_file_size, max_disk);

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
    };

    let server = AgentServer::new(config);
    server.start()
}
