---
sidebar_position: 1
title: Agent Mode
---

# Agent API

The agent API wraps exec mode in an HTTP server, letting AI agents create isolated WASM sandboxes, upload files, execute code, and retrieve structured output, all via REST.

## Starting the Server

```sh
wasmrun agent [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-P, --port` | `8430` | Server port |
| `--host` | `127.0.0.1` | Address to bind. Loopback only by default; `0.0.0.0` exposes the server on every interface |
| `--insecure` | off | Allow a non-loopback bind with auth disabled. Startup is refused otherwise |
| `-t, --timeout` | `300` | Default session idle timeout (seconds) |
| `-m, --max-sessions` | `100` | Maximum concurrent sessions |
| `--max-memory` | `256` | Maximum linear memory per session (MB) |
| `--max-fuel` | `0` | Instruction budget per execution (`0` = unlimited) |
| `--max-output` | `10` | Captured stdout+stderr per execution (MB) |
| `--max-file-size` | `50` | Maximum size of any single file write (MB) |
| `--max-disk` | `100` | Maximum total disk usage per session (MB) |
| `--max-body` | `32` | Maximum accepted request body size (MB) |
| `--max-concurrent-exec` | `100` | Maximum executions in flight across all sessions |
| `--workers` | `0` | Maximum HTTP request-handling threads (`0` = auto, derived from `--max-concurrent-exec`) |
| `--shutdown-timeout` | `10` | Seconds to let in-flight requests finish on shutdown before the process exits |
| `--max-cache-size` | `2048` | Ceiling on the shared npm package cache in MB (`0` = unlimited) |
| `--npm-registry` | `https://registry.npmjs.org` | npm registry base URL for dependency vendoring |
| `--allow-cors` | off | Enable wildcard CORS |
| `-v, --verbose` | off | Add a request-received line per request (a structured access log is always emitted; see [Observability](./usage/observability.md)) |
| `--auth <PATH>` | off | Path to a TOML auth config; enables API-key auth & tenant isolation (omit = open) |
| `--hash-key <KEY>` | - | Print `sha256(KEY)` for the auth config and exit (does not start the server) |

For every size/count limit, `0` means **unlimited** (`--workers` is the exception: `0` means auto). Memory, fuel, output, file-size, and disk caps are **per session** and can be overridden per session at creation (see [Sessions](./usage/sessions.md)); body size and exec concurrency are **server-wide** ingress guards.

All endpoints are under `http://<host>:<port>/api/v1/`.

## Request concurrency

Requests are handled on a pool of worker threads, so a long execution never blocks anything else: other sessions, other tenants, file writes, and `/metrics` all stay responsive while an exec runs to its timeout.

Threads are spawned **on demand** and capped at `--workers`, so an idle server holds none of them. When every worker is busy the server stops accepting until one frees up, leaving the excess queued in the TCP backlog rather than growing threads without limit.

`--workers 0` (the default) sizes the pool from the exec cap: `--max-concurrent-exec` plus 16 spare workers for requests that do not execute anything, or 512 when execution concurrency is unlimited. A request that starts an execution is occupied for its whole duration, so a pool smaller than `--max-concurrent-exec` becomes the real ceiling on concurrent executions. Set `--workers` explicitly to bound memory on a small host, and watch `wasmrun_agent_workers_live` and `wasmrun_agent_requests_in_flight` in [the metrics](./usage/observability.md) to see how much of the pool is actually in use.

## Network exposure and TLS

The server binds **`127.0.0.1` by default**, so a fresh `wasmrun agent` is reachable only from the machine it runs on. That is deliberate: the exec endpoint runs arbitrary code, so a server anyone can reach is a server anyone can run code on.

To serve other hosts, pass `--host 0.0.0.0` (or a specific interface address). Without `--auth`, that combination **refuses to start**:

```sh
wasmrun agent --host 0.0.0.0
# ❌ refusing to bind 0.0.0.0 with authentication disabled.
#      --auth <PATH>      enable API-key auth
#      --host 127.0.0.1   keep the server on loopback (the default)
#      --insecure         bind anyway; only on a network you control
```

