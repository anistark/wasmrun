---
sidebar_position: 1
title: Overview
---

# Server Mode

Wasmrun's server mode (`wasmrun run`) compiles and serves WebAssembly projects with a built-in development server, live reload, and browser-based execution.

## What It Does

Server mode is a development tool that:

1. **Detects** your project language (Rust, Go, C/C++, Python, AssemblyScript)
2. **Compiles** source code to WebAssembly using the appropriate plugin
3. **Serves** the compiled `.wasm` file via a local HTTP server
4. **Opens** a browser UI showing your module's exports, memory layout, and execution
5. **Watches** for file changes and auto-recompiles (with `--watch`)

```sh
wasmrun run ./my-rust-project --watch
```

## When to Use

- Developing WebAssembly modules that target the browser
- Testing wasm-bindgen projects with JavaScript glue
- Iterating on WASM libraries with instant feedback
- Inspecting module structure (exports, memory, sections) in a visual UI

## Quick Example

```sh
# Compile and serve a Rust WASM project
wasmrun run ./examples/rust-hello

# Serve a pre-built WASM file
wasmrun run ./output.wasm

# With live reload
wasmrun run ./my-project --watch --port 3000
```

The server starts at `http://localhost:8420` by default, serving an HTML page that loads and runs your WASM module.
