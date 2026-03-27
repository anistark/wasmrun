//! Agent mode: CLI command handler for `wasmrun agent`.

use crate::agent::server::{AgentConfig, AgentServer};
use crate::agent::session::SessionConfig;
use crate::error::Result;
use std::time::Duration;

pub fn handle_agent_command(
    port: u16,
    timeout: u64,
    max_sessions: usize,
    max_memory: u32,
    allow_cors: bool,
    verbose: bool,
) -> Result<()> {
    let config = AgentConfig {
        port,
        session_config: SessionConfig {
            default_timeout: Duration::from_secs(timeout),
            max_sessions,
            cleanup_interval: Duration::from_secs(30),
        },
        allow_cors,
        verbose,
        max_memory_mb: max_memory,
    };

    let server = AgentServer::new(config);
    server.start()
}
