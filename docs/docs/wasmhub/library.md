---
sidebar_position: 4
title: Library Guide
---

# Rust Library Guide

WasmHub's Rust library provides programmatic access to download, cache, and verify WASM language runtimes.

## Setup

```toml
[dependencies]
wasmhub = "0.1"
tokio = { version = "1", features = ["full"] }
```

The library compiles with zero features by default (no CLI dependencies).

### Optional Features

| Feature | What it enables |
|---------|----------------|
| `progress` | Download progress bars via `indicatif` |

```toml
wasmhub = { version = "0.1", features = ["progress"] }
```

## Core API

### RuntimeLoader

The main entry point for all operations.

```rust
use wasmhub::{RuntimeLoader, Language};

#[tokio::main]
async fn main() -> wasmhub::Result<()> {
    let loader = RuntimeLoader::new()?;

    // Download or get from cache
    let runtime = loader.get_runtime(Language::Go, "1.23").await?;
    println!("Path: {}", runtime.path.display());

    Ok(())
}
```

### Builder Pattern

Use the builder for custom configuration:

```rust
use wasmhub::{RuntimeLoader, CdnSource};
use std::path::PathBuf;

let loader = RuntimeLoader::builder()
    // Custom cache directory
    .cache_dir(PathBuf::from("/tmp/my-cache"))
    // Only use GitHub Releases
    .cdn_sources(vec![CdnSource::GitHubReleases])
    // Retry configuration
    .max_retries(5)
    .initial_backoff_ms(1000)
    .max_backoff_ms(60_000)
    .build()?;
```

### Language Enum

Represents supported runtime languages:

```rust
use wasmhub::Language;

// Parse from string
let lang: Language = "go".parse().unwrap();
let lang: Language = "golang".parse().unwrap(); // aliases work

// All supported languages
for lang in Language::all() {
    println!("{}", lang.as_str());
}
```

### Runtime Struct

Returned by `get_runtime()`:

```rust
pub struct Runtime {
    pub language: Language,
    pub version: String,
    pub path: PathBuf,    // Local filesystem path
    pub size: u64,        // Size in bytes
    pub sha256: String,   // SHA256 checksum
}
```

## Common Patterns

### Download with Version Resolution

```rust
use wasmhub::{RuntimeLoader, Language};

async fn get_latest(loader: &RuntimeLoader) -> wasmhub::Result<()> {
    // Get latest version string
    let version = loader.get_latest_version(Language::Go).await?;
    println!("Latest Go: {}", version);

    // Then download it
    let runtime = loader.get_runtime(Language::Go, &version).await?;
    println!("Downloaded to: {}", runtime.path.display());

    Ok(())
}
```

### List Available Runtimes

```rust
use wasmhub::RuntimeLoader;

async fn list_all(loader: &RuntimeLoader) -> wasmhub::Result<()> {
    let manifest = loader.list_available().await?;

    for (name, info) in &manifest.languages {
        println!("{}: latest={}, versions={:?}",
            name, info.latest, info.versions);
    }

    Ok(())
}
```

### Inspect Runtime Manifest

```rust
use wasmhub::{RuntimeLoader, Language};

async fn inspect(loader: &RuntimeLoader) -> wasmhub::Result<()> {
    let manifest = loader.fetch_runtime_manifest(Language::Go).await?;

    for (version, info) in &manifest.versions {
        println!("{}: {} bytes, sha256={}", version, info.size, info.sha256);
        println!("  WASI: {}, features: {:?}", info.wasi, info.features);
    }

    Ok(())
}
```

### Cache Management

```rust
use wasmhub::{RuntimeLoader, Language};

async fn manage_cache(loader: &RuntimeLoader) -> wasmhub::Result<()> {
    // List cached runtimes
    let cached = loader.list_cached()?;
    for rt in &cached {
        println!("{} {} ({} bytes)", rt.language, rt.version, rt.size);
    }

    // Clear specific runtime
    loader.clear_cache(Language::Go, "1.23")?;

    // Clear all
    loader.clear_all_cache()?;

    Ok(())
}
```

## Error Handling

WasmHub uses a custom `Error` enum via `thiserror`:

```rust
use wasmhub::{RuntimeLoader, Language, Error};

async fn handle_errors(loader: &RuntimeLoader) {
    match loader.get_runtime(Language::Go, "9.99").await {
        Ok(runtime) => println!("Got: {}", runtime.path.display()),
        Err(Error::VersionNotFound { language, version }) => {
            eprintln!("{} {} is not available", language, version);
        }
        Err(Error::ManifestNotFound { language }) => {
            eprintln!("No manifest found for {}", language);
        }
        Err(Error::IntegrityCheckFailed { expected, actual }) => {
            eprintln!("Checksum mismatch! expected={}, got={}", expected, actual);
        }
        Err(Error::Network(e)) => {
            eprintln!("Network error: {}", e);
        }
        Err(e) => eprintln!("Other error: {}", e),
    }
}
```

### Error Variants

| Variant | When |
|---------|------|
| `RuntimeNotFound` | Language/version combo doesn't exist |
| `VersionNotFound` | Version not in manifest |
| `ManifestNotFound` | No manifest for language |
| `IntegrityCheckFailed` | SHA256 mismatch after download |
| `Network` | HTTP request failure |
| `Io` | Filesystem error |
| `JsonError` | Manifest parse failure |
| `InvalidLanguage` | Unknown language string |

## Integration with Wasmrun

WasmHub is used internally by [wasmrun's OS mode](/docs/os) to fetch runtimes:

```rust
use wasmhub::{RuntimeLoader, Language};

// In wasmrun's OS mode boot sequence:
let loader = RuntimeLoader::new()?;
let runtime = loader.get_runtime(Language::Go, "1.23").await?;

// Read the .wasm bytes and instantiate
let wasm_bytes = std::fs::read(&runtime.path)?;
// ... pass to WebAssembly.instantiate() in the browser
```

## API Reference

For the complete API reference with all types and methods, see [docs.rs/wasmhub](https://docs.rs/wasmhub).