`--insecure` is the escape hatch for a network you fully control, such as a private container network. It prints a warning banner at startup and is never the right choice on anything an untrusted client can reach.

**TLS is not terminated in-process.** Traffic is plaintext HTTP, API keys included, so anything beyond loopback belongs behind a reverse proxy (nginx, Caddy, a cloud load balancer) that terminates TLS and forwards to the agent. The recommended shape:

- Bind the agent to loopback, or to a private interface the proxy alone can reach
- Terminate TLS at the proxy and forward to `http://127.0.0.1:8430`
- Keep `--auth` on, so a key is required even if the port is reached directly
- Point liveness and readiness probes at `/health` and `/ready` (see [Observability](./usage/observability.md#health-and-readiness))

## Restarts and shutdown

**Sessions do not survive a restart.** They live in memory, with their files in a temp directory, and a restart destroys all of them. This is an explicit non-goal: there is no persistence, no handoff between instances, and no rolling restart that preserves work. A client should treat a 404 on a session it holds as "create a new one" (see [Sessions](./usage/sessions.md#sessions-do-not-survive-a-restart)).

That fixes the supported deployment shape:

- **One instance owns its sessions.** Behind a load balancer, requests for a session must reach the instance that created it, since any other instance returns 404. Route by session id, or run a single instance per pool.
- **Scale by adding pools, not replicas.** Two instances behind a round-robin balancer will appear to lose sessions at random, which is a routing mistake rather than a bug.

### Draining

`SIGINT` (Ctrl+C), `SIGTERM` and `SIGHUP` all start a clean shutdown. `SIGTERM` matters most: it is what `docker stop`, Kubernetes and systemd send, and a server that ignored it would be `SIGKILL`ed at the end of the stop timeout with its session directories left behind.

The sequence:

1. The listener stops accepting; anything already accepted gets **503**
2. `/ready` reports `shutting_down` (it has done so since the signal arrived)
3. In-flight requests get up to `--shutdown-timeout` seconds to finish. A long execution can outlive that window; it is abandoned at the deadline and the count is logged
4. Every session is destroyed and its directory removed

Give the orchestrator's stop timeout more room than `--shutdown-timeout`, or it will `SIGKILL` mid-drain. To take an instance out of rotation before the signal, deregister it from the balancer first (a `preStop` hook, or waiting one probe interval after `/ready` starts failing); the agent does not delay its own shutdown to wait for probes.

### Orphaned session directories

A crash or `SIGKILL` runs no destructors, so the session tree survives the process. Each server owns one directory, `<temp>/wasmrun-agent-<pid>-<timestamp>/`, containing every session it created and a heartbeat file it refreshes on each cleanup tick.

At startup a server sweeps the temp directory and removes any such tree whose heartbeat has gone stale, meaning no live server has touched it for ten cleanup intervals (at least five minutes). A running server's tree is never swept, because its heartbeat is current. Directories from before this scheme (`wasmrun-session-*`, one per session at the top of the temp directory) are collected once they are more than a day old. The count is reported at startup:

```
   Swept 3 orphaned session tree(s) from a previous run
```

## Disk and caches

Two caches live in the operator's home directory, outside any session, and are shared by every session and tenant on the host:

| Path | Holds | Bounded by |
|------|-------|-----------|
| `~/.wasmrun/npm/` | Downloaded npm packages, per `name@version`, plus their CommonJS-lowered forms | `--max-cache-size` (default 2048 MB) |
| `~/.wasmrun/runtimes/` | wasmhub language runtimes (`.wasm`) and their metadata | One artifact per language; superseded ones are deleted |

Both are swept at startup and every five minutes after that. The npm cache is trimmed **least-recently-used first** until it fits under the ceiling, where "used" means the last time an install read the entry, not when it was downloaded. Entries installed from within the last ten minutes are never removed, so a package being copied into a session cannot be deleted mid-install. If that leaves the cache over its ceiling, the overage is logged rather than forced:

```
Cache: evicted 12 npm entr(ies), freed 340.2 MB, 1907.4 MB in use
Cache: removed 1 superseded runtime artifact(s)
```

Directories left behind by an install that was interrupted partway are always cleared, ceiling or not. Bumping the pinned wasmhub release points each language at a new artifact and the old one is removed on the next sweep; without that, the runtime cache would grow by one full runtime per release.

Setting `--max-cache-size 0` disables the size ceiling entirely, leaving only the debris and superseded-runtime cleanup. Evicting a package costs a re-download the next time it is needed, never correctness: every install verifies integrity against the registry regardless of what the cache holds.

## Authentication

By default the server is **open**; any caller can create and access any session. Pass `--auth <path>` to require an API key on every request and isolate sessions per tenant. Without `--auth`, behavior is exactly as before (no header needed).

```sh
wasmrun agent --port 8430 --auth ./auth.toml
# banner shows:  Auth:  enabled (2 tenants)
```

### Config file

The auth config is a TOML file listing tenants. Keys are stored **hashed** (SHA-256, hex), never in plaintext:

```toml
[[tenants]]
id = "copilot"
key_sha256 = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"

[[tenants]]
id = "ci"
key_sha256 = "60303ae22b998861bce3b28f33eec1be758a213c86c93c076dbe9f558c11c752"
```

Each `id` and `key_sha256` must be unique, and `key_sha256` must be 64 lowercase hex characters. Invalid or missing config **aborts startup**; the server never silently runs open when auth was requested. Restrict the file so other users can't read the hashes:

```sh
chmod 600 auth.toml
```

### Generating a key hash

Generate a high-entropy random key, then hash it for the config:

```sh
KEY=$(openssl rand -hex 32)
wasmrun agent --hash-key "$KEY"
# → 4b4090ccee1e713c3d411b96a4226b90bd0f0deb34e02d19475a951316fd04ee
```

Put the hash in `key_sha256`, hand the raw `$KEY` to that tenant, and keep the raw key out of the config.

### Making authenticated requests

Send the raw key as a Bearer token on every `/api/v1/*` request (including `/tools`):

```sh
curl -X POST http://localhost:8430/api/v1/sessions \
  -H "Authorization: Bearer $KEY"
```

A missing, malformed, or unknown key returns **401 Unauthorized**.

### Tenant isolation

Each session is owned by the tenant that created it. A tenant can only see and operate on its own sessions; any request targeting another tenant's session returns **404 Not Found**, identical to a nonexistent session so existence isn't leaked.

### Per-tenant limits and rate limits

Each tenant can carry its own resource ceiling and request budget, layered on top of the server defaults. Both are optional sub-tables under a `[[tenants]]` entry:

```toml
[[tenants]]
id = "ci"
key_sha256 = "60303ae22b998861bce3b28f33eec1be758a213c86c93c076dbe9f558c11c752"

  [tenants.limits]
  max_memory_mb = 128
  max_disk_mb = 50

  [tenants.rate]
  max_sessions = 10
  max_concurrent_exec = 4
  max_requests_per_min = 600
```

`[tenants.limits]` sets a per-tenant resource ceiling, with the same fields as a [per-session override](./usage/sessions.md#per-session-limit-overrides) (`max_memory_mb`, `max_fuel`, `max_output_mb`, `max_file_size_mb`, `max_disk_mb`). Effective session limits compose in three layers: **server defaults → tenant baseline → per-session override clamped to the tenant baseline**. The tenant ceiling is a hard cap; a per-session override may only *tighten* a dimension, never raise it above the tenant's cap (a per-session "unlimited" `0` is pulled down to the tenant's finite ceiling).

`[tenants.rate]` throttles the tenant independently so one tenant cannot exhaust the shared server: `max_sessions`, `max_concurrent_exec`, `max_requests_per_min` (each `0` or omitted inherits the server-wide default). Over any of these limits returns **429 Too Many Requests**.

In open mode (no `--auth`) there is no tenant baseline: a per-session override applies un-clamped and only the global limits apply, exactly as before.

### Live config reload

The `--auth` file is watched for modification and reloaded **without a restart**; edit the config and the new tenants, keys, limits, and rates take effect for subsequent key resolution and newly created sessions. In-flight sessions keep their original owner and limits. A malformed or invalid edit is **logged and ignored**, keeping the previous config, so a bad edit never drops auth or crashes the server. The banner shows the watched path.

## How It Works

The agent API manages **sessions**. Each session is an isolated exec mode sandbox with its own:

- **Filesystem**: temp directory on the host, preopened at `/` via WASI
- **Environment variables**: independent per session
- **Output buffers**: stdout/stderr captured per execution
- **Timeout**: auto-cleanup after idle expiry

The exec endpoint accepts four input modes (a shell command line, a JavaScript or TypeScript source snippet, a multi-file JS/TS project, or a pre-compiled `.wasm` file) and returns captured stdout/stderr/exit code as JSON. JavaScript runs through the [wasmhub `nodejs` runtime](https://anistark.github.io/wasmhub/runtimes/nodejs/); TypeScript is first transpiled to JavaScript by the [wasmhub `swc` module](https://anistark.github.io/wasmhub/runtimes/swc/) running inside the same sandbox; WASM modules run through the same interpreter used by `wasmrun exec`. Shell commands are handled by an in-process built-in shell with no subprocess or host shell access.

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

# Run a shell command in the session
curl -X POST http://localhost:8430/api/v1/sessions/a1b2c3.../exec \
  -H "Content-Type: application/json" \
  -d '{"command": "echo hello > out.txt && cat out.txt"}'
# → {"stdout": "hello\n", "stderr": "", "exit_code": 0, ...}

# Or run JavaScript inline
curl -X POST http://localhost:8430/api/v1/sessions/a1b2c3.../exec \
  -H "Content-Type: application/json" \
  -d '{"source": "console.log(1+1)", "language": "javascript"}'
# → {"stdout": "2\n", "exit_code": 0, ...}

# Or run a pre-compiled WASM file
curl -X POST http://localhost:8430/api/v1/sessions/a1b2c3.../files \
  -H "Content-Type: application/json" \
  -d '{"path": "hello.wasm", "content": "..."}'
curl -X POST http://localhost:8430/api/v1/sessions/a1b2c3.../exec \
  -H "Content-Type: application/json" \
  -d '{"wasm_path": "hello.wasm"}'
# → {"stdout": "Hello, World!\n", "stderr": "", "exit_code": 0, "duration_ms": 12}

# List the sessions you can reuse
curl http://localhost:8430/api/v1/sessions
# → {"sessions": [{"session_id": "a1b2c3...", "state": "active", ...}], "count": 1}

# Clean up
curl -X DELETE http://localhost:8430/api/v1/sessions/a1b2c3...
```

See the [Agent Execution](./usage/exec.md) reference for all four input modes (shell `command`, JS `source`, multi-file `files`+`entry`, `wasm_path`).

## Tool Schemas for LLM Agents

The server exposes tool definitions that can be passed directly to OpenAI or Anthropic APIs for function calling:

```sh
# OpenAI format (default)
curl http://localhost:8430/api/v1/tools

# Anthropic format
curl http://localhost:8430/api/v1/tools?format=anthropic
```

Available tools: `create_session`, `execute_code`, `write_file`, `read_file`, `list_files`, `list_sessions`, `destroy_session`.

Each tool includes a description, parameter schema with types, and required fields, ready to pass to an LLM as function definitions.

## Observability

The server exposes runtime metrics at `GET /api/v1/metrics` (Prometheus text by default, JSON with `?format=json`) and writes a structured, request-id-tagged access-log line to stderr for every request. `GET /health` and `GET /ready` are unauthenticated probes for orchestrators. See [Observability](./usage/observability.md) for the full metric set, the probe semantics, and the log format.

```sh
curl http://localhost:8430/api/v1/metrics
# wasmrun_agent_exec_total{result="success"} 12
# wasmrun_agent_sessions_active 3
# ...
```

## API Reference

See the usage sub-pages for full endpoint documentation:

- [Sessions](./usage/sessions.md): create, status, destroy
- [Execution](./usage/exec.md): run WASM with timeout and structured output
- [File Operations](./usage/files.md): write, read, list, delete
- [Environment Variables](./usage/environment.md): set and get per-session env
- [Observability](./usage/observability.md): metrics endpoint and access log
