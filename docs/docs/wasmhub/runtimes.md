---
sidebar_position: 5
title: Runtimes
---

# Available Runtimes

WasmHub provides versioned WASM runtime binaries built from source inside Docker for reproducibility. All runtimes target **WASI Preview 1** (`wasip1`).

## Go

| Property | Value |
|----------|-------|
| **Version** | 1.23 |
| **Compiler** | TinyGo 0.34.0 |
| **Target** | `wasip1` |
| **Size** | ~261 KB |
| **Source** | [go.dev](https://go.dev/) |
| **License** | BSD-3-Clause |

### Capabilities

- ✅ Filesystem read/write (`os.ReadFile`, `os.WriteFile`)
- ✅ Environment variables (`os.Getenv`, `os.Environ`)
- ✅ Command-line arguments (`os.Args`)
- ✅ Standard I/O (stdin, stdout, stderr)
- ✅ String processing, math, JSON
- ⚠️ No goroutines (TinyGo WASI limitation)
- ⚠️ No networking (WASI limitation)
- ⚠️ Reduced stdlib compared to full Go

### Built-in Commands

The Go runtime binary implements a set of standard commands for testing:

```sh
# Run with wasmrun exec or any WASI-compatible runtime
wasmrun exec go-1.23.wasm version    # Show runtime version info
wasmrun exec go-1.23.wasm echo hello # Echo arguments
wasmrun exec go-1.23.wasm env        # List environment variables
wasmrun exec go-1.23.wasm ls /       # List directory contents
wasmrun exec go-1.23.wasm cat file   # Print file contents
wasmrun exec go-1.23.wasm write f hi # Write to a file
```

---

## Rust

| Property | Value |
|----------|-------|
| **Version** | 1.82 |
| **Compiler** | `rustc` with `wasm32-wasip1` target |
| **Target** | `wasm32-wasip1` |
| **Size** | ~76 KB |
| **Source** | [rust-lang.org](https://www.rust-lang.org/) |
| **License** | MIT/Apache-2.0 |

### Capabilities

- ✅ Filesystem read/write (`std::fs`)
- ✅ Environment variables (`std::env`)
- ✅ Command-line arguments (`std::env::args`)
- ✅ Standard I/O (`std::io`)
- ✅ Full `std` library (collections, string, formatting, etc.)
- ⚠️ No networking (WASI limitation)
- ⚠️ No threads (WASI limitation)

### Built-in Commands

```sh
wasmrun exec rust-1.82.wasm version
wasmrun exec rust-1.82.wasm echo hello world
wasmrun exec rust-1.82.wasm env
wasmrun exec rust-1.82.wasm ls /
wasmrun exec rust-1.82.wasm cat file.txt
wasmrun exec rust-1.82.wasm write file.txt content
```

---

## Coming Soon

### Node.js

JavaScript runtime compiled from source with WASI support. This is the most complex build target — requires patching libuv and Node.js internals for WASI compatibility.

### Python

Python runtime via Pyodide or CPython WASI build. Includes the standard library and support for common packages.

### Ruby

Ruby runtime via [ruby.wasm](https://github.com/ruby/ruby.wasm). CRuby compiled to WASM with standard library support.

### PHP

PHP runtime via php-wasm. PHP interpreter compiled to WASM with common extensions (json, mbstring, etc.).

---

## Manifest Format

Each runtime has a per-language manifest (`runtimes/<lang>/manifest.json`):

```json
{
    "language": "go",
    "latest": "1.23",
    "versions": {
        "1.23": {
            "file": "go-1.23.wasm",
            "size": 266712,
            "sha256": "efa1e13f39dfd3783d0eff5669088ab99a1ea1d38ac79f29b02e2ad8ddfea29d",
            "released": "2026-02-03T13:23:13Z",
            "wasi": "wasip1",
            "features": []
        }
    }
}
```

The global manifest (`manifest.json`) aggregates all runtimes:

```json
{
    "version": "0.1.4",
    "languages": {
        "go": {
            "latest": "1.23",
            "versions": ["1.23"],
            "source": "https://go.dev/",
            "license": "BSD-3-Clause"
        },
        "rust": {
            "latest": "1.82",
            "versions": ["1.82"],
            "source": "https://www.rust-lang.org/",
            "license": "MIT/Apache-2.0"
        }
    }
}
```

## Build Pipeline

All runtimes are built inside Docker for reproducibility:

```
Source code → Docker (TinyGo/rustc) → wasm-opt optimization → SHA256 verification → manifest generation → GitHub Release
```

Build flags:
- **Go:** `tinygo build -target=wasip1`
- **Rust:** `cargo build --target wasm32-wasip1 --release`
- **Optimization:** `wasm-opt -O3 --enable-bulk-memory`

Compressed variants (`.wasm.gz`, `.wasm.br`) are also published for CDN usage.
