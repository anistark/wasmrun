---
sidebar_position: 4
title: WASI Support
---

# WASI in Exec Mode

Wasmrun's exec mode provides WASI Preview 1 support, enabling WASM modules to interact with the host system through a controlled syscall interface.

## Supported Syscalls

| Syscall | Description | Status |
|---|---|---|
| `fd_write` | Write to file descriptors (stdout, stderr) | ✅ |
| `fd_read` | Read from file descriptors (stdin) | ✅ |
| `fd_close` | Close a file descriptor | ✅ |
| `fd_seek` | Seek within a file | ✅ |
| `args_get` | Retrieve command-line arguments | ✅ |
| `args_sizes_get` | Get argument count and buffer sizes | ✅ |
| `environ_get` | Retrieve environment variables | ✅ |
| `environ_sizes_get` | Get environment variable count and sizes | ✅ |
| `clock_time_get` | Get current time (realtime, monotonic) | ✅ |
| `random_get` | Fill buffer with random bytes | ✅ |
| `proc_exit` | Exit with a status code | ✅ |
| `fd_prestat_get` | Preopened directory info | 🔧 Planned |
| `fd_prestat_dir_name` | Preopened directory name | 🔧 Planned |
| `path_open` | Open a file by path | 🔧 Planned |
| `fd_fdstat_get` | File descriptor status | 🔧 Planned |

## How It Works

WASI syscalls are registered as host functions in the linker. When the WASM module calls an imported function from the `wasi_snapshot_preview1` namespace, the executor dispatches to the corresponding Rust implementation.

```
WASM module calls fd_write(fd=1, iovs, iovs_len, nwritten)
    → executor detects imported function
    → dispatches to host function via linker
    → host reads iovec pointers from linear memory
    → writes bytes to stdout
    → writes bytes_written back to linear memory
```

## Environment Setup

The `WasiEnv` struct configures the WASI environment:

```rust
WasiEnv::new()
    .with_args(vec!["program".into(), "arg1".into()])
    .with_env("KEY".into(), "value".into())
```

- **Arguments** — available via `args_get` / `args_sizes_get`
- **Environment variables** — available via `environ_get` / `environ_sizes_get`
- **Output capture** — stdout/stderr buffered in `WasiEnv` for programmatic access

## Clock Support

Two clock types are supported:

| Clock ID | Constant | Description |
|---|---|---|
| `REALTIME` | 0 | Wall clock time (seconds since Unix epoch) |
| `MONOTONIC` | 1 | Monotonically increasing (for measuring intervals) |

## Filesystem

WASI filesystem integration is planned for v0.17.0. It will bridge the exec mode executor to wasmrun's existing `WasiFilesystem`, which provides:

- Mount host directories into the WASM sandbox
- Path traversal protection
- Read-only mode support
- File size limits

See the [roadmap](/docs/exec) for details on upcoming WASI filesystem support.
