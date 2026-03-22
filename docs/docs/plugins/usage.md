---
sidebar_position: 2
title: Usage
---

# Plugin Management

## Commands

### List Plugins

```sh
# List installed plugins
wasmrun plugin list

# Detailed view
wasmrun plugin list --all
```

### Install

```sh
# From crates.io
wasmrun plugin install wasmrust

# Specific version
wasmrun plugin install wasmrust --version 0.5.0
```

### Uninstall

```sh
wasmrun plugin uninstall wasmrust
```

### Update

```sh
# Update one plugin
wasmrun plugin update wasmrust

# Update all
wasmrun plugin update all
```

### Enable / Disable

```sh
wasmrun plugin enable wasmrust
wasmrun plugin enable wasmrust --disable
```

### Info

```sh
wasmrun plugin info wasmrust
```

## Plugin Requirements

Each external plugin requires its language toolchain to be installed:

| Plugin | Requirements |
|---|---|
| `wasmrust` | Rust toolchain + `wasm32-unknown-unknown` target |
| `wasmgo` | TinyGo |
| `waspy` | Python 3.8+ |
| `wasmasc` | Node.js + npm/yarn/pnpm/bun |

## Creating Plugins

Plugins implement the `WasmBuilder` trait:

```rust
pub trait WasmBuilder {
    fn can_handle(&self, project_path: &str) -> bool;
    fn check_dependencies(&self) -> Vec<String>;
    fn build(&self, config: &BuildConfig) -> Result<BuildResult, CompilationError>;
}
```

See [Creating Plugins](./creating-plugins.md) for a full guide.
