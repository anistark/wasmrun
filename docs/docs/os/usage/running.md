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
$ wasmrun os ./my-express-app

🚀 Starting wasmrun in OS mode for project: ./my-express-app
✅ Multi-language kernel started
✅ OS mode templates loaded
🌐 OS Mode server listening on http://127.0.0.1:8420
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
- 🟢 Green — stdout
- 🔴 Red — stderr
- 🔵 Blue — system messages

Includes Run/Stop controls and a clear button.

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

## Examples

### Node.js Express App

```sh
# Project structure:
# my-app/
# ├── package.json
# ├── server.js
# └── routes/
#     └── api.js

wasmrun os ./my-app
# Auto-detects Node.js from package.json
# Fetches QuickJS runtime from wasmhub
# Opens browser UI with console output
```

### Python Flask App

```sh
# Project structure:
# my-api/
# ├── requirements.txt
# └── app.py

wasmrun os ./my-api --language python
# Fetches RustPython runtime from wasmhub
```

### Development Workflow

```sh
# Start with watch mode for live reload
wasmrun os ./my-project --watch --verbose

# Make changes to source files
# Console panel shows reloaded output
```

## See Also

- [Language Selection](./language.md) — how auto-detection works, manual overrides
- [Server Options](./server-options.md) — port, CORS, verbose, watch
- [Features](../features.md) — REST API, runtime management, virtual filesystem
