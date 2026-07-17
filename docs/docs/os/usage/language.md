---
sidebar_position: 3
title: Language Selection
---

# Language Selection

## Auto-Detection

When no `--language` flag is provided, wasmrun detects the project language by checking for marker files:

| File | Detected Language | Runtime |
|---|---|---|
| `package.json` | `nodejs` | [wasmhub `nodejs` runtime](https://anistark.github.io/wasmhub/runtimes/nodejs/) |
| `Cargo.toml` | `rust` | [wasmhub `rust` runtime](https://anistark.github.io/wasmhub/runtimes/rust/); OS-kernel integration pending |
| `go.mod` | `go` | [wasmhub `go` runtime](https://anistark.github.io/wasmhub/runtimes/go/); OS-kernel integration pending |
| `requirements.txt` | `python` | wasmhub `rustpython` runtime |
| `pyproject.toml` | `python` | wasmhub `rustpython` runtime |

Detection is checked in this order. The first match wins.

:::note Runtime availability
The OS kernel currently registers the **Node.js runtime only**. Python (`rustpython`) and Go runtimes are scaffolded in the codebase but not yet enabled, so Node.js projects are the fully supported path today.
:::

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
| `nodejs` | `node`, `js`, `javascript` | [wasmhub `nodejs`](https://anistark.github.io/wasmhub/runtimes/nodejs/) |
| `python` | `py` | wasmhub `rustpython` (integration in progress) |

Any other value is rejected: `Unsupported OS mode language: '<value>'. Supported languages: nodejs, python`.

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

Language runtimes are WASM binaries from [wasmhub](https://github.com/anistark/wasmhub), pinned to a specific wasmhub release. They're fetched on first use and cached at `~/.wasmrun/runtimes/`:

```
~/.wasmrun/runtimes/
├── nodejs.json       # Metadata: version, sha256, wasmhub release
├── nodejs-20.wasm    # Runtime binary (filename comes from the wasmhub manifest)
└── ...
```

### Language Name Mapping

wasmrun maps detected language names to wasmhub runtime names:

| Detected Language | wasmhub Runtime |
|---|---|
| `nodejs`, `javascript`, `js` | `nodejs` |
| `python` | `rustpython` |

Other names pass through unchanged.

### Cache Management

Cached runtimes are keyed to the wasmhub release wasmrun is pinned to; when a new wasmrun version bumps the pin, stale runtimes are invalidated and re-fetched automatically. To force a re-download manually:

```sh
# Remove cached runtime and metadata
rm ~/.wasmrun/runtimes/nodejs*

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
  "wasmhub_runtime": "nodejs",
  "cached": true,
  "cached_version": "0.1.4",
  "wasmhub_version": "0.3.2",
  "available_languages": ["nodejs", "rustpython"]
}
```

`wasmhub_version` and `available_languages` come from the live wasmhub manifest and are omitted if it can't be fetched.

```sh
# Download the runtime binary directly
curl http://localhost:8420/api/runtime/nodejs -o runtime.wasm
```

## See Also

- [Running Projects](./running.md): startup flow and UI
- [Server Options](./server-options.md): port, CORS, watch configuration
- [Plugins](/docs/plugins): language plugins for server mode (different from OS mode runtimes)
