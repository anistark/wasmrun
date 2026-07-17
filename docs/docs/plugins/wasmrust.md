---
sidebar_position: 3
title: wasmrust
---

# wasmrust

Rust WebAssembly plugin for wasmrun.

## About

`wasmrust` compiles Rust projects to WebAssembly for wasmrun's [Server Mode](/docs/server), with automatic project detection, wasm-bindgen support, and full web application builds.

[![Crates.io Version](https://img.shields.io/crates/v/wasmrust)](https://crates.io/crates/wasmrust)

- **GitHub**: [anistark/wasmrust](https://github.com/anistark/wasmrust)
- **crates.io**: [crates.io/crates/wasmrust](https://crates.io/crates/wasmrust)
- **docs.rs**: [docs.rs/wasmrust](https://docs.rs/wasmrust)
- **lib.rs**: [lib.rs/crates/wasmrust](https://lib.rs/crates/wasmrust)

## Install

```sh
wasmrun plugin install wasmrust
```

**Requirements:**

- Rust toolchain (rustup, cargo, rustc)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- Optional, depending on project type: `wasm-pack`, `trunk`, `wasm-opt`

## Usage

Once installed, wasmrun auto-detects Rust projects from `Cargo.toml`:

```sh
# Compile and serve with the dev server
wasmrun ./my-rust-project --watch

# Compile only
wasmrun compile ./my-rust-project

# Plugin management
wasmrun plugin info wasmrust
wasmrun plugin update wasmrust
```

The plugin picks a build strategy automatically: plain `cargo` for standard WASM, `wasm-pack` for wasm-bindgen projects, and `trunk` for full web applications.

## What It Covers

- **Standard WASM**: plain Rust to `.wasm` compilation
- **wasm-bindgen projects**: JavaScript interop with dual `.wasm` + `.js` output
- **Web applications**: full builds via trunk or wasm-pack
- **Framework detection**: Yew, Leptos, Dioxus, and Sycamore projects
- **Optimization levels**: debug, release, and size-optimized builds
- **Live reload**: watch mode recompilation through wasmrun's dev server
- **Project inspection**: dependency analysis and compatibility checking

## What It Doesn't Cover

- **Running WASM**: the plugin only compiles; execution is handled by wasmrun ([Server](/docs/server), [Exec](/docs/exec), or [OS](/docs/os) mode)
- **Toolchain installation**: Rust, wasm-pack, and trunk must be installed separately
- **Exec and OS modes**: those modes run pre-built WASM and [wasmhub runtimes](https://anistark.github.io/wasmhub/); they never invoke compilation plugins

## Configuration

Optional settings live in `wasmrun.toml` at the project root or the global `~/.wasmrun/config.toml`. Environment variables such as `WASMRUST_VERBOSE` and `WASMRUST_BUILD_STRATEGY` override the build behavior per invocation.

## See Also

- [Rust language guide](../server/languages/rust.md): project setup and workflows
- [Plugin usage](./usage.md): install, update, and manage plugins
