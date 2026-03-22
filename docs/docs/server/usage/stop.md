---
sidebar_position: 6
title: stop
---

# wasmrun stop

Stop any running wasmrun server instance.

## Synopsis

```sh
wasmrun stop
```

**Aliases:** `kill`

## Description

Gracefully terminates a running wasmrun development server. Sends SIGTERM to allow cleanup, connection closing, and temp file removal.

## Usage

```sh
# Stop the running server
wasmrun stop
```

Output when a server is running:

```
🛑 Stopping Wasmrun server...
   ✓ Server stopped (PID: 12345)
```

Output when no server is running:

```
ℹ️  No Wasmrun server is currently running
```

## How It Works

1. Reads the PID from `.wasmrun-server/pid`
2. Sends SIGTERM to the process
3. Waits for graceful shutdown
4. Removes the PID and port tracking files

## Examples

### Stop and Restart

```sh
wasmrun stop
wasmrun run ./my-project --port 3000
```

### Stop Before Switching Projects

```sh
wasmrun stop
cd ../other-project
wasmrun run
```

### Port Conflict Resolution

```sh
# "Port 8420 already in use"
wasmrun stop
wasmrun run --port 8420
```

### Force Stop

If `wasmrun stop` doesn't respond:

```sh
# Find the process
ps aux | grep wasmrun

# Force kill
pkill -9 wasmrun
```

## Server State Files

wasmrun tracks the server process in:

```
.wasmrun-server/
├── pid     # Process ID
└── port    # Port number
```

These are created on server start and cleaned up on stop.

## See Also

- [run](./run.md) — start the development server
- [clean](./clean.md) — remove build artifacts
