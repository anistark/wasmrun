---
sidebar_position: 1
title: Overview
---

# Plugin System

Wasmrun uses a plugin architecture to support multiple programming languages. Each language has a dedicated plugin that handles project detection, compilation, and output configuration.

## How It Works

Plugins extend wasmrun's compilation pipeline:

```
wasmrun run ./my-project
    → Plugin system checks installed plugins
    → Matching plugin detects project type (e.g., Cargo.toml → wasmrust)
    → Plugin compiles source → .wasm (+ optional .js glue)
    → Server serves the compiled output
```

## Built-in vs External

**Built-in** plugins ship with wasmrun:
- **C/C++** — Emscripten support

**External** plugins are installed from crates.io:

| Plugin | Language | Install |
|---|---|---|
| `wasmrust` | Rust | `wasmrun plugin install wasmrust` |
| `wasmgo` | Go | `wasmrun plugin install wasmgo` |
| `waspy` | Python | `wasmrun plugin install waspy` |
| `wasmasc` | AssemblyScript | `wasmrun plugin install wasmasc` |

## Quick Start

```sh
# List installed plugins
wasmrun plugin list

# Install a plugin
wasmrun plugin install wasmrust

# Use it (auto-detected from project files)
wasmrun run ./my-rust-project

# Plugin info
wasmrun plugin info wasmrust
```

## Plugin Location

Plugins are installed to `~/.wasmrun/plugins/` and tracked in wasmrun's configuration.

## Plugin Capabilities

Each plugin declares its capabilities:

- **compile_wasm** — can compile to standard WASM
- **compile_webapp** — can compile web applications (wasm-bindgen, etc.)
- **live_reload** — supports watch mode recompilation
- **optimization** — supports debug/release/size optimization levels
- **custom_targets** — supports custom compilation targets
