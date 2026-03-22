---
sidebar_position: 2
title: Features
---

# OS Mode Features

## Multi-Language Runtimes

Language runtimes are fetched from [wasmhub](https://github.com/anistark/wasmhub) and cached locally at `~/.wasmrun/runtimes/`:

| Language | Runtime | Detection |
|---|---|---|
| Node.js / JavaScript | QuickJS | `package.json` |
| Python | RustPython | `requirements.txt`, `pyproject.toml` |
| Rust | Rust WASM runtime | `Cargo.toml` |
| Go | Go WASM runtime | `go.mod` |

Runtimes are downloaded on first use with SHA-256 checksum validation.

## Browser UI

The OS mode UI provides several panels:

- **Console** — live stdout/stderr with color-coded streams (green for stdout, red for stderr, blue for system) and timestamps
- **Filesystem** — browse the WASI virtual filesystem populated from your project files
- **Kernel Status** — active processes, memory usage, WASI capabilities, supported languages
- **Logs** — structured log trail from kernel, server, and runtime events
- **Application** — iframe for app output (when running web servers)

## Virtual Filesystem

Project files are served via `GET /api/project/files` as a base64-encoded JSON bundle:

- `.gitignore` patterns respected (glob matching with `*`, `**`, `?`, negation)
- Default ignore patterns for `node_modules`, `target`, `.git`, `__pycache__`, binary files
- Size limits: 10MB per file, 50MB total, 5000 file cap
- Files are decoded in the browser and written to the WASI virtual FS

## Network Isolation

Each process runs in its own network namespace:

- Isolated port bindings (no conflicts between processes)
- Port forwarding from guest to host
- Connection tracking and stats
- Per-process network statistics

See [Network Isolation](./network-isolation.md) for details.

## Port Forwarding

Expose services running in isolated namespaces:

```sh
wasmrun os ./app --forward 8080:3000
```

See [Port Forwarding](./port-forwarding.md) for details.

## Public Tunneling

Expose local apps to the internet via [bore.pub](https://bore.pub):

- Built-in bore client (Rust implementation)
- Automatic reconnection
- Optional authentication for private servers
- Start/stop/status via REST API

See [Public Tunneling](./public-tunneling.md) for details.

## REST API

OS mode exposes a JSON API:

| Endpoint | Method | Description |
|---|---|---|
| `/api/kernel/stats` | GET | Kernel statistics (processes, memory, capabilities) |
| `/api/fs/stats` | GET | Filesystem statistics |
| `/api/fs/read/<path>` | GET | Read file contents |
| `/api/fs/list/<path>` | GET | List directory |
| `/api/fs/write/<path>` | POST | Write file |
| `/api/fs/mkdir/<path>` | POST | Create directory |
| `/api/fs/delete/<path>` | POST | Delete file |
| `/api/project/files` | GET | Get all project files (base64 bundle) |
| `/api/runtime/<language>` | GET | Serve cached runtime WASM binary |
| `/api/runtimes` | GET | Available runtimes manifest |
| `/api/logs` | GET | All structured logs |
| `/api/logs/recent` | GET | Recent logs |
| `/api/kernel/start` | POST | Start project |
| `/api/kernel/restart` | POST | Restart project |
| `/api/tunnel/start` | POST | Start bore tunnel |
| `/api/tunnel/status` | GET | Tunnel status |
| `/api/tunnel/stop` | POST | Stop tunnel |
| `/api/processes/<pid>/ports` | GET | List port mappings |
| `/api/processes/<pid>/forward` | POST | Create port forward |
