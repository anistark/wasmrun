---
sidebar_position: 2
title: Running Projects
---

# Running Projects in OS Mode

## Basic Usage

Point OS mode at a project directory:

```sh
wasmrun os ./my-project
```

wasmrun will:
1. Detect the project language from files (`package.json`, `requirements.txt`, `Cargo.toml`, etc.)
2. Start an HTTP server with the OS mode UI
3. Fetch the appropriate language runtime from [wasmhub](https://github.com/anistark/wasmhub) (cached locally)
4. Serve project files to the browser
5. Boot a WASM VM in the browser that executes your project

The UI opens at `http://localhost:8420` by default.

## What Happens on Startup

```
$ wasmrun os examples/nodejs-http-api

🚀 Starting wasmrun in OS mode for project: examples/nodejs-http-api
✅ Multi-language kernel started
✅ OS mode templates loaded
🌐 OS Mode server listening on http://127.0.0.1:8420
🔌 Network proxy listening on ws://127.0.0.1:8440
✅ Project mounted at /nodejs-http-api -> examples/nodejs-http-api
✅ Project started with PID: 1
```

Behind the scenes:
1. **MultiLanguageKernel** initializes with a WASI filesystem, scheduler, and syscall handler
2. **Project directory** is mounted into the virtual filesystem
3. **Language runtime** is detected and loaded
4. **Network namespace** is created for the process (isolated port space)
5. **OS server** starts handling HTTP requests and serving the UI

## The Browser UI

Once the server starts, open `http://localhost:8420` in your browser. The UI has several panels:

### Console Panel
Live stdout/stderr output from your running project. Color-coded:
- 🟢 Green: stdout
- 🔴 Red: stderr
- 🔵 Blue: system messages

Includes Run/Stop controls and a clear button. Output appears as the program writes it, and **Stop** ends the program immediately, even inside a busy loop, because the VM runs in a worker the page can terminate.

### Filesystem Panel
Browse the WASI virtual filesystem. View project files as they exist inside the sandbox.

### Kernel Status Panel
Displays:
- Active processes and their state
- Memory usage
- Supported languages and WASI capabilities
- Filesystem mount points

### Logs Panel
Structured log trail from the kernel, server, and runtime. Filterable by source and severity.

## Project Requirements

OS mode requires a **directory** (not a single file):

```sh
# ✅ Directory
wasmrun os ./my-project

# ❌ Single file
wasmrun os ./script.py
# Error: OS mode requires a project directory, not a file
```

The directory must exist:

```sh
# ❌ Missing directory
wasmrun os ./nonexistent
# Error: Project path does not exist
```

## Stopping

Press `Ctrl+C` in the terminal, or from another terminal:

```sh
wasmrun stop
```

This gracefully shuts down the kernel, stops all processes, and cleans up network namespaces.

## What the VM can see

The VM's filesystem is populated from the project directory, minus what would never belong in a sandbox: `node_modules`, `target`, `.git`, `dist` and similar, and any `.wasm` file. Nothing else reaches it: no `npm install` runs, and there is no `node_modules` to find, so a `require()` of an npm package fails with `Cannot find module`. A project that runs in OS mode uses the runtime's built-in modules (`http`, `fs`, `path`, `URL`, `Buffer`) and its own files.

The project's files are readable and writable at `/` inside the VM, with `__dirname` at `/` for an entry at the top of the project. Writes stay in the browser and are gone when the program stops.

## Examples

### A Node.js HTTP API

[`examples/nodejs-http-api`](https://github.com/anistark/wasmrun/tree/main/examples/nodejs-http-api) is a REST API on the `http` module alone, with a test page served from `public/`:

```sh
wasmrun os examples/nodejs-http-api
```

Open the UI, switch to the Console panel and press **Run**. The host binds a port for the program before it starts and prints it, and the program's `listen()` picks it up:

```
Port bound for the program: http://127.0.0.1:3000
HTTP API listening on 127.0.0.1:3000
```

From then on the API is reachable from the host and from any browser tab:

```sh
curl http://127.0.0.1:3000/health
curl -X POST http://127.0.0.1:3000/api/todos \
  -H 'Content-Type: application/json' \
  -d '{"title":"from the host","userId":1}'
```

Each request is logged to the Console panel as it arrives, and **Stop** releases the port. See [Serving a Port](../port-forwarding.md) for how the port is chosen and handed in.

### Python

Python projects are detected (`requirements.txt`, `pyproject.toml`) and accepted by `--language python`, but the `rustpython` runtime is not yet published on wasmhub, so the VM has nothing to run them with. See [Language Selection](./language.md).

### Development Workflow

The page fetches the project's files each time you press **Run**, so the loop is: edit, **Stop**, **Run**. `--watch` is accepted but does not restart the VM on its own yet.

```sh
wasmrun os ./my-project --verbose
```

## See Also

- [Language Selection](./language.md): how auto-detection works, manual overrides
- [Server Options](./server-options.md): port, CORS, verbose, watch
- [Features](../features.md): REST API, runtime management, virtual filesystem
