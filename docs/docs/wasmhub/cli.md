---
sidebar_position: 3
title: CLI Reference
---

# CLI Reference

The WasmHub CLI (`wasmhub`) provides commands for downloading, inspecting, and managing WASM language runtimes.

## Installation

```sh
cargo install wasmhub --features cli
```

## Commands

### `wasmhub get`

Download a runtime (or return the cached version).

```sh
wasmhub get <language> [version] [--force]
```

| Argument | Description | Default |
|----------|-------------|---------|
| `language` | Language runtime to download | Required |
| `version` | Specific version | `latest` |
| `--force`, `-f` | Force re-download even if cached | `false` |

**Examples:**

```sh
# Download latest Go runtime
wasmhub get go

# Download specific version
wasmhub get go 1.23

# Force re-download
wasmhub get go 1.23 --force

# Use LTS version
wasmhub get nodejs lts
```

**Output:**

```
Fetching go runtime (version: 1.23)...

Success!
  Language: go
  Version: 1.23
  Path: /home/user/.cache/wasmhub/go/1.23.wasm
  Size: 0.25 MB
  SHA256: efa1e13f39dfd3783d0eff5669088ab99a1ea1d38ac79f29b02e2ad8ddfea29d
```

---

### `wasmhub list`

List all available runtimes from the registry.

```sh
wasmhub list [language]
```

| Argument | Description | Default |
|----------|-------------|---------|
| `language` | Filter to a specific language | All languages |

**Examples:**

```sh
# List all available runtimes
wasmhub list

# List only Go versions
wasmhub list go
```

---

### `wasmhub info`

Show detailed information about a runtime.

```sh
wasmhub info <language> [version]
```

| Argument | Description | Default |
|----------|-------------|---------|
| `language` | Language to inspect | Required |
| `version` | Show details for a specific version | Overview only |

**Examples:**

```sh
# Overview of Go runtimes
wasmhub info go

# Detailed info for a specific version
wasmhub info go 1.23
```

**Output (with version):**

```
Runtime Info: go

  Latest: 1.23
  Source: https://go.dev/
  License: BSD-3-Clause

Version Details for 1.23:

  File: go-1.23.wasm
  Size: 0.25 MB
  SHA256: efa1e13f...
  Released: 2026-02-03T13:23:13Z
  WASI: wasip1
```

---

### `wasmhub cache show`

Show cache location and list all cached runtimes.

```sh
wasmhub cache show
```

**Output:**

```
Cache Information:

  Location: /home/user/.cache/wasmhub

  Cached Runtimes:

    • go 1.23 (0.25 MB)
    • rust 1.82 (0.07 MB)

  Total: 2 runtimes, 0.32 MB total
```

---

### `wasmhub cache clear`

Remove a specific cached runtime.

```sh
wasmhub cache clear <language> <version>
```

**Example:**

```sh
wasmhub cache clear go 1.23
```

---

### `wasmhub cache clear-all`

Remove all cached runtimes.

```sh
wasmhub cache clear-all [--yes]
```

| Option | Description |
|--------|-------------|
| `--yes`, `-y` | Skip confirmation prompt |

**Example:**

```sh
# Interactive (prompts for confirmation)
wasmhub cache clear-all

# Non-interactive
wasmhub cache clear-all --yes
```

---

## Language Aliases

WasmHub accepts multiple aliases for each language:

| Language | Accepted values |
|----------|----------------|
| Node.js | `nodejs`, `node`, `node.js` |
| Python | `python`, `py` |
| Ruby | `ruby`, `rb` |
| PHP | `php` |
| Go | `go`, `golang` |
| Rust | `rust`, `rs` |

```sh
# These are equivalent
wasmhub get go 1.23
wasmhub get golang 1.23
```

---

## CDN Sources

The CLI downloads from multiple sources with automatic fallback:

1. **GitHub Releases** (primary) — `github.com/anistark/wasmhub/releases/`
2. **jsDelivr** (fallback) — `cdn.jsdelivr.net/gh/anistark/wasmhub@latest/`

If the primary source fails, the CLI automatically retries with exponential backoff, then falls back to the next source.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (network failure, runtime not found, integrity check failed, etc.) |
