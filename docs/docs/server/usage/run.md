---
sidebar_position: 2
title: run
---

# wasmrun run

Compile and serve a WebAssembly project with a built-in development server.

## Synopsis

```sh
wasmrun run [PROJECT] [OPTIONS]
wasmrun [PROJECT] [OPTIONS]          # run is the default command
```

**Aliases:** `dev`, `serve`

## Description

The `run` command is wasmrun's primary development workflow. It detects your project type, compiles source code to WebAssembly using the appropriate plugin, and starts an HTTP server that serves the compiled module in a browser-based inspection UI.

When given a `.wasm` file directly, it skips compilation and serves immediately.

## Options

### `-p, --path <PATH>`

Path to a project directory or WASM file.

```sh
wasmrun run --path ./my-project
wasmrun run -p ./output.wasm
```

Default: current directory (`.`)

You can also use a positional argument:

```sh
wasmrun run ./my-project
```

### `-P, --port <PORT>`

Port for the development server.

```sh
wasmrun run --port 3000
wasmrun run -P 8080
```

- Default: `8420`
- Range: `1–65535`

If the port is already in use, wasmrun will prompt or auto-select an available port.

### `-l, --language <LANGUAGE>`

Force a specific language instead of auto-detection. Useful when a project could match multiple plugins.

```sh
wasmrun run --language rust
wasmrun run -l go
```

Options: `rust`, `go`, `c`, `asc`, `python`

Without this flag, wasmrun auto-detects based on project files:

| File | Detected Language |
|---|---|
| `Cargo.toml` | Rust |
| `go.mod` | Go |
| `Makefile` (with emcc) | C/C++ |
| `asconfig.json` | AssemblyScript |
| `*.py` | Python |

### `--watch`

Enable file watching and auto-recompilation. When source files change, wasmrun recompiles and the browser refreshes automatically.

```sh
wasmrun run --watch
```

See [Live Reload](../live-reload.md) for details on watched file types and behavior.

### `-v, --verbose`

Show detailed compilation output including compiler commands, timings, and file paths.

```sh
wasmrun run --verbose
```

### `-s, --serve`

Open the browser automatically when the server starts.

```sh
wasmrun run --serve
```

## How It Works

1. **Path resolution** — resolves the input path (positional or `-p` flag)
2. **Type detection** — if it's a `.wasm` file, skip to step 5. If it's a directory, continue.
3. **Plugin matching** — checks installed plugins for one that handles this project type. Falls back to built-in language detection.
4. **Compilation** — the matched plugin compiles source to `.wasm` (and optional `.js` glue for wasm-bindgen projects)
5. **Server startup** — starts an HTTP server on the configured port
6. **Browser UI** — serves an HTML page that loads the WASM module and displays its exports, memory layout, sections, and plugin info

## Examples

### Serve a WASM File

```sh
# Serve a pre-built WASM file
wasmrun run ./hello.wasm

# On a custom port
wasmrun run ./hello.wasm --port 3000

# Open browser automatically
wasmrun run ./hello.wasm --serve
```

### Compile and Serve a Rust Project

```sh
# Auto-detects Rust from Cargo.toml
wasmrun run ./my-rust-project

# With live reload during development
wasmrun run ./my-rust-project --watch --serve

# Force language if needed
wasmrun run ./my-rust-project --language rust
```

### Compile and Serve a Go Project

```sh
# Auto-detects Go from go.mod
wasmrun run ./my-go-project

# With verbose output to see TinyGo commands
wasmrun run ./my-go-project --verbose
```

### wasm-bindgen Projects

wasmrun automatically detects wasm-bindgen output:

```sh
# Detects _bg.wasm + .js glue files
wasmrun run ./pkg/my_lib_bg.wasm

# Or point to the JS file
wasmrun run ./pkg/my_lib.js
```

Both the WASM binary and JavaScript glue code are served together.

### Development Workflow

```sh
# Start with live reload
wasmrun run ./my-project --watch --serve --port 3000

# In another terminal, make changes to source files
# Browser refreshes automatically after recompilation
```

### CI/CD Usage

```sh
# Compile and verify, don't start server
wasmrun compile ./my-project --optimization release
wasmrun verify ./dist/output.wasm --detailed
```

## Browser UI

The served page provides:

- **Module info** — function count, exports, imports, memory limits, section sizes
- **Export list** — all exported functions with their signatures
- **Plugin info** — which plugin compiled the module, its version, and capabilities
- **Version info** — wasmrun version

This data is also available via JSON endpoints:
- `GET /api/module-info` — module analysis
- `GET /api/version` — wasmrun version

## Port Conflicts

If port 8420 (or your specified port) is already in use:

```sh
# wasmrun detects the conflict and offers alternatives
wasmrun run --port 8420
# ⚠️ Port 8420 already in use
# Using port 8421 instead
```

## See Also

- [compile](./compile.md) — compile without serving
- [Live Reload](../live-reload.md) — details on `--watch` behavior
- [Plugins](/docs/plugins) — install language plugins
