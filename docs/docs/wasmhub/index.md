---
sidebar_position: 1
title: Overview
---

# WasmHub

**WasmHub** is a centralized registry and package manager for WebAssembly language runtimes. It provides versioned WASM runtime binaries that can be downloaded, cached, and verified — usable as a Rust library, CLI tool, or via CDN.

- **Repository:** [github.com/anistark/wasmhub](https://github.com/anistark/wasmhub)
- **Crate:** [crates.io/crates/wasmhub](https://crates.io/crates/wasmhub)
- **API Docs:** [docs.rs/wasmhub](https://docs.rs/wasmhub)

## What It Does

WasmHub solves the problem of scattered WASM language runtimes by providing a single source for:

1. **Downloading** versioned WASM runtime binaries (Go, Rust, and more coming)
2. **Caching** them locally so they're downloaded once and reused
3. **Verifying** integrity via SHA256 checksums
4. **Distributing** via GitHub Releases and CDN

## How It Fits with Wasmrun

Wasmrun's [OS Mode](/docs/os) uses WasmHub to fetch language runtimes. When you run `wasmrun os ./my-project`, it:

1. Detects the project language
2. Calls WasmHub to download the appropriate WASM runtime
3. Boots the runtime in a browser-based WASM VM

You can also use WasmHub independently as a library or CLI tool.

## Quick Example

### CLI

```sh
# Install
cargo install wasmhub --features cli

# Download a runtime
wasmhub get go 1.23

# List available runtimes
wasmhub list

# Show runtime details
wasmhub info go 1.23
```

### Rust Library

```rust
use wasmhub::{RuntimeLoader, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loader = RuntimeLoader::new()?;

    // Download Go 1.23 (auto-cached)
    let runtime = loader.get_runtime(Language::Go, "1.23").await?;
    println!("Runtime at: {}", runtime.path.display());

    // List all available runtimes
    let manifest = loader.list_available().await?;
    for (lang, info) in &manifest.languages {
        println!("{}: latest = {}", lang, info.latest);
    }

    Ok(())
}
```

### CDN (Browser)

```javascript
const url = 'https://github.com/anistark/wasmhub/releases/latest/download/go-1.23.wasm';
const response = await fetch(url);
const wasmBytes = await response.arrayBuffer();
const module = await WebAssembly.compile(wasmBytes);
```

## Available Runtimes

| Language | Versions | Size | Status |
|----------|----------|------|--------|
| **Go** | 1.23 | 261 KB | ✅ Available |
| **Rust** | 1.82 | 76 KB | ✅ Available |
| **Node.js** | — | — | 🔜 Coming Soon |
| **Python** | — | — | 🔜 Coming Soon |
| **Ruby** | — | — | 🔜 Coming Soon |
| **PHP** | — | — | 🔜 Coming Soon |

Both runtimes target **WASI Preview 1** (`wasip1`) and support filesystem, environment variables, arguments, and stdio.
