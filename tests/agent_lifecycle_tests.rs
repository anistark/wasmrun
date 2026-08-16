//! Integration tests for agent server lifecycle: signal handling and the
//! session tree a server leaves behind (or does not) when it stops.
//!
//! These drive the real binary rather than the library, because the thing under
//! test is process-level: which signals reach the shutdown path at all.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn wasmrun_binary() -> PathBuf {
    // target/debug/deps/<test binary> → target/<profile>/wasmrun
    let mut path = std::env::current_exe().expect("current exe");
    path.pop();
    path.pop();
    path.push("wasmrun");
    path
}

/// A port nothing is listening on, released for the child to bind.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The instance tree a running agent claims: `<temp>/wasmrun-agent-<pid>-<ms>`.
fn instance_tree(pid: u32) -> Option<PathBuf> {
    let prefix = format!("wasmrun-agent-{pid}-");
    std::fs::read_dir(std::env::temp_dir())
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
}

fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(v) = f() {
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    wait_for(|| f().then_some(())).is_some()
}

/// Start an agent on a free port and wait until it has claimed its tree.
fn start_agent() -> (Child, PathBuf) {
    let child = Command::new(wasmrun_binary())
        .args(["agent", "--port", &free_port().to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start wasmrun agent");
    let tree = wait_for(|| instance_tree(child.id()))
        .expect("the agent never claimed a session tree in the temp dir");
    (child, tree)
}

fn signal(child: &Child, sig: &str) {
    let status = Command::new("kill")
        .args([sig, &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(status.success(), "failed to send {sig}");
}

#[test]
fn test_sigterm_shuts_down_and_removes_the_session_tree() {
    // SIGTERM is how a container or a systemd unit is stopped. Before 0.22.9
    // only SIGINT was handled, so this path ended in SIGKILL at the end of the
    // stop timeout with the whole tree leaked.
    let (mut child, tree) = start_agent();
    assert!(tree.exists());

    signal(&child, "-TERM");
    let status = wait_for(|| child.try_wait().ok().flatten())
        .expect("the agent ignored SIGTERM and had to be waited out");

    assert!(status.success(), "unclean exit after SIGTERM: {status:?}");
    assert!(
        !tree.exists(),
        "the session tree survived a clean shutdown: {}",
        tree.display()
    );
}

#[test]
fn test_sigint_shuts_down_and_removes_the_session_tree() {
    let (mut child, tree) = start_agent();

    signal(&child, "-INT");
    let status = wait_for(|| child.try_wait().ok().flatten()).expect("the agent ignored SIGINT");

    assert!(status.success(), "unclean exit after SIGINT: {status:?}");
    assert!(!tree.exists(), "the session tree survived Ctrl+C");
}

#[test]
fn test_sigkill_leaves_the_tree_for_a_later_sweep() {
    // The case the startup sweep exists for: no destructor runs, so the tree
    // stays until another server finds its heartbeat stale. That window is
    // minutes long by design, so this asserts the leak rather than the sweep;
    // `sweep_orphans_in` covers the collection itself.
    let (mut child, tree) = start_agent();

    signal(&child, "-KILL");
    assert!(
        wait_until(|| child.try_wait().ok().flatten().is_some()),
        "the agent survived SIGKILL"
    );
    assert!(
        tree.exists(),
        "nothing to sweep: the tree vanished without a shutdown path"
    );
    assert!(heartbeat_of(&tree).exists(), "no heartbeat to age out");

    std::fs::remove_dir_all(&tree).expect("test cleanup");
}

fn heartbeat_of(tree: &Path) -> PathBuf {
    tree.join(".alive")
}
