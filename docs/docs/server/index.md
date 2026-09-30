---
sidebar_position: 1
title: Overview
---

# Server Mode

Wasmrun's server mode (`wasmrun`) compiles and serves WebAssembly projects with a built-in development server, live reload, and browser-based execution.

## What It Does

Server mode is a development tool that:

1. **Detects** your project language (Rust, Go, C/C++, Python, AssemblyScript)
2. **Compiles** source code to WebAssembly using the appropriate plugin
3. **Serves** the result on loopback: a module in a browser UI showing its exports, memory layout, and execution; a [web app](./web-apps.md) from its own page, with a control center beside it
4. **Watches** for file changes, rebuilds, and reloads the page (with `--watch`)

```sh
wasmrun ./my-rust-project --watch
```

## When to Use

- Developing WebAssembly modules that target the browser
- Running a WebAssembly web app (Leptos, Yew, AssemblyScript) with its console and requests in view
- Iterating on WASM libraries with instant feedback
- Inspecting module structure (exports, memory, sections) in a visual UI

## Quick Example

```sh
# Compile and serve a Rust WASM project
wasmrun ./examples/rust-hello

# Serve a pre-built WASM file
wasmrun ./output.wasm

# With live reload
wasmrun ./my-project --watch --port 3000
```

The server starts at `http://127.0.0.1:8420` by default. A web app gets a second port, `8421` by default, for its own page.
