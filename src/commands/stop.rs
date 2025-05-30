use crate::server;
use crate::ui::{print_info, print_status, print_success};

/// Handle stop command
pub fn handle_stop_command() -> Result<(), String> {
    if !server::is_server_running() {
        print_info("No Chakra server is currently running");
        return Ok(());
    }

    print_status("Stopping Chakra server...");

    match server::stop_existing_server() {
        Ok(()) => {
            print_success("Chakra Server Stopped", "Server terminated successfully");
            Ok(())
        }
        Err(e) => Err(format!("Error stopping server: {}", e)),
    }
}
