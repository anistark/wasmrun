# Agent API Example Flows

Runnable end-to-end flows against the [Agent API](https://wasmrun.readthedocs.io/en/latest/docs/exec/agent) — the same request sequences an LLM agent makes through the `execute_code` tool schema (`GET /api/v1/tools`).

## Prerequisites

- `wasmrun agent` running locally (default port 8430):

  ```sh
  wasmrun agent
  ```

- `curl` and `jq` on your PATH.
- Network on first run: the JS runtime (and TS transpiler) are fetched from wasmhub once, then cached. The npm flow also talks to the npm registry.

## Flows

### `typescript-project.sh` — multi-file TypeScript project

Creates a session, executes a three-file TypeScript project in a single request (`files` + `entry` + `language: "typescript"`), and destroys the session. The `.ts` files are transpiled in-sandbox by the swc WASI transpiler; the emitted JavaScript resolves its imports through the runtime's own CommonJS `require()`.

```sh
./typescript-project.sh
# stdout: area=78.53981633974483
#         perimeter=31.41592653589793
```

### `npm-dependencies.sh` — npm dependency vendoring

Executes JavaScript that `require()`s a real npm package (`lodash`), declared with the `dependencies` field. wasmrun resolves and fetches the package host-side (the sandbox has no network), verifies its sha512 integrity, and vendors it into the session's `node_modules` — no `npm` binary involved, lifecycle scripts never run.

```sh
./npm-dependencies.sh
# stdout: pairs=[["a",1],["b",2]]
#         chunked=[[1,2],[3,4],[5]]
```

## Notes

- Everything a flow writes lives in the session's isolated filesystem and is deleted when the session is destroyed (or expires).
- Sandboxed code has **no network**: `fetch()` rejects with a clear `network access is not supported` error. Dependencies must come through the `dependencies` field or be shipped inline via `files`.
- Point the scripts at a remote server with `WASMRUN_AGENT_URL=http://host:port ./typescript-project.sh`.
