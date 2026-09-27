---
sidebar_position: 2
title: Features
---

# OS Mode Features

## Multi-Language Runtimes

Language runtimes are fetched from [wasmhub](https://github.com/anistark/wasmhub) and cached locally at `~/.wasmrun/runtimes/`:

| Language | Runtime | Detection |
|---|---|---|
| Node.js / JavaScript | [wasmhub `nodejs`](https://anistark.github.io/wasmhub/runtimes/nodejs/) | `package.json` |
| Python | wasmhub `rustpython` (integration in progress) | `requirements.txt`, `pyproject.toml` |
| Rust | Rust WASM runtime | `Cargo.toml` |
| Go | Go WASM runtime | `go.mod` |

Runtimes are downloaded on first use with SHA-256 checksum validation.

## Browser UI

The OS mode UI provides several panels:

- **Console**: live stdout/stderr with color-coded streams (green for stdout, red for stderr, blue for system) and timestamps
- **Filesystem**: browse the WASI virtual filesystem populated from your project files
- **Kernel Status**: active processes, memory usage, WASI capabilities, supported languages
- **Logs**: structured log trail from kernel, server, and runtime events
- **Application**: iframe for app output (when running web servers)

## Virtual Filesystem

Project files are served via `GET /api/project/files` as a base64-encoded JSON bundle:

- `.gitignore` patterns respected (glob matching with `*`, `**`, `?`, negation)
- Default ignore patterns for `node_modules`, `target`, `.git`, `__pycache__`, binary files
- Size limits: 10MB per file, 50MB total, 5000 file cap
- Files are decoded in the browser and written to the WASI virtual FS

## Networking

A program in the VM has no network of its own. What it gets goes through a local proxy under a policy the project sets in `wasmrun.toml`:

- A port from the policy's `bind_ports` range is bound for the program when it starts, so `server.listen()` in a Node project answers requests on `localhost`
- Outbound connections are not wired up yet

See [Network Policy](./network-isolation.md) and [Serving a Port](./port-forwarding.md).

## Public Tunneling

`wasmrun os --expose` publishes the program's port on the internet through a [bore](https://github.com/ekzhang/bore) server, `bore.pub` by default:

- One public URL for the session, kept across **Stop** and **Run**
- Reconnects on its own and asks for the same public port back
- Private bore servers with `--tunnel-server` and a secret
- Local and public addresses in the Console panel, with the tunnel's state

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
| `/api/network/status` | GET | Network proxy URL and `bind_ports` |
| `/api/tunnel/status` | GET | Tunnel status and public URL |
| `/api/tunnel/target` | POST | Point the tunnel at the program's port |
| `/api/tunnel/start` | POST | Reopen the tunnel (`--expose` only) |
| `/api/tunnel/stop` | POST | Close the tunnel |
| `/api/processes/<pid>/ports` | GET | List port mappings |
| `/api/processes/<pid>/forward` | POST | Create port forward |
