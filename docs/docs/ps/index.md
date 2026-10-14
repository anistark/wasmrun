---
sidebar_position: 1
title: Overview
---

# Process Manager

`wasmrun ps` keeps WebAssembly programs running. A supervisor starts the programs listed in a `processes.toml`, restarts them when they end the way their policy says, and keeps a log per program, so a WASI service runs the way a Node service runs under PM2 or a binary under systemd.

```sh
wasmrun ps start                 # start the programs in ./processes.toml
wasmrun ps list                  # what is running
wasmrun ps logs api -f           # follow one program's log
wasmrun ps restart api
wasmrun ps stop                  # stop everything, and the supervisor
```

## How it works

- **One supervisor per user.** `wasmrun ps start` runs it in the background and returns; `--foreground` keeps it in the terminal, which is what a container or a systemd unit wants. Later `ps start --config other.toml` calls add programs to the running supervisor.
- **Each program is its own process.** The supervisor runs it as `wasmrun exec`, so it gets exec mode's interpreter and WASI, and a program that crashes the interpreter takes down only itself. Its memory can be read and it can be stopped like any other process.
- **Restarts follow a policy** (`always`, `on-failure`, `never`). A program that keeps failing is restarted with a doubling delay, and one that fails too often within a window is marked `errored` and left alone until `ps start` asks for it again.
- **Logs are files.** Each program's stdout and stderr go line by line to `~/.wasmrun/ps/logs/<name>.log`, with the supervisor's own notes (started, exited, restarting) marked `[wasmrun ps ...]`, and the file is rotated by size.
- **Limits are agent mode's.** `max_memory_mb` (256 by default) and `max_fuel` cap a program the way they cap an agent execution.

## A first program

```toml
# processes.toml
[[process]]
name = "api"
script = "server.js"            # JavaScript, on wasmhub's nodejs runtime
listen = "127.0.0.1:8080"       # the host binds the port and hands it over

[[process]]
name = "worker"
wasm = "target/wasm32-wasip1/release/worker.wasm"
restart = "always"
```

```sh
$ wasmrun ps start
NAME                 STATUS        PID    UPTIME RESTARTS    MEMORY  PROGRAM
api                  running      4312        0s        0   20.4 MB  server.js (nodejs)
worker               running      4313        0s        0    6.1 MB  /srv/app/target/wasm32-wasip1/release/worker.wasm
```

A WASI program cannot open a listening socket itself (Preview 1 has no call for it), so a server names the address in `listen` and the host binds it, the same way `wasmrun exec --tcplisten` works. The program finds it through `WASMHUB_LISTEN_FD`, which wasmhub's runtimes read; a Rust program can take fd 3, or the fd after any `dirs` it was given.

- [Configuration](./configuration.md): every `processes.toml` field
- [Commands](./commands.md): `start`, `stop`, `restart`, `list`, `status`, `logs`

## Where its state lives

`~/.wasmrun/ps/` (or `WASMRUN_PS_HOME`):

| File | What |
|---|---|
| `supervisor.json` | The running supervisor's PID, control port and token. Readable by you only; every control request carries the token |
| `supervisor.log` | The supervisor's own output when it runs in the background |
| `logs/<name>.log` | Each program's log, with `<name>.log.1` and so on after rotation |

The supervisor's control API listens on a loopback port only. It does not survive a reboot on its own: run `wasmrun ps start --foreground` from a systemd unit or a container's command to have it started at boot.
