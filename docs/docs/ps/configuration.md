---
sidebar_position: 2
title: Configuration
---

# processes.toml

A `processes.toml` lists the programs to supervise, one `[[process]]` table each. `wasmrun ps start` reads `./processes.toml` unless `--config` names another. Relative paths are relative to the file.

The whole file is checked before anything starts: a missing file, an unknown field, a bad address or a malformed network rule is reported by `ps start`, not discovered later as a program that keeps crashing.

```toml
[[process]]
name = "api"
script = "server.js"
cwd = "app"
args = ["--verbose"]
env = { NODE_ENV = "production" }
listen = "127.0.0.1:8080"
allow_net = ["api.example.com:443"]
restart = "on-failure"
max_memory_mb = 128
```

## What to run

Exactly one of these:

| Field | What |
|---|---|
| `wasm` | A WASI module to run with exec mode's interpreter |
| `script` | A JavaScript file to run on wasmhub's nodejs runtime, which is downloaded once and cached. The path is relative to `cwd`, and the program sees `cwd` as `/` |

## Fields

| Field | Default | What |
|---|---|---|
| `name` | required | Letters, digits, `-` and `_`. Names the program in every command and its log file |
| `args` | `[]` | Arguments after the program |
| `cwd` | the file's directory, for a `script` | A directory the program sees as `/`. A `wasm` program has none unless this is set |
| `dirs` | `[]` | More directories, as `HOST[:GUEST]`, the same as `wasmrun exec --dir` |
| `env` | `{}` | Environment variables. The program sees only these, not the supervisor's |
| `listen` | none | An address the host binds and hands to the program (`wasmrun exec --tcplisten`). `WASMHUB_LISTEN_FD` and `WASMHUB_LISTEN_ADDR` say where it is |
| `allow_net` | `[]` | Outbound destinations the program may reach (`wasmrun exec --allow-net`). Without any, it has no network |
| `restart` | `on-failure` | `always`, `on-failure` (a non-zero exit or a signal), or `never` |
| `max_restarts` | `10` | Restarts allowed within `restart_window_secs` before the program is marked `errored` |
| `restart_window_secs` | `60` | The window `max_restarts` counts within |
| `backoff_ms` | `500` | The delay before a restart. It doubles for each run in a row that lasted under 10 seconds |
| `max_backoff_ms` | `30000` | The most the delay grows to |
| `max_memory_mb` | `256` | The program's linear memory cap; `0` is no cap |
| `max_fuel` | `0` | WASM instructions before the program is stopped; `0` is no cap |
| `log_max_mb` | `10` | The log's size before it is rotated |
| `log_files` | `3` | Rotated logs kept beside the live one |
| `autostart` | `true` | Start with the supervisor. `false` registers it stopped, for `ps start <name>` |

## Restarts in practice

With the defaults, a program that exits with code 3 straight away is started again after 0.5 s, then 0.5 s, 1 s, 2 s, 4 s and so on, up to 30 s. After 10 restarts within a minute it is marked `errored`, with the reason in `ps status` and its log, and stays stopped until `wasmrun ps start <name>`, which also forgets the earlier failures.

A program that exits 0 under `on-failure` has finished its job: it is marked `exited` and not restarted. Under `always` it is restarted after the base delay.
