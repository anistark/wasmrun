---
sidebar_position: 2
title: Getting Started
---

# Getting Started

WasmHub can be used in two ways: as a **CLI tool** for downloading and managing runtimes, or as a **Rust library** for programmatic access.

## Installation

### CLI Tool

```sh
cargo install wasmhub --features cli
```

Verify the installation:

```sh
wasmhub --version
```

### Rust Library

Add to your `Cargo.toml`:

```toml
[dependencies]
wasmhub = "0.1"
tokio = { version = "1", features = ["full"] }
```

For download progress bars, enable the `progress` feature:

```toml
wasmhub = { version = "0.1", features = ["progress"] }
```

## First Download

### Using the CLI

```sh
# Download the latest Go runtime
wasmhub get go

# Download a specific version
wasmhub get go 1.23

# Check what's available
wasmhub list
```

The runtime is downloaded to `~/.cache/wasmhub/` and reused on subsequent calls.

### Using the Library

```rust
use wasmhub::{RuntimeLoader, Language};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loader = RuntimeLoader::new()?;

    // Downloads if not cached, returns cached path otherwise
    let runtime = loader.get_runtime(Language::Go, "1.23").await?;

    println!("Language: {}", runtime.language);
    println!("Version:  {}", runtime.version);
    println!("Path:     {}", runtime.path.display());
    println!("Size:     {} bytes", runtime.size);
    println!("SHA256:   {}", runtime.sha256);

    Ok(())
}
```

## How It Works

```
wasmhub get go 1.23
        │
        ▼
┌─ Check local cache ──────────────────┐
│  ~/.cache/wasmhub/go/1.23.wasm       │
│  Found? → Return immediately         │
│  Not found? → Continue to download   │
└───────────────────────────────────────┘
        │
        ▼
┌─ Fetch runtime manifest ─────────────┐
│  GET go-manifest.json                 │
│  Contains: versions, sizes, sha256s   │
└───────────────────────────────────────┘
        │
        ▼
┌─ Download binary ─────────────────────┐
│  GET go-1.23.wasm                     │
│  Sources: GitHub Releases → jsDelivr  │
│  Retries with exponential backoff     │
└───────────────────────────────────────┘
        │
        ▼
┌─ Verify integrity ───────────────────┐
│  Compute SHA256 of downloaded bytes   │
│  Compare against manifest checksum    │
│  Mismatch → Error (no cache)          │
└───────────────────────────────────────┘
        │
        ▼
┌─ Cache and return ───────────────────┐
│  Store to ~/.cache/wasmhub/go/       │
│  Return Runtime { path, sha256, .. } │
└───────────────────────────────────────┘
```

## Cache Management

Runtimes are cached at `~/.cache/wasmhub/` (Linux/macOS) or the platform's cache directory.

```sh
# See what's cached
wasmhub cache show

# Clear a specific runtime
wasmhub cache clear go 1.23

# Clear everything
wasmhub cache clear-all --yes
```

## Next Steps

- [CLI Reference](./cli.md) — All commands and options
- [Library Guide](./library.md) — Rust API usage patterns
- [Runtimes](./runtimes.md) — Available runtimes and capabilities
