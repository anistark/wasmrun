---
sidebar_position: 2
title: Features
---

# Server Mode Features

## Multi-Language Compilation

Server mode auto-detects your project type and compiles using the appropriate toolchain:

| Language | Detection | Compiler |
|---|---|---|
| Rust | `Cargo.toml` | `cargo build --target wasm32-unknown-unknown` via wasmrust plugin |
| Go | `go.mod` | TinyGo via wasmgo plugin |
| C/C++ | `Makefile` with emcc | Emscripten (built-in) |
| Python | `*.py` files | waspy plugin |
| AssemblyScript | `asconfig.json` | asc via wasmasc plugin |

Plugins are installed separately — see [Plugins](/docs/plugins) for setup.

## Built-in HTTP Server

A lightweight HTTP server (powered by `tiny_http`) serves:

- The compiled `.wasm` file with correct `application/wasm` content type
- JavaScript glue code for wasm-bindgen projects
- An HTML page with module inspection UI
- Static assets from the project directory

## wasm-bindgen Support

Server mode automatically detects wasm-bindgen projects:

- Recognizes `_bg.wasm` files and their JS counterparts
- Serves both the WASM binary and JavaScript glue code
- Handles module imports/exports correctly

```sh
# Automatically detected
wasmrun run ./my-wasm-bindgen-project
```

## Live Reload

With `--watch`, wasmrun monitors your source files and recompiles on changes:

1. File system watcher detects modifications to source, config, and asset files
2. Project is recompiled using the same toolchain
3. Browser refreshes automatically with the new build
4. Build errors are displayed in the terminal without crashing the server

```sh
wasmrun run ./my-project --watch
```

### Watched File Types

- **Source:** `*.rs`, `*.go`, `*.py`, `*.c`, `*.cpp`, `*.h`, `*.ts`
- **Config:** `Cargo.toml`, `go.mod`, `package.json`, `Makefile`
- **Assets:** `*.html`, `*.css`, `*.js`, `*.json`

## Module Inspection UI

The browser UI provides:

- **Module info** — exports, imports, memory layout, section sizes
- **Plugin info** — which plugin compiled the module, its capabilities
- **Version info** — wasmrun version and build metadata

Available via the `/api/module-info` and `/api/version` endpoints.

## Smart Project Detection

When given a directory, wasmrun:

1. Checks for installed plugins that match the project
2. Falls back to built-in language detection
3. Compiles using the detected toolchain
4. Serves the output

When given a `.wasm` file directly, it skips compilation and serves immediately.
