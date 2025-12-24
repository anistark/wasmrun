---
sidebar_position: 1
---

# Introduction

Welcome to **Wasmrun** - a lightweight WebAssembly development server and runtime.

## What is Wasmrun?

Wasmrun is a development server and runtime for WebAssembly projects that supports multiple programming languages through a plugin system.

## Key Features

- **Multi-language support** - Rust, Go, Python, C/C++, AssemblyScript
- **Plugin system** - Extensible architecture for language support
- **Live reload** - Automatic recompilation on file changes
- **Native execution** - Run WASM modules natively via `wasmrun exec`
- **WASI support** - WebAssembly System Interface compatibility
- **Network isolation** - Per-process network namespaces
- **Port forwarding** - Easy port mapping for web applications
- **OS Mode** - Browser-based multi-language execution environment

## Getting Started

Check out the [Installation](./installation.md) guide to get started with Wasmrun.

## Quick Example

```bash
# Install wasmrun
cargo install wasmrun

# Run a Rust WASM project
wasmrun run ./my-rust-project

# Execute a WASM module natively
wasmrun exec ./module.wasm
```
