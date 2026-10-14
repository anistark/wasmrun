---
sidebar_position: 3
title: Commands
---

# Commands

## ps start

```sh
wasmrun ps start [--config FILE] [--foreground]
wasmrun ps start NAME
```

Starts a supervisor for the programs in `FILE` (default `./processes.toml`) in the background, waits for it, and lists what it runs. If a supervisor is already running, the file's programs are added to it; a name it already supervises is refused, and nothing from that file is added.

`--foreground` runs the supervisor in this terminal until Ctrl+C or SIGTERM, then stops every program. Use it under systemd or as a container's command.

With a `NAME`, starts one stopped, exited or errored program again.

## ps stop

```sh
wasmrun ps stop NAME     # one program; the supervisor keeps running
wasmrun ps stop          # every program, then the supervisor
```

A program gets SIGTERM and five seconds to exit before it is killed.

## ps restart

```sh
wasmrun ps restart NAME
```

Stops the program and starts it straight away, without a backoff and without counting towards `max_restarts`.

## ps list

```sh
wasmrun ps list [--json]      # alias: ps ls
```

```text
NAME                 STATUS        PID    UPTIME RESTARTS    MEMORY  PROGRAM
api                  running      3582        3s        2   21.8 MB  api.js (nodejs)
crasher              errored         -         -        3         -  crash.js (nodejs)
once                 exited          -         -        0         -  once.js (nodejs)
```

A status is one of `starting`, `running`, `waiting` (between a restart's exit and its start), `stopped`, `exited` (ended, and its policy says not to restart) and `errored` (restarted too often, or could not start). Memory is the process's resident memory.

## ps status

```sh
wasmrun ps status NAME [--json]
```

```text
crasher
  status    errored: restarted 3 times within 60s; not restarting again (ps start crasher to try once more)
  program   crash.js (nodejs)
  restart   on-failure (3 so far)
  log       /home/me/.wasmrun/ps/logs/crasher.log
  exits (oldest first)
    2026-10-14T11:06:34.893985+00:00  code 3 after 1.5s
    2026-10-14T11:06:36.510953+00:00  code 3 after 1.4s
```

The last ten exits are kept, each with its code or signal and how long the run lasted. `--json` gives the same fields for scripts.

## ps logs

```sh
wasmrun ps logs NAME [-n LINES] [-f]
```

Prints the last `LINES` (default 50) of the program's log, then with `-f` keeps printing as it grows, across rotations, until Ctrl+C. It reads the file directly, so it works after the supervisor has stopped too.
