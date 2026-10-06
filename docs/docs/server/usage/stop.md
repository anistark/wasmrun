---
sidebar_position: 6
title: stop
---

# wasmrun stop

Stop every running wasmrun development server.

## Synopsis

```sh
wasmrun stop
```

**Aliases:** `kill`

## Description

Stops the dev sessions started by `wasmrun` or `wasmrun run`. Several can run at once, each on its own UI port, and `stop` stops all of them. Each one gets `SIGTERM` (`taskkill` on Windows), removes its own entry and exits.

`stop` only reaches server mode. An OS mode server (`wasmrun os`) or an agent server (`wasmrun agent`) stops with Ctrl+C in its terminal, or a signal to its process.

## Usage

```sh
wasmrun stop
```

Output when servers are running:

```
⏳ Stopping Wasmrun server...

  ✅ Wasmrun Server Stopped
  ✅ http://127.0.0.1:8420 (PID 18382), http://127.0.0.1:8421 (PID 18664)
```

Output when none is running:

```
  ℹ️  No Wasmrun server is currently running
```

## How It Works

Each dev session records itself when its UI port is bound: one file per session in `wasmrun/servers/` under the system temp directory (`$TMPDIR` on macOS and Linux), named by the session's PID and holding its UI URL. The file is removed when the session ends, by Ctrl+C or by `stop`.

A session that was killed outright (`kill -9`, a crash) leaves its file behind. `stop` ignores and removes any entry whose process is no longer a `wasmrun` or whose UI port no longer answers, so a PID the system has since handed to another program is never signalled.

## Examples

### Stop and Restart

```sh
wasmrun stop
wasmrun ./my-project --port 3000
```

### Port Conflict Resolution

A session whose port is taken moves to the next free one rather than failing. To get the port back:

```sh
wasmrun stop
wasmrun --port 8420
```

## See Also

- [run](./run.md): start the development server
- [clean](./clean.md): remove build artifacts
