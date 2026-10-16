//! The performance baseline: fixed workloads through the real binary, timed
//! end to end (process start, parse, run, exit), median of several runs.
//!
//! Ignored by default; run it with `just bench`, which builds in release:
//!
//! ```sh
//! cargo test --release --test benchmark -- --ignored --nocapture
//! ```
//!
//! The JavaScript rows need the nodejs runtime: `WASMRUN_BENCH_NODEJS` names a
//! `nodejs-20.wasm`, or the one `~/.wasmrun/runtimes/nodejs.json` records as
//! cached is used, and they are skipped when neither exists. Results also go to `target/benchmark.json`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const WASMRUN: &str = env!("CARGO_BIN_EXE_wasmrun");
const RUNS: usize = 5;

fn wat(text: &str) -> Vec<u8> {
    let buf = wast::parser::ParseBuffer::new(text).expect("lex");
    let mut module: wast::Wat = wast::parser::parse(&buf).expect("parse");
    module.encode().expect("encode")
}

/// Exports `run` doing the work; `_start` calls it and exits with its low bits
/// so the result cannot be optimised away by anything.
fn program(body: &str) -> Vec<u8> {
    wat(&format!(
        r#"(module
          (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
          (memory 256)
          {body}
          (func (export "_start") (call $exit (i32.and (call $run) (i32.const 0)))))"#
    ))
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

fn time_exec(args: &[&str]) -> Duration {
    median(
        (0..RUNS)
            .map(|_| {
                let start = Instant::now();
                let out = Command::new(WASMRUN)
                    .arg("exec")
                    .arg("-q")
                    .args(args)
                    .output()
                    .expect("run wasmrun");
                let took = start.elapsed();
                assert!(
                    out.status.success(),
                    "{args:?} failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                took
            })
            .collect(),
    )
}

fn nodejs_runtime() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("WASMRUN_BENCH_NODEJS") {
        return Some(PathBuf::from(path));
    }
    // The cache keeps files from earlier releases beside the current one, so
    // take the file its metadata names rather than any `nodejs-*.wasm`.
    let dir = dirs_home()?.join(".wasmrun/runtimes");
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("nodejs.json")).ok()?).ok()?;
    let path = dir.join(meta["filename"].as_str()?);
    path.exists().then_some(path)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

struct Row {
    name: &'static str,
    detail: String,
    ms: f64,
}

#[test]
#[ignore = "a benchmark: run with `just bench`"]
fn benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, bytes: Vec<u8>| -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let empty = write(
        "empty.wasm",
        program("(func $run (result i32) (i32.const 0))"),
    );
    let fib = write(
        "fib.wasm",
        program(
            r#"(func $fib (param i32) (result i32)
                 (if (result i32) (i32.lt_u (local.get 0) (i32.const 2))
                   (then (local.get 0))
                   (else (i32.add (call $fib (i32.sub (local.get 0) (i32.const 1)))
                                  (call $fib (i32.sub (local.get 0) (i32.const 2)))))))
               (func $run (result i32) (call $fib (i32.const 25)))"#,
        ),
    );
    let loop_iters: u64 = 10_000_000;
    let sum = write(
        "loop.wasm",
        program(&format!(
            r#"(func $run (result i32) (local $i i32) (local $acc i32)
                 (loop $l
                   (local.set $acc (i32.add (local.get $acc) (i32.xor (local.get $i) (i32.const 7))))
                   (local.set $i (i32.add (local.get $i) (i32.const 1)))
                   (br_if $l (i32.lt_u (local.get $i) (i32.const {loop_iters}))))
                 (local.get $acc))"#
        )),
    );
    let mem = write(
        "memory.wasm",
        program(
            r#"(func $run (result i32) (local $i i32) (local $acc i32)
                 (loop $w
                   (i32.store (local.get $i) (local.get $i))
                   (local.set $i (i32.add (local.get $i) (i32.const 4)))
                   (br_if $w (i32.lt_u (local.get $i) (i32.const 16777216))))
                 (local.set $i (i32.const 0))
                 (loop $r
                   (local.set $acc (i32.add (local.get $acc) (i32.load (local.get $i))))
                   (local.set $i (i32.add (local.get $i) (i32.const 4)))
                   (br_if $r (i32.lt_u (local.get $i) (i32.const 16777216))))
                 (local.get $acc))"#,
        ),
    );

    let path = |p: &Path| p.to_str().unwrap().to_string();
    let mut rows = Vec::new();
    rows.push(Row {
        name: "startup",
        detail: "parse and run an empty module".into(),
        ms: ms(time_exec(&[&path(&empty)])),
    });
    rows.push(Row {
        name: "calls",
        detail: "recursive fib(25), 242,785 calls".into(),
        ms: ms(time_exec(&[&path(&fib)])),
    });
    let loop_ms = ms(time_exec(&[&path(&sum)]));
    rows.push(Row {
        name: "arithmetic",
        // 9 instructions per iteration
        detail: format!(
            "10M-iteration loop, {:.0}M instructions/s",
            (loop_iters * 9) as f64 / (loop_ms / 1000.0) / 1e6
        ),
        ms: loop_ms,
    });
    rows.push(Row {
        name: "memory",
        detail: "write then read 16 MiB as i32".into(),
        ms: ms(time_exec(&[&path(&mem)])),
    });

    if let Some(node) = nodejs_runtime() {
        let node = path(&node);
        println!("JS rows on {node}");
        let js = |src: &str| time_exec(&[&node, "eval", src]);
        rows.push(Row {
            name: "js-startup",
            detail: "nodejs runtime, eval 1+1".into(),
            ms: ms(js("1+1")),
        });
        rows.push(Row {
            name: "js-loop",
            detail: "nodejs, 10k-iteration loop".into(),
            ms: ms(js("let s=0; for (let i=0;i<10000;i++) s+=i; s")),
        });
        rows.push(Row {
            name: "js-json",
            detail: "nodejs, stringify and parse 200 objects".into(),
            ms: ms(js("const a=[]; for (let i=0;i<200;i++) a.push({i, s:'x'+i}); JSON.parse(JSON.stringify(a)).length")),
        });
        rows.push(Row {
            name: "js-sha256",
            detail: "nodejs, native sha256 of 64 KiB".into(),
            ms: ms(js(
                "require('crypto').createHash('sha256').update(Buffer.alloc(65536)).digest('hex')",
            )),
        });
    } else {
        println!("(JS rows skipped: no nodejs runtime; set WASMRUN_BENCH_NODEJS)");
    }

    println!("\n| Workload | What | Median of {RUNS} |\n|---|---|---|");
    for r in &rows {
        println!("| {} | {} | {:.0} ms |", r.name, r.detail, r.ms);
    }
    let json: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| serde_json::json!({"name": r.name, "detail": r.detail, "median_ms": r.ms}))
        .collect();
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/benchmark.json");
    let _ = std::fs::write(
        &out,
        serde_json::to_string_pretty(&serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "runs": RUNS,
            "rows": json,
        }))
        .unwrap(),
    );
    println!("\nwritten to {}", out.display());
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
