//! `wasmrun ps` end to end, through the real binary: a supervisor in the
//! background, a program that keeps failing, one that keeps running, and the
//! commands that inspect and stop them. The modules are assembled by hand so
//! the test needs no toolchain and no network.

use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const WASMRUN: &str = env!("CARGO_BIN_EXE_wasmrun");

/// A module whose `_start` calls `proc_exit(code)`, or spins forever.
fn module(body: &[u8]) -> Vec<u8> {
    let mut m = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    // types: (i32) -> (), () -> ()
    m.extend([0x01, 0x08, 0x02, 0x60, 0x01, 0x7f, 0x00, 0x60, 0x00, 0x00]);
    // import wasi_snapshot_preview1.proc_exit as func 0
    m.extend([0x02, 0x24, 0x01, 0x16]);
    m.extend(b"wasi_snapshot_preview1");
    m.push(0x09);
    m.extend(b"proc_exit");
    m.extend([0x00, 0x00]);
    // one function of type 1, a page of memory, and the exports
    m.extend([0x03, 0x02, 0x01, 0x01]);
    m.extend([0x05, 0x03, 0x01, 0x00, 0x01]);
    m.extend([0x07, 0x13, 0x02, 0x06]);
    m.extend(b"memory");
    m.extend([0x02, 0x00, 0x06]);
    m.extend(b"_start");
    m.extend([0x00, 0x01]);
    // code
    let entry_len = body.len() as u8;
    m.extend([0x0a, entry_len + 2, 0x01, entry_len]);
    m.extend(body);
    m
}

fn exits_with_3() -> Vec<u8> {
    // no locals; i32.const 3; call 0; end
    module(&[0x00, 0x41, 0x03, 0x10, 0x00, 0x0b])
}

fn spins() -> Vec<u8> {
    // no locals; loop br 0 end; end
    module(&[0x00, 0x03, 0x40, 0x0c, 0x00, 0x0b, 0x0b])
}

fn ps(home: &Path, args: &[&str]) -> Output {
    Command::new(WASMRUN)
        .arg("ps")
        .args(args)
        .env("WASMRUN_PS_HOME", home)
        .output()
        .expect("run wasmrun ps")
}

fn status(home: &Path, name: &str) -> serde_json::Value {
    let out = ps(home, &["status", name, "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("status JSON")
}

fn wait_until(home: &Path, name: &str, what: &str, done: impl Fn(&serde_json::Value) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let s = status(home, name);
        if done(&s) {
            return;
        }
        assert!(Instant::now() < deadline, "{name} never became {what}: {s}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn ps_restarts_a_failing_program_gives_up_and_stops_everything() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::write(dir.path().join("fail.wasm"), exits_with_3()).unwrap();
    std::fs::write(dir.path().join("spin.wasm"), spins()).unwrap();
    let config = dir.path().join("processes.toml");
    std::fs::write(
        &config,
        r#"
[[process]]
name = "failing"
wasm = "fail.wasm"
max_restarts = 2
backoff_ms = 50

[[process]]
name = "spinning"
wasm = "spin.wasm"
"#,
    )
    .unwrap();

    let out = ps(&home, &["start", "--config", config.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Two restarts, three exits with code 3, then it stops trying.
    wait_until(&home, "failing", "errored", |s| s["status"] == "errored");
    let failing = status(&home, "failing");
    assert_eq!(failing["restarts"], 2);
    let history = failing["history"].as_array().unwrap();
    assert_eq!(history.len(), 3);
    assert!(history.iter().all(|e| e["code"] == 3), "{failing}");
    let log = std::fs::read_to_string(home.join("logs/failing.log")).unwrap();
    assert!(log.contains("exited with code 3"), "{log}");
    assert!(log.contains("not restarting again"), "{log}");

    wait_until(&home, "spinning", "running", |s| s["status"] == "running");
    let pid = status(&home, "spinning")["pid"].as_u64().unwrap();

    let out = ps(&home, &["list", "--json"]);
    let list: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 2);

    // Stopping one program leaves the supervisor running.
    let out = ps(&home, &["stop", "spinning"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    wait_until(&home, "spinning", "stopped", |s| s["status"] == "stopped");

    // A restart brings it back on a new process.
    let out = ps(&home, &["restart", "spinning"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    wait_until(&home, "spinning", "running again", |s| {
        s["status"] == "running" && s["pid"].as_u64() != Some(pid)
    });

    // And stopping everything leaves nothing behind.
    let out = ps(&home, &["stop"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!home.join("supervisor.json").exists());
    let out = ps(&home, &["list"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("No supervisor is running"));
}

#[test]
fn ps_refuses_a_bad_processes_file_before_starting_anything() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let config = dir.path().join("processes.toml");
    std::fs::write(
        &config,
        "[[process]]\nname = \"x\"\nwasm = \"missing.wasm\"\n",
    )
    .unwrap();
    let out = ps(&home, &["start", "--config", config.to_str().unwrap()]);
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stderr) + String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("does not exist"), "{text}");
    assert!(!home.join("supervisor.json").exists());
}
