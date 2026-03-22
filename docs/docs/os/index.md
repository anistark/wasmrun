---
sidebar_position: 1
title: Overview
---

# OS Mode

Wasmrun's OS mode (`wasmrun os`) provides a browser-based execution environment that runs projects inside a WASM virtual machine with a full development UI.

## What It Does

OS mode is a micro-kernel environment that:

1. **Detects** your project language (Node.js, Python, Rust, Go, C)
2. **Fetches** the appropriate language runtime from [wasmhub](https://github.com/anistark/wasmhub) (QuickJS for JS, RustPython for Python)
3. **Serves** a browser UI with panels for console, filesystem, kernel status, and logs
4. **Boots** a WASM VM in the browser using `WebAssembly.instantiate()`
5. **Runs** your project code inside the sandboxed VM with WASI syscalls

```sh
wasmrun os ./my-project
```

## When to Use

- Running Node.js or Python projects in a sandboxed WASM environment
- Browser-based development with real-time logs and file browsing
- Experimenting with multi-language runtime execution
- Projects that need network isolation and port forwarding

## Quick Example

```sh
# Run a Node.js project
wasmrun os ./my-express-app

# Run with explicit language
wasmrun os ./my-app --language python

# Custom port with file watching
wasmrun os ./my-project --port 3000 --watch
```

The UI opens at `http://localhost:8420` with panels for application output, console, filesystem browser, kernel status, and structured logs.

## Architecture

```
┌─ wasmrun server (Rust, on host) ──────────────────────┐
│  • Detects language, serves browser UI                 │
│  • Serves project files via /api/project/files         │
│  • Fetches runtime .wasm from wasmhub (cached)         │
│  • REST API for kernel stats, filesystem, logs         │
└────────────────────────────────────────────────────────┘
         │ HTTP
         ▼
┌─ Browser ──────────────────────────────────────────────┐
│  ┌─ UI (Preact) ────────────────────────────────────┐  │
│  │  Console, Filesystem, Kernel Status, Logs panels │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌─ WASM VM ────────────────────────────────────────┐  │
│  │  Language Runtime .wasm (QuickJS, RustPython)     │  │
│  │  WASI Shim (JS) → virtual FS, stdout, args       │  │
│  │  User code runs fully sandboxed                   │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

## Security Model

User code never touches the host system:

- **WASM VM** is memory-isolated by the browser
- **Filesystem** is virtual (in-memory, populated from server)
- **Network** calls go through wasmnet with policy enforcement
- **No access** to `window`, `document`, `fetch`, DOM, or host APIs
