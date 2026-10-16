---
sidebar_position: 4
title: Stability and Upgrading
description: What wasmrun 1.0 promises to keep stable, and what changed from 0.23
---

# Stability and Upgrading

From 1.0, wasmrun follows [Semantic Versioning](https://semver.org/). A patch release fixes bugs, a minor release adds things, and anything that breaks one of the surfaces below waits for 2.0.

## What is stable

| Surface | What that covers |
|---|---|
| **The CLI** | Every command and alias, every flag and its default, and exit codes. `wasmrun exec` exits with the program's own code |
| **Agent mode's REST API** | Routes, request fields, response fields and status codes. A minor release may add an optional request field or a response field, so a client should ignore fields it does not know |
| **Config files** | `wasmrun.toml` (`[os]`, `[os.limits]`, `[os.network]`), `processes.toml`, and the tenants file `wasmrun agent --auth` reads. Unknown keys are refused, so a file that uses a key added in a later minor is refused by an earlier wasmrun |
| **Machine-readable output** | `wasmrun ps list --json` and `ps status --json` |
| **What a guest sees** | WASI Preview 1 as implemented, the `sock_open` and `sock_connect` extensions with their arguments and errno values, and `WASMHUB_LISTEN_FD` / `WASMHUB_LISTEN_ADDR` |
| **The plugin ABI** | The C symbols an external plugin exports (`create_wasm_builder`, `can_handle_project`, `build`, `clean`, `clone_box`, `drop`, `free_build_result`), the `BuildConfigC`, `BuildResultC` and `PluginInfoC` layouts, and the plugin's `wasmrun.toml` manifest |

## What is not

- **Terminal output meant for people**: wording, layout, colors and emoji can change in any release. Scripts should read exit codes and `--json` output.
- **The HTTP endpoints behind wasmrun's own pages**: server mode's `/api/*` on the UI port and everything OS mode serves. They exist for the console, the control center and the OS page, and change with them.
- **The wasmhub release wasmrun is pinned to.** A minor release may move to a newer one, which brings newer language runtimes; a runtime cached from the old release is fetched again on first use.
- **Performance.** The [baseline](./exec/performance.md) is tracked to catch regressions, not promised.
- **`--debug` output**, and the Rust items inside the `wasmrun` crate, which is a binary with no library API.

## Upgrading from 0.23

Most projects need no changes. These are the differences a script or setup may notice.

### All modes

- **`wasmrun init` works**, with templates for every mode. See [init](./server/usage/init.md).
- **wasmrun runs wasmhub v0.6.0 runtimes.** The nodejs runtime is fetched again the first time it is needed.

### Exec mode

- **`wasmrun exec` exits with the program's exit code.** It used to exit 0 whatever the program returned, so a CI step that ran a failing program passed. If something depended on that, check the code explicitly.
- **stdout streams as it is written**, the way stderr already did, instead of all at once when the program ends.

### Server mode

- **Both ports bind `127.0.0.1`.** The UI used to listen on every interface, so another machine on the network could open it. To reach a dev session from elsewhere, forward the port, for example with `ssh -L 8420:127.0.0.1:8420`.
- **A web app runs on its own port.** A project with an `index.html`, or whose build emits JS glue, is served from the app port (the first free one from `8500`, or `--app-port`) and framed by the UI. It used to be handed to the module console. See [Web Apps](./server/web-apps.md).
- **`wasmrun stop` stops every running session**, with `SIGTERM`.
- **`wasmrun plugin install` refuses a name crates.io does not have.** It used to install a template plugin that built nothing; remove one left over from 0.23 with `wasmrun plugin uninstall <name>`.

### OS mode

- **The per-project server on port 8000 + PID, the `/app/` proxy, and `/api/processes/:pid/ports` and `/api/processes/:pid/forward` are gone.** Nothing used the server, and the port mappings were recorded but never honoured. A program's port is reached directly on the host through `bind_ports`, and published with `--expose`. See [Port Forwarding](./os/port-forwarding.md).
- **`--watch` restarts the program** when the project changes. It was accepted and did nothing.
- **`[os]` and `[os.limits]` in `wasmrun.toml` are read**, and a misspelled key in them stops the server, as one in `[os.network]` already did.
- **`examples/nodejs-express-api` is replaced by `examples/nodejs-http-api`**, which runs in the VM: it has no npm dependencies, and the VM has no `node_modules`.

### Agent mode

- **A tenant's stderr is no longer copied to the server's stderr.** It is in the response, as before; the server's stderr is its access log.
- **Responses can carry `warnings`**, for npm metadata wasmrun does not refuse. A client that rejects unknown fields needs to allow it.
