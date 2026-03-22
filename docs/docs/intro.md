---
sidebar_position: 1
---

# Introduction

**Wasmrun** is a WebAssembly runtime that simplifies development, compilation, and execution of WASM applications across multiple programming languages.

## Three Modes

### [Server Mode](/docs/server) — `wasmrun run`

Compile and serve WebAssembly projects with a built-in dev server, live reload, and browser-based module inspection.

```sh
wasmrun run ./my-rust-project --watch
```

### [Exec Mode](/docs/exec) — `wasmrun exec`

Run WASM files natively using a built-in interpreter with WASI support. No browser, no server.

```sh
wasmrun exec ./program.wasm arg1 arg2
```

### [OS Mode](/docs/os) — `wasmrun os`

Browser-based execution environment with a WASM virtual machine, virtual filesystem, and multi-language runtime support.

```sh
wasmrun os ./my-node-project
```

## Key Features

- **Multi-Language** — Rust, Go, Python, C/C++, AssemblyScript via [plugins](/docs/plugins)
- **Plugin Architecture** — extensible system for language support and build tools
- **Live Reload** — file watching with auto-recompilation
- **Native Execution** — built-in WASM interpreter with WASI syscalls
- **OS Mode** — browser-based sandboxed execution with network isolation
- **Zero Config** — auto-detects project type, sensible defaults

## Getting Started

1. [Install wasmrun](./installation.md)
2. [Quick start guide](./quick-start.md)
3. Pick a mode: [Server](/docs/server), [Exec](/docs/exec), or [OS](/docs/os)
