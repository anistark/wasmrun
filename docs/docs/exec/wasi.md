---
sidebar_position: 4
title: WASI Support
---

# WASI in Exec Mode

Wasmrun's exec mode provides WASI Preview 1 support, enabling WASM modules to interact with the host system through a controlled syscall interface.

## Supported Syscalls

| Syscall | Description | Status |
|---|---|---|
| `fd_write` | Write to file descriptors (stdout, stderr) — reads iovecs from memory | ✅ |
| `fd_read` | Read from file descriptors (stdin returns EOF) | ✅ |
| `fd_close` | Close a file descriptor | ✅ |
| `fd_seek` | Seek within a file descriptor | ✅ |
| `fd_fdstat_get` | File descriptor status (filetype, flags, rights) | ✅ |
| `fd_prestat_get` | Preopened directory info (returns EBADF — no preopens yet) | ✅ Stub |
| `fd_prestat_dir_name` | Preopened directory name | ✅ Stub |
| `args_get` | Retrieve command-line arguments from memory | ✅ |
| `args_sizes_get` | Get argument count and buffer sizes | ✅ |
| `environ_get` | Retrieve environment variables from memory | ✅ |
| `environ_sizes_get` | Get environment variable count and sizes | ✅ |
| `clock_time_get` | Get current time (realtime, monotonic) in nanoseconds | ✅ |
| `random_get` | Fill buffer with random bytes | ✅ |
| `proc_exit` | Exit with a status code (terminates execution cleanly) | ✅ |
| `poll_oneoff` | Poll for events (stub — returns ENOSYS) | ✅ Stub |
| `sched_yield` | Yield execution (stub — returns success) | ✅ Stub |
| `path_open` | Open a file by path | 🔧 Planned |
| `path_filestat_get` | Stat a path | 🔧 Planned |
| `path_create_directory` | Create directory | 🔧 Planned |
| `fd_readdir` | Read directory entries | 🔧 Planned |

## How It Works

WASI syscalls are registered as host functions in the linker under the `wasi_snapshot_preview1` module namespace. When the WASM module calls an imported function, the executor dispatches to the corresponding Rust implementation with access to linear memory.

```
WASM module calls fd_write(fd=1, iovs, iovs_len, nwritten)
    → executor detects imported function (func_idx < import_count)
    → dispatches to host function via linker
    → host reads iovec structs {buf_ptr, buf_len} from linear memory
    → reads string bytes from memory at buf_ptr
    → appends bytes to WasiEnv stdout buffer
    → writes total bytes_written to nwritten pointer in memory
    → returns errno (0 = success)
```

## Environment Setup

The `WasiEnv` struct configures the WASI environment:

```rust
WasiEnv::new()
    .with_args(vec!["program".into(), "arg1".into()])
    .with_env("KEY".into(), "value".into())
```

- **Arguments** — written to linear memory via `args_get` / `args_sizes_get`
- **Environment variables** — written as `KEY=VALUE\0` strings via `environ_get` / `environ_sizes_get`
- **Output capture** — stdout/stderr buffered in `WasiEnv` for programmatic access via `get_stdout()` / `get_stderr()`

## Linker Integration

The executor uses a `Linker` to resolve imported functions. The linker maps `(module, name)` pairs to host function implementations:

```rust
let wasi_env = Arc::new(Mutex::new(WasiEnv::new()));
let linker = create_wasi_linker(wasi_env.clone());
let mut executor = Executor::new_with_linker(module, linker)?;
```

Host functions receive `&mut LinearMemory` so they can read pointers and write results directly into the module's address space.

## Clock Support

| Clock ID | Constant | Description |
|---|---|---|
| `REALTIME` | 0 | Wall clock time (nanoseconds since Unix epoch) |
| `MONOTONIC` | 1 | Monotonically increasing (for measuring intervals) |

## Process Exit

`proc_exit` terminates execution by raising a sentinel error that the executor catches. The exit code is extracted and returned to the caller:

```rust
match executor.execute_with_args(func_idx, args) {
    Ok(_) => 0,
    Err(e) => Executor::is_proc_exit(&e).unwrap_or(-1),
}
```

## Filesystem

WASI filesystem integration is planned for v0.17.4. It will bridge the exec mode executor to wasmrun's existing `WasiFilesystem`, which provides:

- Mount host directories into the WASM sandbox
- Path traversal protection
- Read-only mode support
- File size limits

See the [roadmap](/docs/exec) for details on upcoming WASI filesystem support.
