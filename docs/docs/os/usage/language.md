---
sidebar_position: 3
title: Language Selection
---

# Language Selection

## Auto-Detection

When no `--language` flag is provided, wasmrun detects the project language by checking for marker files:

| File | Detected Language | Runtime |
|---|---|---|
| `package.json` | `nodejs` | QuickJS (from wasmhub) |
| `requirements.txt` | `python` | RustPython (from wasmhub) |
| `pyproject.toml` | `python` | RustPython (from wasmhub) |
| `Cargo.toml` | `rust` | Rust WASM runtime |
| `go.mod` | `go` | Go WASM runtime |

Detection is checked in this order. The first match wins.

```sh
# Has package.json → detected as nodejs
wasmrun os ./my-node-app
# 🔍 Detected: nodejs project

# Has requirements.txt → detected as python
wasmrun os ./my-python-app
# 🔍 Detected: python project
```

If no marker files are found:

```
Could not auto-detect language for project: ./my-project
Please specify --language
```

## Manual Override

Use `-l` or `--language` to force a specific language:

```sh
wasmrun os ./my-app --language nodejs
wasmrun os ./my-app -l python
```

### Supported Values

| Value | Aliases | Runtime |
|---|---|---|
| `nodejs` | `node`, `js`, `javascript` | QuickJS |
| `python` | `py` | RustPython |

```sh
# All equivalent
wasmrun os ./app --language nodejs
wasmrun os ./app --language node
wasmrun os ./app --language js
wasmrun os ./app --language javascript
```

### When to Override

Override is useful when:

- The project has multiple language markers (e.g., both `package.json` and `requirements.txt`)
- Auto-detection picks the wrong language
- You want to test the same project with a different runtime

```sh
# Project has both package.json and requirements.txt
# Force Python
wasmrun os ./hybrid-project --language python
```

## Runtime Fetching

Language runtimes are WASM binaries from [wasmhub](https://github.com/anistark/wasmhub). They're fetched on first use and cached at `~/.wasmrun/runtimes/`.

```
~/.wasmrun/runtimes/
├── quickjs.wasm          # Node.js/JavaScript runtime
├── quickjs.wasm.sha256   # Checksum
├── rustpython.wasm       # Python runtime
└── rustpython.wasm.sha256
```

### Language Name Mapping

wasmrun maps detected language names to wasmhub runtime names:

| Detected Language | wasmhub Runtime |
|---|---|
| `nodejs` | `quickjs` |
| `python` | `rustpython` |
| `rust` | `rust` |
| `go` | `go` |

### Cache Management

Runtimes are cached indefinitely. To force a re-download:

```sh
# Remove cached runtime
rm ~/.wasmrun/runtimes/quickjs.wasm*

# Next run will re-fetch
wasmrun os ./my-app
```

### Integrity

Every download is validated with a SHA-256 checksum. If a cached file is corrupted, wasmrun detects the mismatch and re-downloads automatically.

## Runtime API

Check runtime status via the REST API:

```sh
# Get detected language and cache status
curl http://localhost:8420/api/runtimes
```

```json
{
  "detected_language": "nodejs",
  "wasmhub_runtime": "quickjs",
  "cached": true,
  "cached_version": "0.1.4",
  "available_languages": ["quickjs", "rustpython", "rust", "go"]
}
```

```sh
# Download the runtime binary directly
curl http://localhost:8420/api/runtime/nodejs -o runtime.wasm
```

## See Also

- [Running Projects](./running.md) — startup flow and UI
- [Server Options](./server-options.md) — port, CORS, watch configuration
- [Plugins](/docs/plugins) — language plugins for server mode (different from OS mode runtimes)
