---
sidebar_position: 4
title: Live Reload
description: Rebuild on change and reload the browser
---

# Live Reload

With `--watch`, wasmrun watches the project directory, rebuilds when source changes, and the page in the browser reloads itself once the new build is ready.

```sh
wasmrun ./my-project --watch
```

Without `--watch`, the server serves the build it started with until it is stopped.

## What a change does

| Changed | What happens |
|---|---|
| **Source**: `*.rs`, `*.go`, `*.c`, `*.cc`, `*.cpp`, `*.h`, `*.hpp`, `*.ts`, `*.js`, `*.mjs`, `*.py`, `*.toml`, `*.mod`, `Makefile`, `package.json`, `asconfig.json` | Rebuild, then reload |
| **Page and assets**: `*.html`, `*.css`, `*.json`, images, fonts | Reload, no build |
| **Anything under** `target/`, `node_modules/`, `pkg/`, `build/`, `dist/`, or a hidden file or directory | Nothing: these are build output, and watching them would rebuild on every build |

Changes are debounced by half a second, so saving several files at once builds once.

## How the page reloads

Every build that succeeds, and every page or asset change, moves a build *generation* forward. Pages poll for it once a second and reload when it changes:

- **A web app** reloads from the script the app port injects into its page, whether it is open in the control center's frame or in its own tab. The control center marks the reload in its Console panel.
- **A module** in the console page reloads the console.

The terminal shows the same events:

```
📂 src/lib.rs changed, rebuilding...
✅ Rebuilt in 206 ms
📂 index.html changed, reloading
```

## When a build fails

The page keeps running the last build that succeeded, and nothing reloads. The compiler's output is printed in the terminal and, for a web app, shown over the app in the control center, with the Build panel marked. Fix the error and save: the next successful build clears it and reloads.

## Limits

- `--watch` applies to project directories. For a `.wasm` passed directly there is nothing to rebuild, and the flag is ignored with a note.
- A reload starts the page over. There is no hot module replacement, so in-page state is lost.
- OS mode accepts `--watch` but does not act on it yet; see [OS Mode](../os/).

## See Also

- [Web Apps](./web-apps.md): the app port and the control center
- [run command](./usage/run.md): full `run` command reference
