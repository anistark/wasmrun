# Agent API Example Flows

Runnable end-to-end flows against the [Agent API](https://wasmrun.readthedocs.io/en/latest/docs/agent): the same request sequences an LLM agent makes through the `execute_code` tool schema (`GET /api/v1/tools`).

## Prerequisites

- `wasmrun agent` running locally (default port 8430):

  ```sh
  wasmrun agent
  ```

- `curl` and `jq` on your PATH.
- Network on first run: the JS runtime (and TS transpiler) are fetched from wasmhub once, then cached. The npm and test flows also talk to the npm registry.

## Flows

### `typescript-project.sh`: multi-file TypeScript project

Creates a session, executes a three-file TypeScript project in a single request (`files` + `entry` + `language: "typescript"`), and destroys the session. The `.ts` files are transpiled in-sandbox by the swc WASI transpiler; the emitted JavaScript resolves its imports through the runtime's own CommonJS `require()`.

```sh
./typescript-project.sh
# stdout: area=78.53981633974483
#         perimeter=31.41592653589793
```

### `npm-dependencies.sh`: npm dependency vendoring

Executes JavaScript that `require()`s a real npm package (`lodash`), declared with the `dependencies` field. wasmrun resolves and fetches the package host-side (the sandbox has no network), verifies its sha512 integrity, and vendors it into the session's `node_modules`; no `npm` binary involved, lifecycle scripts never run.

```sh
./npm-dependencies.sh
# stdout: pairs=[["a",1],["b",2]]
#         chunked=[[1,2],[3,4],[5]]
```

### `typescript-tests.sh`: a TypeScript project's test suite

Runs a project the way an agent actually would: a tsconfig (`target`, and a `@app/*` path alias), an ESM-only npm dependency (`escape-string-regexp`, converted to CommonJS on the way in), and its tests under the runtime's built-in `node:test`. One test passes and one fails on purpose.

```sh
./typescript-tests.sh
# stdout: TAP version 13
#         ok 1 - treats metacharacters literally
#         not ok 2 - counts every occurrence
#           error: '1 == 2'
#           stack: |-
#                 at <anonymous> (tests/search.test.ts:11)
#         # pass 1
#         # fail 1
# exit_code: 1
```

Two things worth noticing in that output. The frame names `tests/search.test.ts:11`, the line the assertion is actually on, rather than the JavaScript the transpiler emitted: stack frames are remapped through the source map before the response. And `exit_code` is `1`, so an agent can tell a failed suite from a passing one without parsing TAP at all.

## Notes

- Everything a flow writes lives in the session's isolated filesystem and is deleted when the session is destroyed (or expires).
- Sandboxed code has **no network**: `fetch()` rejects with a clear `network access is not supported` error. Dependencies must come through the `dependencies` field or be shipped inline via `files`.
- Point the scripts at a remote server with `WASMRUN_AGENT_URL=http://host:port ./typescript-project.sh`.
