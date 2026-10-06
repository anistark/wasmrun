use crate::error::Result;
use crate::server;
use crate::ui::{print_info, print_status, print_success};

/// Handle stop command
pub fn handle_stop_command() -> Result<()> {
    if !server::is_server_running() {
        print_info("No Wasmrun server is currently running");
        return Ok(());
    }

    print_status("Stopping Wasmrun server...");

    let stopped = server::stop_running_servers()?;
    let detail = stopped
        .iter()
        .map(|s| format!("{} (PID {})", s.url, s.pid))
        .collect::<Vec<_>>()
        .join(", ");
    print_success("Wasmrun Server Stopped", &detail);
    Ok(())
}
