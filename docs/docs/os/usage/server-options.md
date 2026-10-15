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
      --expose              Publish the program's port through a bore tunnel
      --tunnel-server <HOST[:PORT]>  Bore server [default: bore.pub]
      --tunnel-secret <SECRET>       Secret for a private bore server
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

```sh
wasmrun os ./my-project --watch
```

The server watches the project, and when a file changes the page restarts the program on the new files, which it fetches fresh. The Console shows `Project changed: restarting`. A program you have not run yet is left alone: the next **Run** gets the new files anyway.

Changes under `node_modules`, `target`, `dist`, `build`, `pkg` and hidden directories are ignored.

## Project settings: `wasmrun.toml`

A `wasmrun.toml` at the project root sets what the flags cannot, read once at startup. A file that does not parse, or an `[os]` key that is misspelled, stops the server before it starts rather than being ignored.

```toml
[os]
language = "nodejs"          # instead of detection; --language wins over it
entry = "src/server.js"      # the file the VM runs, instead of package.json main

[os.limits]
max_memory_mb = 256          # the VM's linear memory; 0 is no cap
max_output_mb = 10           # console output kept; the rest is dropped, with a note
timeout_secs = 0             # how long the program may run; 0 (the default) is until stopped

[os.network]
allow = ["api.example.com:443"]
```

| Limit | Default | When it is hit |
|---|---|---|
| `max_memory_mb` | 256 | The program is ended with `memory limit exceeded: N MB is past [os.limits] max_memory_mb = M`. It is checked at every WASI call, since the runtime owns its memory |
| `max_output_mb` | 10 | Output past it is dropped after one `[output limit reached]` line, so a runaway loop cannot fill the page |
| `timeout_secs` | 0 | The VM is stopped with `Stopped: the program ran past [os.limits] timeout_secs = N` |

The same units and the same `0`-means-unlimited rule as agent mode's limits. `[os.network]` is described in [Network Policy](../network-isolation.md).

## CORS Configuration

### `--allow-cors`

By default, the OS mode server restricts CORS to `http://127.0.0.1:<port>`; only the local browser UI can access the API.

With `--allow-cors`, the server sets `Access-Control-Allow-Origin: *`, allowing any origin to access the API:

```sh
wasmrun os --allow-cors
```

**When to use:**
- Accessing the API from a different port or domain
- Integrating with external tools or IDEs
- Development with separate frontend running on another port

**Security note:** Don't use `--allow-cors` in production or on shared networks. It allows any website to call your OS mode API.

## Public Tunnel

### `--expose`

Publish the port the program serves on through a [bore](https://github.com/ekzhang/bore) server. The public URL is printed at startup and shown in the Console panel:

```sh
wasmrun os ./my-project --expose
```

### `--tunnel-server <HOST[:PORT]>` and `--tunnel-secret <SECRET>`

Use a private bore server instead of `bore.pub`. The secret can come from `WASMRUN_TUNNEL_SECRET` instead, which keeps it out of shell history. Both need `--expose`.

```sh
WASMRUN_TUNNEL_SECRET=mysecret123 wasmrun os ./my-project --expose --tunnel-server tunnel.example.com
```

See [Public Tunneling](../public-tunneling.md) for the lifecycle, the API, and what is and is not safe to publish.

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
🔍 OS Mode: Analyzing project path: ./my-app
🏷️  Detected language: nodejs
✅ Multi-language kernel started
✅ OS mode templates loaded
📦 Loading nodejs runtime from cache (~/.wasmrun/runtimes/)
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

- [Running Projects](./running.md): startup flow and UI panels
- [Language Selection](./language.md): auto-detection and manual override
- [Network Isolation](../network-isolation.md): per-process network namespaces
- [Port Forwarding](../port-forwarding.md): expose sandbox services to the host
