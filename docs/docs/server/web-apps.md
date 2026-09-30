---
sidebar_position: 3
title: Web Apps
description: Run a WebAssembly web app from its own page, with a control center beside it
---

# Web Apps

A web app is a project that builds a page rather than a module: a Leptos or Yew app, a wasm-bindgen library with a page of its own, an AssemblyScript project with an `index.html`. Server mode runs it the way a browser would run it in production, from its own `index.html`, natively in the browser. Nothing is rendered on the server.

```sh
wasmrun ./examples/web-leptos
```

```
  🅦 Wasmrun  web-leptos (web_leptos_bg.wasm, 733.3 KB)

  🎛️  UI:  http://127.0.0.1:8420
  🌐 App: http://127.0.0.1:8421
```

Two ports, both on loopback:

| Port | Serves |
|---|---|
| **UI** (`--port`, default `8420`) | The control center: the app in a frame, with its console, requests, build and module beside it |
| **App** (`--app-port`, default the first free port above the UI port) | The app itself, exactly as it would be deployed. Open it in its own tab whenever you like |

## What counts as a web app

A project runs as a web app when either is true:

- it has an `index.html` at its root, or
- its build emitted JavaScript glue (wasm-bindgen's `pkg.js`, Emscripten's loader)

Everything else is a module, and the UI port shows the [module console](./usage/run.md#browser-ui) instead, as it always has. A `.wasm` passed directly is a module too, unless it is wasm-bindgen output (`name_bg.wasm` beside `name.js`).

## How the app port finds files

A page asks for its files by path, and the app port answers from three places in order:

1. **`/pkg/...` from the build output.** wasm-bindgen writes the glue and the `_bg.wasm` into wasmrun's build directory, and pages conventionally import them as `./pkg/name.js` (the `wasm-pack` layout). The fresh build always wins, so a stale `pkg/` left in the project from an earlier `wasm-pack` run never shadows it.
2. **The project directory.** `index.html`, stylesheets, images, and anything a build writes into the project itself (AssemblyScript's `build/`).
3. **The build output at the root**, for a page that imports `./name.js` directly.

Hidden files (`.env`, `.git/`) are never served, and nothing outside those two directories is reachable, through `..` or through a symlink.

### A wasm-bindgen library with no page

If the build emitted wasm-bindgen glue and the project has no `index.html`, the app port generates a small page that loads the glue, runs `init()` (and with it any `#[wasm_bindgen(start)]` function) and lists the exports. They are on `window.wasm`, so the browser console is a playground:

```js
wasm.greet('wasmrun')
```

## The control center

The UI port frames the app and watches it:

- **Console**: everything the app logs, its uncaught errors and unhandled rejections. The app port injects a small script into every page it serves, first in `<head>`, so logging from the earliest module is caught. The script only ever posts to the control center's own origin.
- **Requests**: every request the app made to the app port, with status, size and time. A `404` here is usually a path the page expects and the build does not produce.
- **Build**: builds, their durations, failures with the compiler's output, and file changes in watch mode.
- **Module**: exports, imports, sections and the plugin that built the module.
- **Metrics**: requests, bytes sent, 404s, errors, page load time, build count and the size of the `.wasm`.

The same data is available as JSON from the UI port:

| Endpoint | Returns |
|---|---|
| `GET /api/dev` | The project, both URLs, the current build (status, generation, error, duration, `.wasm` size) and request metrics |
| `GET /api/logs?since=N` | Build, file-change and request log entries after sequence `N`, and the `next` value to pass |
| `GET /api/module-info` | Static analysis of the current `.wasm` |

## Live reload

With `--watch`, a source change rebuilds and the app reloads itself; an `index.html` or stylesheet change reloads without building. A build that fails leaves the last good build running, shows the compiler's output over the app, and clears once a later build succeeds. See [Live Reload](./live-reload.md).

## Examples

```sh
# Leptos, wasm-bindgen glue imported from ./pkg/
wasmrun ./examples/web-leptos --watch

# AssemblyScript, bindings written to ./build/ by `asc --bindings esm`
wasmrun ./examples/web-asc

# A wasm-bindgen library with no page: wasmrun generates one
wasmrun ./examples/rust-hello

# Pick the app port
wasmrun ./examples/web-leptos --port 9000 --app-port 9100
```

## Limits

- **Loopback only.** Both ports bind `127.0.0.1`, because the app port serves the project directory. To reach the app from another device, put a reverse proxy in front of it.
- **No server-side code.** The app port is a static file server. A page that expects an API from its own backend needs that backend running separately.
- **One page reload per build.** There is no hot module replacement; the page reloads and its state starts over.
