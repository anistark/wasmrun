//! [Shared] Other processes, by PID: whether one is a wasmrun, how much memory
//! it holds, and asking it to stop. Server mode's `stop` and the process
//! manager both need these, and std has none of them.

use std::process::Command;

/// Whether `pid` is a running wasmrun, which is what keeps a reused PID from
/// being signalled.
#[cfg(unix)]
pub fn is_wasmrun(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("wasmrun"))
}

#[cfg(windows)]
pub fn is_wasmrun(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("wasmrun"))
}

/// Resident memory of `pid` in bytes, when the platform says.
#[cfg(unix)]
pub fn rss_bytes(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "rss="])
        .output()
        .ok()?;
    let kib: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kib * 1024)
}

#[cfg(windows)]
pub fn rss_bytes(_pid: u32) -> Option<u64> {
    None
}

/// Ask `pid` to stop: SIGTERM where there are signals, so it can clean up,
/// and taskkill on Windows. The error is the tool's own message.
#[cfg(unix)]
pub fn terminate(pid: u32) -> Result<(), String> {
    run(Command::new("kill").arg(pid.to_string()))
}

#[cfg(windows)]
pub fn terminate(pid: u32) -> Result<(), String> {
    run(Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]))
}

fn run(command: &mut Command) -> Result<(), String> {
    let output = command.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn this_process_has_memory_and_a_child_can_be_stopped() {
        assert!(rss_bytes(std::process::id()).unwrap() > 0);
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        terminate(child.id()).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
    }
}
