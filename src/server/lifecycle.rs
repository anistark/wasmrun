use crate::config::PID_FILE;
use crate::error::{Result, ServerError, WasmrunError};

/// Check if a wasmrun server is currently running
pub fn is_server_running() -> bool {
    if !std::path::Path::new(PID_FILE).exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(PID_FILE) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            let ps_command = std::process::Command::new("ps")
                .arg("-p")
                .arg(pid.to_string())
                .output();

            if let Ok(output) = ps_command {
                return output.status.success()
                    && String::from_utf8_lossy(&output.stdout).lines().count() > 1;
            }
        }
    }

    false
}

/// Stop an existing wasmrun server if one is running
pub fn stop_existing_server() -> Result<()> {
    if !is_server_running() {
        if std::path::Path::new(PID_FILE).exists() {
            std::fs::remove_file(PID_FILE).map_err(|e| {
                WasmrunError::Server(ServerError::StopFailed {
                    pid: 0,
                    reason: format!("Failed to remove stale PID file: {e}"),
                })
            })?;
        }
        return Err(WasmrunError::Server(ServerError::NotRunning));
    }

    let pid_str = std::fs::read_to_string(PID_FILE).map_err(|e| {
        WasmrunError::Server(ServerError::StopFailed {
            pid: 0,
            reason: format!("Failed to read PID file: {e}"),
        })
    })?;

    let pid = pid_str.trim().parse::<u32>().map_err(|e| {
        WasmrunError::Server(ServerError::StopFailed {
            pid: 0,
            reason: format!("Failed to parse PID '{}': {}", pid_str.trim(), e),
        })
    })?;

    let kill_command = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| {
            WasmrunError::Server(ServerError::StopFailed {
                pid,
                reason: format!("Failed to kill server process: {e}"),
            })
        })?;

    if kill_command.status.success() {
        std::fs::remove_file(PID_FILE).map_err(|e| {
            WasmrunError::Server(ServerError::StopFailed {
                pid,
                reason: format!("Failed to remove PID file: {e}"),
            })
        })?;
        println!("💀 Existing Wasmrun server terminated successfully.");
        Ok(())
    } else {
        let error_msg = String::from_utf8_lossy(&kill_command.stderr);
        Err(WasmrunError::Server(ServerError::StopFailed {
            pid,
            reason: error_msg.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_server_running_no_pid_file() {
        // This test ensures is_server_running doesn't crash when there's no PID file
        let _result = is_server_running();
        // Result depends on whether server is actually running, but shouldn't crash
        // Just verify the function returns without panicking
    }

    #[test]
    fn test_stop_existing_server_no_server() {
        let result = stop_existing_server();
        // Should return error when no server is running
        assert!(result.is_err());

        if let Err(WasmrunError::Server(ServerError::NotRunning)) = result {
            // Expected error when no server is running
        } else {
            // Other errors are also acceptable (e.g., permission issues)
        }
    }
}
