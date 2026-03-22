---
sidebar_position: 4
title: Server Options
---

# OS Mode Server Options

## All Options

```sh
wasmrun os [PROJECT] [OPTIONS]

Options:
  -p, --path <PATH>         Project directory path [default: ./]
  -P, --port <PORT>         Server port [default: 8420]
  -l, --language <LANG>     Force language (nodejs, python)
      --watch               Enable file watching and live reload
  -v, --verbose             Show detailed output
      --allow-cors          Allow cross-origin requests from any domain
```

## Port Configuration

### `-P, --port <PORT>`

Set the HTTP server port. Default is `8420`. Range: `1–65535`.

```sh
wasmrun os --port 3000
wasmrun os -P 8080
```

The OS mode UI, REST API, and all endpoints are served on this port:

```
http://localhost:3000/              → UI
http://localhost:3000/api/kernel/stats → Kernel stats API
http://localhost:3000/api/fs/list/  → Filesystem API
```

### Port Conflicts

If the port is in use:

```sh
wasmrun stop                  # stop any existing server
wasmrun os --port 8421        # or use a different port
```

## Watch Mode

### `--watch`

Monitor project files for changes and trigger reload.

```sh
wasmrun os ./my-project --watch
```

When files change:
- The file watcher detects modifications
- Project files are re-served to the browser
- The WASM runtime can be restarted with fresh files

Works well during active development for quick iteration.

## CORS Configuration

### `--allow-cors`

By default, the OS mode server restricts CORS to `http://127.0.0.1:<port>` — only the local browser UI can access the API.

With `--allow-cors`, the server sets `Access-Control-Allow-Origin: *`, allowing any origin to access the API:

```sh
wasmrun os --allow-cors
```

**When to use:**
- Accessing the API from a different port or domain
- Integrating with external tools or IDEs
- Development with separate frontend running on another port

**Security note:** Don't use `--allow-cors` in production or on shared networks. It allows any website to call your OS mode API.

## Verbose Output

### `-v, --verbose`

Show detailed startup and runtime information:

```sh
wasmrun os ./my-project --verbose
```

Verbose output includes:
- Project path analysis
- Language detection details
- Template loading progress
- API request logging
- Runtime fetch status

```
🔍 OS Mode: Analyzing project path: ./my-express-app
🏷️  Detected language: nodejs
✅ Multi-language kernel started
✅ OS mode templates loaded
📦 Loading QuickJS runtime from cache (~/.wasmrun/runtimes/quickjs.wasm)
🌐 OS Mode server listening on http://127.0.0.1:8420
✅ Project started with PID: 1
📝 Received request for: /
📝 Received request for: /api/kernel/stats
📝 Received request for: /api/project/files
```

## Combined Examples

### Full Development Setup

```sh
wasmrun os ./my-project \
  --port 3000 \
  --language nodejs \
  --watch \
  --verbose
```

### Minimal

```sh
wasmrun os
# Uses current directory, auto-detects language, port 8420
```

### External API Access

```sh
wasmrun os ./my-project --allow-cors --port 9000

# From another terminal or tool:
curl http://localhost:9000/api/kernel/stats
curl http://localhost:9000/api/fs/list/
```

### Multiple Projects

Run separate OS mode instances on different ports:

```sh
# Terminal 1
wasmrun os ./frontend --port 8420

# Terminal 2
wasmrun os ./backend --port 8421
```

## See Also

- [Running Projects](./running.md) — startup flow and UI panels
- [Language Selection](./language.md) — auto-detection and manual override
- [Network Isolation](../network-isolation.md) — per-process network namespaces
- [Port Forwarding](../port-forwarding.md) — expose sandbox services to the host
