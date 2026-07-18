---
sidebar_position: 4
title: wasmgo
---

# wasmgo

Go WebAssembly plugin for wasmrun.

## About

`wasmgo` compiles Go projects to WebAssembly for wasmrun's [Server Mode](/docs/server) using [TinyGo](https://tinygo.org/) as the backend compiler, producing small WASM binaries suited to the browser.

[![Crates.io Version](https://img.shields.io/crates/v/wasmgo)](https://crates.io/crates/wasmgo)

- **GitHub**: [anistark/wasmgo](https://github.com/anistark/wasmgo)
- **crates.io**: [crates.io/crates/wasmgo](https://crates.io/crates/wasmgo)
- **docs.rs**: [docs.rs/wasmgo](https://docs.rs/wasmgo)
- **lib.rs**: [lib.rs/crates/wasmgo](https://lib.rs/crates/wasmgo)

## Install

```sh
wasmrun plugin install wasmgo
```

**Requirements:**

- Go toolchain
- [TinyGo](https://tinygo.org/getting-started/install/) compiler

## Usage

Once installed, wasmrun auto-detects Go projects from `go.mod`:

```sh
# Compile and serve with the dev server
wasmrun ./my-go-project --watch

# Compile only
wasmrun compile ./my-go-project

# Plugin management
wasmrun plugin info wasmgo
wasmrun plugin update wasmgo
```

## What It Covers

- **Go to WASM compilation** through TinyGo
- **Flexible entry points**: `go.mod`, `main.go`, `cmd/main.go`, and `app.go` layouts
- **Optimization options**, including size-optimized builds
- **Live reload**: watch mode recompilation through wasmrun's dev server
- **Compatibility checking** and dependency validation before builds
- **Custom build targets**

## What It Doesn't Cover

- **Web application packaging**: unlike `wasmrust`, there is no web app build pipeline
- **Standard `go build` output**: compilation goes through TinyGo, so [TinyGo's language coverage](https://tinygo.org/docs/reference/lang-support/) applies; packages relying on unsupported reflection or cgo won't compile
- **Running WASM**: execution is handled by wasmrun, not the plugin

## See Also

- [Go language guide](../server/languages/go.md): project setup and workflows
- [Plugin usage](./usage.md): install, update, and manage plugins
