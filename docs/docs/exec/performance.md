---
sidebar_position: 6
title: Performance
---

# Performance

wasmrun's interpreter decodes and runs bytecode directly: no JIT, no ahead-of-time compilation. That keeps it small and portable, and makes it slower than wasmtime or a browser engine by a wide margin. These numbers are the baseline 1.0 ships with, so a change that makes things slower shows up against them.

Exec mode, agent mode and `wasmrun ps` all run on this interpreter, so the same numbers apply to each.

## Baseline

Wall-clock time through the release binary, process start to exit, median of five runs. Apple M1, wasmrun 1.0.0 with wasmhub v0.6.0's `20-net` nodejs runtime, the one wasmrun fetches.

| Workload | What it does | Time |
|---|---|---|
| startup | Parse and run an empty module | 15 ms |
| calls | Recursive `fib(25)`: 242,785 calls | 52 ms |
| arithmetic | A 10M-iteration loop, about 62M instructions/s | 1.45 s |
| memory | Write then read 16 MiB as `i32` | 1.19 s |
| js-startup | The nodejs runtime evaluating `1+1` | 345 ms |
| js-loop | A 10,000-iteration JavaScript loop | 1.05 s |
| js-json | Build, stringify and parse 200 objects | 489 ms |
| js-sha256 | `createHash('sha256')` over 64 KiB (native digest) | 485 ms |

A request to the `nodejs-http-api` example under `wasmrun exec --tcplisten` takes about 160 ms.

## What to expect

- **A compiled program** (Rust, C, Go to `wasm32-wasip1`) runs at tens of millions of instructions per second. Fine for tools, tests and request handlers; slow for number crunching
- **JavaScript** runs inside a WASM build of QuickJS, so it is interpreted twice. Starting the runtime costs about a third of a second, and every line of JavaScript costs far more than it would on Node. Work that a native binding does (hashing, JSON) is much cheaper than the same work in JavaScript
- **Memory** costs time to touch, not to reserve: a 256 MiB limit is free until the program writes to it

## Run it yourself

```sh
just bench
```

That builds in release and runs `tests/benchmark.rs`, printing the table and writing `target/benchmark.json`. The JavaScript rows need a nodejs runtime: set `WASMRUN_BENCH_NODEJS` to a `nodejs-20.wasm`, or run any JavaScript through wasmrun once so one is cached in `~/.wasmrun/runtimes`, where the benchmark uses the build wasmrun itself would. Without one they are skipped.

Compare against the table on the same machine: the absolute numbers move with the hardware, the ratios between rows should not.
