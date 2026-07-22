---
sidebar_position: 6
title: Agent Execution
---

# Agent Execution

Run code inside a session's sandbox. The exec endpoint accepts four mutually exclusive input modes:

| Mode | Request field(s) | Use when |
|------|------------------|----------|
| Shell command | `command` | You want a familiar terminal-style one-liner over the session FS |
| JS/TS source | `source` + `language` | You have a single JavaScript or TypeScript snippet to evaluate |
| Multi-file JS/TS project | `files` + `entry` (+ `language`) | You have several source files that need to live on disk together |
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
| `stream` | no | `false` | Stream output as Server-Sent Events while the code runs (see [Streaming Output](#streaming-output)) |

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

Output buffers are cleared between calls; each response contains only the output of that invocation.

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

Unknown commands return exit code `127` with `command not found` on stderr; there is no fallback to the host shell.

**Example:**
```sh
curl -X POST .../exec -H "Content-Type: application/json" -d '{
  "command": "mkdir -p logs && echo started > logs/run.log && ls logs"
}'
# → {"stdout": "run.log\n", "stderr": "", "exit_code": 0, ...}
```

---

## JavaScript Source

Evaluate a single source string with the [wasmhub JavaScript runtime](https://anistark.github.io/wasmhub/runtimes/nodejs/). The runtime is fetched once and cached.

```json
{
  "source": "console.log(1 + 1)",
  "language": "javascript"
}
```

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `source` | yes | - | Source code to execute |
| `language` | no | `javascript` | One of `javascript`, `js`, `nodejs`, `typescript`, `ts`, `tsx` |
| `dependencies` | no | - | npm packages to vendor before execution (see [npm Dependencies](#npm-dependencies)) |

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

## TypeScript

Pass `language: "typescript"` (aliases `ts`, `tsx`) with either `source` or `files`. Before execution, wasmrun runs an swc-based transpiler (itself a WASI WASM module executing inside the sandbox) over the TypeScript inputs, then runs the emitted JavaScript with the usual runtime. No type *checking* is performed; types are stripped (like `tsc --transpileOnly` or esbuild).

```json
{
  "files": {
    "main.ts": "import {x} from './lib'; console.log(x)",
    "lib.ts": "export const x = 2"
  },
  "entry": "main.ts",
  "language": "typescript"
}
```

What the transpile stage does:

- **Types stripped**: interfaces, annotations, generics, `enum` lowering, decorators parsed
- **ES modules lowered to CommonJS**: `import`/`export` become `require()`/`exports`, resolved by the runtime's module system (the runtime does not execute `import` syntax directly); default-import interop helpers are inlined
- **TSX**: `.tsx` files (or `language: "tsx"` for a single `source` snippet) get JSX lowered to `React.createElement` calls; provide your own `React` implementation (e.g. vendored via `node_modules`)
- In `files` mode every `.ts`/`.tsx` file is transpiled in place to a sibling `.js`; other files (`.js`, `.json`, `node_modules/**`) pass through untouched, and a `.ts` entry runs as its emitted `.js`

Malformed TypeScript fails the request with `error: "TypeScript transpilation failed: <file>:<line>:<col>: <message>"` referencing the original `.ts` source. Runtime errors reference the emitted `.js` files (source maps are not yet applied).

---

## npm Dependencies

Declare npm packages with the `dependencies` field (works with `source` and `files`, JavaScript and TypeScript alike). The sandbox has no network, so wasmrun resolves and fetches them host-side from the npm registry, verifies each tarball's sha512 integrity, and lays them out in the session's `node_modules`, where the runtime's own `require()` finds them:

```json
{
  "source": "const _ = require('lodash'); console.log(_.chunk([1,2,3,4], 2));",
  "dependencies": { "lodash": "^4.17.21" }
}
```

**How it works:**

- No `npm` binary involved; wasmrun talks to the registry directly, and package lifecycle scripts are **never** executed
- Transitive (production) dependencies are installed npm2-style: each package's deps live in its own nested `node_modules`, deduped against ancestors exactly the way node resolves; always correct, at the cost of some duplication
- Downloads are cached per `name@version` under `~/.wasmrun/npm/`, so repeat runs skip the network; a dependency already present in the session at a satisfying version is skipped entirely
- Vendored files count against the session's disk and file-size limits
- The registry defaults to `https://registry.npmjs.org` and is configurable with `wasmrun agent --npm-registry <URL>` (private registries, mirrors)

**Supported version ranges:** the full npm range grammar. Exact (`4.17.21`), caret (`^4.17.21`), tilde (`~4.17.0`), comparators (`>=`, `>`, `<=`, `<`, `=`), x-ranges (`4`, `4.17`, `4.17.x`), `*`, dist-tags (`latest`), comparator sets (`>=1.2.0 <2.0.0`), unions (`^1 || ^2`), and hyphen ranges (`1.2.0 - 1.4.0`).

Prereleases follow the npm rule: `1.2.3-beta` only satisfies a range that names a prerelease of `1.2.3`, so `<2.0.0` never quietly pulls in `2.0.0-rc.1`.

**Limitations:**

- Pure-JS packages only: anything with an install script, `binding.gyp`, or prebuilt `.node` binaries is rejected with an error naming the package (native code can't run in the sandbox)
- CommonJS entry points work best; packages relying on ESM-only entry, `exports` maps, or `fetch`/network at import time may not load
- An uploaded `package.json` is inert unless you set `install_package_json`, so no execution turns into an unrequested network fetch. `devDependencies` are ignored: there is nothing in the sandbox to run them with yet

Malformed names/ranges fail with HTTP `400` before execution; resolution or download failures surface in the response's `error` field.

### Installing from package.json

Set `install_package_json` to read the `dependencies` of the project's `package.json`, either one uploaded in `files` or one already written to the session:

```json
{
  "files": {
    "package.json": "{\"dependencies\":{\"lodash\":\"^4.17.21\"}}",
    "main.js": "const _ = require('lodash'); console.log(_.chunk([1,2,3,4], 2));"
  },
  "entry": "main.js",
  "install_package_json": true
}
```

An explicit `dependencies` map still wins on conflict, so you can pin one package without rewriting the file.

### Reproducible installs with a lockfile

Version ranges resolve to whatever is newest at the time, so the same request can install different versions on different days. Every execution that installs anything returns a `lockfile` describing the resolved tree, keyed by install path:

```json
{
  "stdout": "[[1,2],[3,4]]\n",
  "exit_code": 0,
  "lockfile": {
    "node_modules/lodash": {
      "version": "4.17.21",
      "resolved": "https://registry.npmjs.org/lodash/-/lodash-4.17.21.tgz",
      "integrity": "sha512-..."
    }
  }
}
```

Send it back as the request's `lockfile` to install exactly those versions:

```json
{
  "source": "const _ = require('lodash'); console.log(_.VERSION);",
  "lockfile": { "node_modules/lodash": { "version": "4.17.21", "resolved": "...", "integrity": "..." } }
}
```

Replay walks the lockfile rather than the dependency graph, so nothing is re-resolved and no registry document is fetched. With a warm package cache the whole install runs offline. Tarballs are still integrity-verified and still scanned for native artifacts. Any `dependencies` not covered by the lockfile are resolved normally on top of it, so adding one package to a pinned tree does not unpin the rest.

A package skipped because an earlier execution in the same session already installed it is not in the lockfile: recording it would need the registry lookup the skip exists to avoid. Lock from a fresh session to get a complete tree.

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
| `files` | yes | - | Map of filename → file content. Filenames must be relative and free of `..` |
| `entry` | yes | - | Entry filename; must be a key in `files` |
| `language` | no | `javascript` | One of `javascript`, `js`, `nodejs`, `typescript`, `ts`, `tsx` |
| `dependencies` | no | - | npm packages to vendor before execution (see [npm Dependencies](#npm-dependencies)) |

Validation (missing entry, unknown language, absolute/traversal paths) runs synchronously and returns HTTP `400` immediately.

Files can load each other with CommonJS `require()`; the runtime resolves modules natively (see [JavaScript runtime capabilities](#javascript-runtime-capabilities)):

```json
{
  "files": {
    "main.js": "const {x} = require('./lib'); console.log(x);",
    "lib.js": "module.exports = {x: 2};"
  },
  "entry": "main.js"
}
```

Bare specifiers resolve through a `node_modules/<name>` tree, so a project can ship vendored pure-JS dependencies as part of `files` (e.g. `"node_modules/greet/index.js": "module.exports = ..."`).

---

## JavaScript Runtime Capabilities

JavaScript executes in the [wasmhub `nodejs` runtime](https://anistark.github.io/wasmhub/runtimes/nodejs/) (QuickJS-based, WASI; v0.3.2+), fetched once and cached. Supported surface:

**Module system (CommonJS):**
- Relative and absolute `require()` (`./x`, `../x`), with `.js`/`.json` extension probing, `index.*` resolution, and `package.json` `main`
- Bare `require('<name>')` resolved via `node_modules/<name>` walk-up
- Node-style module wrapper: `module.exports`, `require.cache`, `require.resolve`, `require.main`, `__filename`, `__dirname`
- JSON loading via `require('./data.json')`

**Built-in modules:** `path`, `fs`, `os`, `events`, `util`, `assert`, `stream`, `buffer`.

**Globals & event loop:** `process`, `setTimeout`/`setInterval`/`setImmediate`, `queueMicrotask`, `process.nextTick`, async/await with full Promise resolution (pending timers and microtasks are drained before exit), `Buffer`, `TextEncoder`/`TextDecoder`, `atob`/`btoa`.

**Web platform globals:**
- `URL` / `URLSearchParams`: WHATWG parsing, relative resolution against a base, `searchParams` kept in sync with the URL
- `crypto.getRandomValues` / `crypto.randomUUID`: entropy comes from the WASI `random_get` syscall
- `structuredClone`: deep clone with cycles, `Map`/`Set`/`Date`/`RegExp`/`ArrayBuffer`/TypedArrays; functions and symbols throw `DataCloneError`, matching the spec
- `fetch` is **defined but always rejects** with a clear `network access is not supported` error (`code: 'ERR_NETWORK_UNSUPPORTED'`): sandboxed code has no sockets yet, and a documented rejection beats a bare `ReferenceError`

**Not yet available:**
- Network I/O: `fetch` and sockets are deferred to the wasmnet milestone (see above for the interim `fetch` behavior)
- Native extensions / C addons: pure JS only
- npm installation from inside the sandbox: there is no package manager in it; declare packages with the [`dependencies` field](#npm-dependencies) (vendored host-side) or ship a `node_modules` tree via `files`

---

## TypeScript Project Configuration

A project may ship a `tsconfig.json`, uploaded in `files` or already in the session. It is parsed as JSONC, so comments and trailing commas are fine.

**`paths` aliases are honored.** Each alias is materialized under `node_modules` as a small CommonJS shim, so the runtime's own resolver handles it with no rewriting of your imports:

```json
{
  "files": {
    "tsconfig.json": "{\"compilerOptions\":{\"paths\":{\"@app/*\":[\"src/*\"]}}}",
    "src/util.ts": "export const greet = (n: string) => `hi ${n}`;",
    "main.ts": "import { greet } from '@app/util'; console.log(greet('there'));"
  },
  "entry": "main.ts",
  "language": "typescript"
}
```

`baseUrl` is applied to alias targets. Both sides of an alias must agree on whether they use `*`, and an alias pointing outside the project fails with a `400` rather than a confusing require error at runtime.

**Options that are refused.** These would silently produce broken output, so they fail the request by name instead:

| Option | Why |
|--------|-----|
| `experimentalDecorators` | The transpiler has no decorator transform, so decorators would be emitted as-is and the runtime cannot execute them |
| `emitDecoratorMetadata` | Same |
| `jsx` other than `"react"` | Only the classic runtime is available; JSX compiles to `React.createElement` calls |

**`target` is accepted and ignored.** There is no down-level pass, so the source syntax reaches the runtime either way. It is a no-op rather than a wrong answer.

**Types are stripped, not checked.** A project with type errors still runs. Run `tsc` yourself if you need type checking.

---

## Streaming Output

By default a response arrives only when execution finishes, which makes a long run silent until the end. Set `stream` to receive output as it happens, as [Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events):

```bash
curl -N -X POST http://127.0.0.1:8420/api/v1/sessions/$SID/exec \
  -H 'Content-Type: application/json' \
  -d '{"source": "for (let i = 0; i < 3; i++) console.log(i);", "stream": true}'
```

```
event: output
data: {"stdout":"0\n1\n","stderr":""}

event: output
data: {"stdout":"2\n","stderr":""}

event: result
data: {"stdout":"0\n1\n2\n","stderr":"","exit_code":0,"duration_ms":412}
```

- `output` events carry only what is new since the previous event. They are best-effort: a fast program may finish before the first sample and produce none at all
- The final `result` event carries the same object the buffered response would have returned, including the complete output, so a client can ignore the intermediate events entirely
- Errors that prevent execution from starting (unknown session, bad request) are returned as an ordinary JSON error response, not as an event stream
- Disconnecting cancels the execution, so an abandoned run does not keep burning a worker

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
| `wasm_path` | yes | - | Path to `.wasm` file relative to session root |
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
| 400 | Bad request: no input mode given, unknown language, invalid `files`/`entry`, or path traversal |
| 401 | Auth enabled (`--auth`) but the API key is missing, malformed, or unknown |
| 404 | Session not found, or owned by another tenant (cross-tenant access is hidden as 404) |
| 410 | Session expired |
| 413 | Request body exceeded `--max-body` |
| 429 | `--max-concurrent-exec` reached; too many executions already in flight; retry after backoff |

A 429 is returned immediately (no thread is spawned). The exec slot it was waiting on is freed only when an in-flight execution actually completes, so a long-running or timed-out execution continues to hold its slot until it is cancelled.

## Workflow

A typical agent loop:

1. Create a session.
2. Either upload files explicitly (file endpoints) or pass them inline via `files`/`source`/`command`.
3. Execute via `/exec`.
4. Read the structured response.
5. Optionally run more executions in the same session; the filesystem and exported env vars persist.
6. Destroy the session when done.
