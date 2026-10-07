---
sidebar_position: 2
title: run
---

# wasmrun

Compile and serve a WebAssembly project with a built-in development server.

## Synopsis

```sh
wasmrun [PROJECT] [OPTIONS]
```

Running is the default mode, so no subcommand is needed. `wasmrun run`, `wasmrun dev`, and `wasmrun serve` are accepted as explicit aliases.

## Description

The `run` command is wasmrun's primary development workflow. It detects your project type, compiles source code to WebAssembly using the appropriate plugin, and starts a development server.

What the server shows depends on what the build produced:

- **A module** is loaded into the console UI, which lists its exports and lets you call them.
- **A web app** (the project has an `index.html`, or the build emitted JS glue) runs from its own page on a second port, and the UI port becomes a control center around it. See [Web Apps](../web-apps.md).

When given a `.wasm` file directly, it skips compilation and serves immediately.

## Options

### `-p, --path <PATH>`

Path to a project directory or WASM file.

```sh
wasmrun --path ./my-project
wasmrun -p ./output.wasm
```

Default: current directory (`.`)

You can also use a positional argument:

```sh
wasmrun ./my-project
```

### `-P, --port <PORT>`

Port for the development server.

```sh
wasmrun --port 3000
wasmrun -P 8080
```

- Default: `8420`
- Range: `1-65535`

If the port is already in use, wasmrun picks the next free port within ten above it, stepping over 8430 so an agent server (`wasmrun agent`) can still start on its default.

### `--app-port <PORT>`

Port for a web app's own page. Only used when the project is a web app.

```sh
wasmrun ./examples/web-leptos --app-port 9100
```

- Default: the first free port in `8500-8599`, whatever the UI port is, skipping the UI port if `--port` points into that range
- An explicit `--app-port` must differ from `--port`, and if something is already listening on it, wasmrun fails to start rather than picking another

### `-l, --language <LANGUAGE>`

Force a specific language instead of auto-detection. Useful when a project could match multiple plugins.

```sh
wasmrun --language rust
wasmrun -l go
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

Enable file watching and auto-recompilation. When source files change, wasmrun recompiles and the page reloads once the new build is ready. A build that fails leaves the last good build running.

```sh
wasmrun --watch
```

See [Live Reload](../live-reload.md) for details on watched file types and behavior.

### `-v, --verbose`

Show detailed compilation output including compiler commands, timings, and file paths.

```sh
wasmrun --verbose
```

### `-s, --serve`

Open the browser automatically when the server starts.

```sh
wasmrun --serve
```

## How It Works

1. **Path resolution**: resolves the input path (positional or `-p` flag)
2. **Type detection**: if it's a `.wasm` file, skip to step 5. If it's a directory, continue.
3. **Plugin matching**: checks installed plugins for one that handles this project type. Falls back to built-in language detection.
4. **Compilation**: the matched plugin compiles source to `.wasm` (and optional `.js` glue for wasm-bindgen projects) in a build directory of its own under the system temp directory
5. **Server startup**: starts the UI server on the configured port, and for a web app the app server beside it. Both bind `127.0.0.1` only
6. **Browser UI**: for a module, a page that loads it and displays its exports, memory layout, sections, and plugin info; for a web app, the control center with the app framed in it

## Examples

### Serve a WASM File

```sh
# Serve a pre-built WASM file
wasmrun ./hello.wasm

# On a custom port
wasmrun ./hello.wasm --port 3000

# Open browser automatically
wasmrun ./hello.wasm --serve
```

### Compile and Serve a Rust Project

```sh
# Auto-detects Rust from Cargo.toml
wasmrun ./my-rust-project

# With live reload during development
wasmrun ./my-rust-project --watch --serve

# Force language if needed
wasmrun ./my-rust-project --language rust
```

### Compile and Serve a Go Project

```sh
# Auto-detects Go from go.mod
wasmrun ./my-go-project

# With verbose output to see TinyGo commands
wasmrun ./my-go-project --verbose
```

### Web Apps and wasm-bindgen Projects

A project with an `index.html`, or whose build emits JS glue, runs as a web app:

```sh
# UI on 8420, the app itself on 8500
wasmrun ./examples/web-leptos

# Pre-built wasm-bindgen output: the _bg.wasm with its .js beside it
wasmrun ./pkg/my_lib_bg.wasm
```

See [Web Apps](../web-apps.md) for how files are resolved and what the control center shows.

### Development Workflow

```sh
# Start with live reload
wasmrun ./my-project --watch --serve --port 3000

# In another terminal, make changes to source files
# The page reloads once the rebuild succeeds
```

### CI/CD Usage

```sh
# Compile and verify, don't start server
wasmrun compile ./my-project --optimization release
wasmrun verify ./dist/output.wasm --detailed
```

## Browser UI

The served page provides:

- **Module info**: function count, exports, imports, memory limits, section sizes
- **Export list**: all exported functions with their signatures
- **Plugin info**: which plugin compiled the module, its version, and capabilities
- **Version info**: wasmrun version

This data is also available via JSON endpoints:
- `GET /api/module-info`: module analysis
- `GET /api/version`: wasmrun version
- `GET /api/dev`: the current build (status, generation, error, `.wasm` size), both URLs, and request metrics
- `GET /api/logs?since=N`: build, file-change and request log entries after sequence `N`

A web app gets the control center instead; see [Web Apps](../web-apps.md#the-control-center).

## Port Conflicts

If port 8420 (or your specified port) is already in use:

```sh
# wasmrun detects the conflict and moves up
wasmrun --port 8420
# ⚠️  Port 8420 is already in use
# 🔄 Using port 8421 for the UI
```

## See Also

- [compile](./compile.md): compile without serving
- [Web Apps](../web-apps.md): the app port and the control center
- [Live Reload](../live-reload.md): details on `--watch` behavior
- [Plugins](/docs/plugins): install language plugins
