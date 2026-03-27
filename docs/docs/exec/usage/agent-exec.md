---
sidebar_position: 6
title: Agent Execution
---

# Agent WASM Execution

Execute a `.wasm` file within a session's sandbox.

## Execute WASM

```
POST /api/v1/sessions/:id/exec
```

**Request body:**
```json
{
  "wasm_path": "hello.wasm",
  "function": "_start",
  "args": ["arg1", "arg2"],
  "timeout": 30,
  "env": {
    "MY_VAR": "value"
  }
}
```

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `wasm_path` | yes | — | Path to `.wasm` file relative to session root |
| `function` | no | auto-detect | Exported function to call (defaults to `_start`, `main`, or start section) |
| `args` | no | `[]` | Arguments passed to the WASM program |
| `timeout` | no | `30` | Execution timeout in seconds |
| `env` | no | `{}` | Environment variables to set before execution |

**Response** (200):
```json
{
  "stdout": "Hello, World!\n",
  "stderr": "",
  "exit_code": 0,
  "duration_ms": 12
}
```

If execution fails (parse error, trap, etc.), the response still returns 200 with an `error` field:

```json
{
  "stdout": "",
  "stderr": "",
  "exit_code": -1,
  "duration_ms": 3,
  "error": "Failed to parse WASM module: invalid magic bytes"
}
```

## Timeout

If execution exceeds the timeout, the response includes:

```json
{
  "stdout": "partial output...",
  "stderr": "",
  "exit_code": -1,
  "duration_ms": 30000,
  "error": "Execution timed out after 30s"
}
```

## Multiple Executions

A session supports multiple sequential executions. Output buffers are cleared between each call — you always get only the output from the current execution.

```sh
# First exec
curl -X POST .../exec -d '{"wasm_path": "a.wasm"}'
# → {"stdout": "output from a", ...}

# Second exec (does NOT include output from a)
curl -X POST .../exec -d '{"wasm_path": "b.wasm"}'
# → {"stdout": "output from b", ...}
```

## Workflow

A typical agent workflow:

1. Create session
2. Write `.wasm` file via file upload endpoint
3. Execute it via `/exec`
4. Read the structured response
5. Optionally run more executions
6. Destroy session when done
