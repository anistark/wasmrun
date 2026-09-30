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

Plugins are installed separately; see [Plugins](/docs/plugins) for setup.

## Built-in HTTP Server

A lightweight HTTP server (powered by `tiny_http`) serves, on loopback:

- The compiled `.wasm` file with correct `application/wasm` content type
- An HTML page with module inspection UI
- For a web app, a second server on its own port: the project's `index.html`, its static files, and the build output under `/pkg/`

## Web Apps

A project with an `index.html`, or whose build emits JS glue (wasm-bindgen, Emscripten), runs natively in the browser from its own page on the app port. The UI port becomes a control center: the app in a frame, its console, its requests, the build, and the module. A wasm-bindgen library with no page gets a generated one that loads it and lists its exports.

```sh
wasmrun ./examples/web-leptos
```

See [Web Apps](./web-apps.md).

## Live Reload

With `--watch`, wasmrun monitors your source files and recompiles on changes:

1. File system watcher detects modifications to source, config, and asset files
2. Source changes rebuild with the same toolchain; page and asset changes skip the build
3. The page reloads itself once the new build is ready
4. A failed build leaves the last good build running and shows the error in the terminal and the control center

```sh
wasmrun ./my-project --watch
```

### Watched File Types

- **Rebuild:** `*.rs`, `*.go`, `*.py`, `*.c`, `*.cpp`, `*.h`, `*.ts`, `*.js`, `*.toml`, `go.mod`, `package.json`, `Makefile`
- **Reload only:** `*.html`, `*.css`, `*.json`, images, fonts

See [Live Reload](./live-reload.md) for the full list and what is ignored.

## Module Inspection UI

The browser UI provides:

- **Module info**: exports, imports, memory layout, section sizes
- **Plugin info**: which plugin compiled the module, its capabilities
- **Version info**: wasmrun version and build metadata

Available via the `/api/module-info` and `/api/version` endpoints.

## Smart Project Detection

When given a directory, wasmrun:

1. Checks for installed plugins that match the project
2. Falls back to built-in language detection
3. Compiles using the detected toolchain
4. Serves the output

When given a `.wasm` file directly, it skips compilation and serves immediately.
