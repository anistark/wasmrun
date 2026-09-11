---
sidebar_position: 6
title: Serving
---

# Serving

A session can run a long-lived server: a program that binds a port, keeps running, and answers requests until you stop it.

This is the counterpart to [Execution](./exec.md). `POST /exec` runs a program to completion and answers with what it printed, which is the wrong shape for a server: by the time the response could tell you the port, the thing listening on it would be gone. A server is started, addressed, and stopped as three separate requests.

## Starting a server

```http
POST /api/v1/sessions/{id}/serve
```

```sh
curl -X POST http://localhost:8430/api/v1/sessions/$SESSION/serve \
  -H 'Content-Type: application/json' \
  -d '{
    "source": "const http = require(\"http\");\nhttp.createServer((req, res) => {\n  res.writeHead(200, {\"Content-Type\": \"application/json\"});\n  res.end(JSON.stringify({ ok: true }));\n}).listen();",
    "language": "javascript"
  }'
```

```json
{
  "server_id": "srv_0a1b2c3d4e5f",
  "addr": "127.0.0.1:53412",
  "port": 53412
}
```

The response returns as soon as the port is bound, before the program has necessarily reached `listen()`. The address is real either way: the host bound it, so a connection will be accepted once the program starts accepting. Poll the status endpoint if you need to know it is ready.

```sh
curl http://127.0.0.1:53412
# {"ok":true}
```

Note that `listen()` takes no port. It does not need one, and a port passed to it is ignored: the socket already exists.

### Request fields

| Field | Type | Notes |
|---|---|---|
| `source` | string | Program source, as in `POST /exec` |
| `files` | object | Multi-file project: filename to content |
| `entry` | string | Entry filename, required with `files` |
| `wasm_path` | string | A `.wasm` in the session, as an alternative to source |
| `language` | string | `javascript` (default), `typescript` |
| `dependencies` | object | npm packages to vendor before the server starts |
| `lockfile` | object | A lockfile from a previous exec, replayed |
| `env` | object | Extra environment variables |
| `port` | number | A specific loopback port. Omit for an ephemeral one |

Exactly one of `source`, `files` or `wasm_path` is required.

Prefer an ephemeral port. It is the default, it cannot collide, and the caller reads the resolved port out of the response, so nothing has to agree on a number in advance.

## Checking on it

```http
GET /api/v1/sessions/{id}/serve
```

```json
{
  "server_id": "srv_0a1b2c3d4e5f",
  "addr": "127.0.0.1:53412",
  "port": 53412,
  "running": true,
  "uptime_ms": 8412,
  "stdout": "listening\n",
  "stderr": ""
}
```

`stdout` and `stderr` accumulate as the server runs, subject to the session's output cap, which is how you read a startup log or a crash. The entry survives the program ending, so a server that died is still reportable:

```json
{
  "server_id": "srv_0a1b2c3d4e5f",
  "addr": "127.0.0.1:53412",
  "port": 53412,
  "running": false,
  "uptime_ms": 240,
  "ended": "exited",
  "exit_code": 1,
  "stdout": "",
  "stderr": "Error: connect ECONNREFUSED\n"
}
```

`ended` is `exited` (the program returned or called `proc_exit`), `failed` (the execution itself failed) or `stopped` (you stopped it, or the session went away).

## Stopping it

```http
DELETE /api/v1/sessions/{id}/serve
```

```json
{ "message": "Server stopped for session abc123..." }
```

Stopping is cooperative, the same as an exec timeout: the flag is checked between instructions and inside the blocking socket calls, so a program parked in `accept` stops promptly. The call returns as soon as the flag is set rather than waiting for the program to notice, so a guest that ignores it cannot block your request.

A server also stops when its session is deleted or expires, and when the agent server shuts down. A bound port never outlives the process that bound it.

## Limits

**One server per session.** A second start returns `409 Conflict` while the first is still running. The wasmhub runtime gives a process one socket, so a second `listen()` inside the same program fails `EADDRINUSE` regardless; a limit the API states is better than one the program discovers. A server that has exited does not block a new one.

**A cap across sessions**, `--max-servers`, default 8. Exceeding it is a `429`. Servers are counted separately from `--max-concurrent-exec` on purpose: a server holds its thread, its stack and its port for as long as it runs, where an exec gives all three back in seconds, so a serving session can never exhaust the pool ordinary executions draw from.

**No fuel limit.** A fuel budget bounds how long an execution may run, and a server is supposed to run until stopped. The session's memory cap still applies, as do its output and disk caps.

**Loopback only.** The port is bound on `127.0.0.1` and is reachable by anything that can reach the host. What serves on it is code the caller supplied, so putting it in front of the internet is a reverse-proxy decision, the same one agent mode already asks you to make for [its own listener](../index.md).

## Networking from inside

A server gets its session's tenant network policy, like any other execution: it may well need to reach a database to answer a request. See [exec networking](../../exec/networking.md) for `--allow-net` and the `[tenants.network]` table.

Outbound HTTP from JavaScript is a separate matter. wasmhub's runtime implements `net` and `http` **servers** only; `net.connect`, `http.request` and `fetch` throw `ERR_NOT_SUPPORTED`, because WASI Preview 1 has no call that opens a socket and the runtime has not yet been built against wasmrun's extensions for it.

## How it works

WASI Preview 1 has no call that creates a socket, so a guest cannot ask for a port. The host binds one and hands the listening socket over as a file descriptor, the same way a preopened directory arrives, and the program calls `accept` on what it was given. A sandbox cannot request a port it should not have because it cannot request a port at all.

wasmhub's `net` and `http` find that descriptor through two environment variables wasmrun sets:

| Variable | Value |
|---|---|
| `WASMHUB_LISTEN_FD` | The descriptor number the listener landed on |
| `WASMHUB_LISTEN_ADDR` | The address it is bound to, for `server.address()` |

The fd is not a fixed number: a session preopens its working directory first, so the listener follows it. Read the variable rather than assuming a value.

For the command-line equivalent, see [`wasmrun exec --tcplisten`](../../exec/networking.md).
