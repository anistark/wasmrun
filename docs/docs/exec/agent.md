---
sidebar_position: 5
title: Agent API
---

# Agent API

The agent API wraps exec mode in an HTTP server, letting AI agents create isolated WASM sandboxes, upload files, execute code, and retrieve structured output — all via REST.

## Starting the Server

```sh
wasmrun agent [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-P, --port` | `8430` | Server port |
| `-t, --timeout` | `300` | Default session idle timeout (seconds) |
| `-m, --max-sessions` | `100` | Maximum concurrent sessions |
| `--max-memory` | `256` | Maximum linear memory per session (MB) |
| `--allow-cors` | off | Enable wildcard CORS |
| `-v, --verbose` | off | Log all incoming requests |

All endpoints are under `http://<host>:<port>/api/v1/`.

## How It Works

The agent API manages **sessions** — each session is an isolated exec mode sandbox with its own:

- **Filesystem** — temp directory on the host, preopened at `/` via WASI
- **Environment variables** — independent per session
- **Output buffers** — stdout/stderr captured per execution
- **Timeout** — auto-cleanup after idle expiry

When you call the exec endpoint, the server loads the WASM file from the session's filesystem, runs it through the same interpreter used by `wasmrun exec`, and returns the captured output as JSON.

```
┌─ wasmrun agent ─────────────────────────────────────────┐
│                                                         │
│  REST API (/api/v1/...)                                 │
│       ↓                                                 │
│  Session Manager → create/track/expire/destroy          │
│       ↓                                                 │
│  Per-Session Sandbox                                    │
│    ├─ Isolated temp directory (WASI preopen at /)       │
│    ├─ WasiEnv (stdout/stderr, args, env vars)           │
│    └─ Idle timeout tracking                             │
│       ↓                                                 │
│  Exec Mode Engine (same as `wasmrun exec`)              │
│    ├─ Module parser                                     │
│    ├─ Bytecode interpreter                              │
│    ├─ Linear memory                                     │
│    └─ WASI syscalls                                     │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Quick Example

```sh
# Start the server
wasmrun agent --port 8430

# Create a session
curl -X POST http://localhost:8430/api/v1/sessions
# → {"session_id": "a1b2c3...", "created_at": "..."}

# Upload a WASM file
curl -X POST http://localhost:8430/api/v1/sessions/a1b2c3.../files \
  -H "Content-Type: application/json" \
  -d '{"path": "hello.wasm", "content": "..."}'

# Execute it
curl -X POST http://localhost:8430/api/v1/sessions/a1b2c3.../exec \
  -H "Content-Type: application/json" \
  -d '{"wasm_path": "hello.wasm"}'
# → {"stdout": "Hello, World!\n", "stderr": "", "exit_code": 0, "duration_ms": 12}

# Clean up
curl -X DELETE http://localhost:8430/api/v1/sessions/a1b2c3...
```

## Tool Schemas for LLM Agents

The server exposes tool definitions that can be passed directly to OpenAI or Anthropic APIs for function calling:

```sh
# OpenAI format (default)
curl http://localhost:8430/api/v1/tools

# Anthropic format
curl http://localhost:8430/api/v1/tools?format=anthropic
```

Available tools: `create_session`, `execute_code`, `write_file`, `read_file`, `list_files`, `destroy_session`.

Each tool includes a description, parameter schema with types, and required fields — ready to pass to an LLM as function definitions.

## API Reference

See the usage sub-pages for full endpoint documentation:

- [Sessions](./usage/agent-sessions.md) — create, status, destroy
- [Execution](./usage/agent-exec.md) — run WASM with timeout and structured output
- [File Operations](./usage/agent-files.md) — write, read, list, delete
- [Environment Variables](./usage/agent-environment.md) — set and get per-session env
