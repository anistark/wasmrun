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

    match server::stop_existing_server() {
        Ok(()) => {
            print_success("Wasmrun Server Stopped", "Server terminated successfully");
            Ok(())
        }
        Err(e) => Err(e),
    }
}
