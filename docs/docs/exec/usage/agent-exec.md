---
sidebar_position: 6
title: Agent Execution
---

# Agent Execution

Run code inside a session's sandbox. The exec endpoint accepts four mutually exclusive input modes:

| Mode | Request field(s) | Use when |
|------|------------------|----------|
| Shell command | `command` | You want a familiar terminal-style one-liner over the session FS |
| JavaScript source | `source` + `language` | You have a single JS snippet to evaluate |
| Multi-file JS project | `files` + `entry` (+ `language`) | You have several source files that need to live on disk together |
| Pre-compiled WASM | `wasm_path` (+ `function`, `args`) | You already have a `.wasm` file in the session FS |

If more than one is provided, dispatch follows that priority order (`command` → `files` → `source` → `wasm_path`).

```
POST /api/v1/sessions/:id/exec
```

## Common Fields

These apply to every mode:

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `timeout` | no | `30` | Execution timeout in seconds |
| `env` | no | `{}` | Environment variables to set before execution |

## Common Response

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

Output buffers are cleared between calls — each response contains only the output of that invocation.

---

## Shell Command

Run a built-in shell command line against the session filesystem. No language runtime, no WASM module, no subprocess.

```json
{
  "command": "echo hello > out.txt && cat out.txt"
}
```

**Supported built-ins:** `echo`, `cat`, `ls`, `pwd`, `cd`, `mkdir` (`-p`), `rm` (`-r`/`-rf`), `cp`, `mv`, `env`, `export`.

**Operators:** pipes (`|`), redirection (`>`, `>>`, `<`), sequencing (`&&` short-circuit on failure, `;` always continue), single and double quoted strings.

**Scope:**
- CWD starts at `/` for every request. `cd` mutates the in-invocation CWD, so chains like `cd sub && pwd && ls` work, but the CWD is not carried across requests.
- `export KEY=value` writes through to the session's environment, so the variable is visible to subsequent `command`/`source`/`files`/`wasm_path` executions in the same session.
- Path traversal that escapes the session root is rejected.

Unknown commands return exit code `127` with `command not found` on stderr — there is no fallback to the host shell.

**Example:**
```sh
curl -X POST .../exec -H "Content-Type: application/json" -d '{
  "command": "mkdir -p logs && echo started > logs/run.log && ls logs"
}'
# → {"stdout": "run.log\n", "stderr": "", "exit_code": 0, ...}
```

---

## JavaScript Source

Evaluate a single source string with the wasmhub JavaScript runtime. The runtime is fetched once and cached.

```json
{
  "source": "console.log(1 + 1)",
  "language": "javascript"
}
```

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `source` | yes | — | Source code to execute |
| `language` | no | `javascript` | One of `javascript`, `js`, `nodejs` |

Unsupported languages return HTTP `400` with a clear message before any thread is spawned.

**Example:**
```sh
curl -X POST .../exec -H "Content-Type: application/json" -d '{
  "source": "console.log(1+1)",
  "language": "javascript"
}'
# → {"stdout": "2\n", "exit_code": 0, ...}
```

---

## Multi-file JavaScript Project

Upload an entire project in one request. All files are written to the session root (creating intermediate directories) and the entry file is run.

```json
{
  "files": {
    "main.js": "console.log('hi');",
    "lib/util.js": "exports.greet = n => 'hi ' + n;"
  },
  "entry": "main.js",
  "language": "javascript"
}
```

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `files` | yes | — | Map of filename → file content. Filenames must be relative and free of `..` |
| `entry` | yes | — | Entry filename; must be a key in `files` |
| `language` | no | `javascript` | One of `javascript`, `js`, `nodejs` |

Validation (missing entry, unknown language, absolute/traversal paths) runs synchronously and returns HTTP `400` immediately.

Sibling files are visible to the runtime via the preopened WASI directory, but loading them from JS requires runtime-side support. The wasmhub `nodejs` runtime (v0.2.0) does not yet implement CommonJS `require()` — files can be uploaded today and will become loadable once the runtime ships that.

---

## Pre-compiled WASM

Execute a `.wasm` file already present in the session filesystem.

```json
{
  "wasm_path": "hello.wasm",
  "function": "_start",
  "args": ["arg1", "arg2"]
}
```

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `wasm_path` | yes | — | Path to `.wasm` file relative to session root |
| `function` | no | auto-detect | Exported function to call (defaults to `_start`, `main`, or start section) |
| `args` | no | `[]` | Arguments passed to the WASM program |

---

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

Partial output captured before the timeout is still returned. The worker is then cooperatively cancelled so it stops executing and frees its resources, even under the default (unlimited) fuel budget.

## Limits & Errors

Execution is bounded by the per-session resource limits and server-wide ingress guards configured on [`wasmrun agent`](../agent.md#starting-the-server) (and overridable per session). These surface in two ways.

**Within a 200 response** (the execution ran but hit a soft cap):

| Field / value | Meaning |
|---------------|---------|
| `error` contains `"instruction limit"` | The execution exceeded `--max-fuel` and was aborted (`exit_code: -1`) |
| `output_truncated: true` | Captured stdout+stderr hit `--max-output`; output in the response is truncated (the field is omitted when `false`) |
| `error` contains `"File size limit"` / `"Disk usage limit"` | A file write from the sandbox exceeded `--max-file-size` or the session's `--max-disk` quota |

**As an HTTP error status** (the request was rejected before or instead of running):

| Status | When |
|--------|------|
| 400 | Bad request — no input mode given, unknown language, invalid `files`/`entry`, or path traversal |
| 404 | Session not found |
| 410 | Session expired |
| 413 | Request body exceeded `--max-body` |
| 429 | `--max-concurrent-exec` reached — too many executions already in flight; retry after backoff |

A 429 is returned immediately (no thread is spawned). The exec slot it was waiting on is freed only when an in-flight execution actually completes, so a long-running or timed-out execution continues to hold its slot until it is cancelled.

## Workflow

A typical agent loop:

1. Create a session.
2. Either upload files explicitly (file endpoints) or pass them inline via `files`/`source`/`command`.
3. Execute via `/exec`.
4. Read the structured response.
5. Optionally run more executions in the same session — the filesystem and exported env vars persist.
6. Destroy the session when done.
