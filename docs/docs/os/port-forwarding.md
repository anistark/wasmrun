---
sidebar_position: 6
title: Serving a Port
---

# Serving a Port

A program in a sandbox that binds a port and answers requests on it. Where this works depends on which mode is running the program, and OS mode is the one where it does not work yet.

## OS mode: not yet

There is no `--forward` flag and no port forwarding in OS mode. A project running in the browser VM cannot bind a port, so there is nothing to forward.

This page previously documented a `--forward HOST_PORT:PROCESS_PORT` flag with examples for Express, Flask and PostgreSQL. None of it was implemented; the flag has never existed.

What is missing is the browser socket bridge, described in [Network Policy](./network-isolation.md). The proxy that would carry those sockets is running, and the WASI shim in the browser does not call it yet. Tracked in [wasmrun#99](https://github.com/anistark/wasmrun/issues/99).

## What works today

Both of these run the program on wasmrun's own interpreter rather than in a browser, which is why sockets are available there and not here.

### Agent mode: a server in a session

`POST /sessions/:id/serve` starts a long-lived program and binds a loopback port for it. The response carries the port, so nothing has to agree on a number in advance:

```sh
curl -X POST http://localhost:8430/api/v1/sessions/$SESSION/serve \
  -H 'Content-Type: application/json' \
  -d '{
    "source": "const http = require(\"http\"); http.createServer((req, res) => res.end(\"hello\")).listen();",
    "language": "javascript"
  }'
```

```json
{ "server_id": "srv_0a1b2c3d4e5f", "addr": "127.0.0.1:53412", "port": 53412 }
```

```sh
curl http://127.0.0.1:53412
# hello
```

See [Serving from a session](../agent/usage/serving.md) for the full lifecycle.

### Exec mode: a bound port on the command line

`wasmrun exec --tcplisten` binds a port on the host and hands the listening socket to the program:

```sh
wasmrun exec --tcplisten 127.0.0.1:8080 ./server.wasm
```

See [exec networking](../exec/networking.md).

## Why the host binds the port

In both cases the *host* binds and the program is handed the result. This is not a limitation to work around, it is the design.

WASI Preview 1 has no call that creates a socket. A guest cannot ask for a port, which means it cannot ask for one it should not have: a listener arrives the way a preopened directory does, already decided. A program that wants to serve calls `accept` on what it was given.

wasmhub's `net` and `http` modules read the descriptor number from `WASMHUB_LISTEN_FD` and the address from `WASMHUB_LISTEN_ADDR`, which is what makes an ordinary `server.listen()` and `server.address()` work in JavaScript without the program knowing any of this happened.

## See also

- [Network Policy](./network-isolation.md): what a sandbox may connect out to
- [Serving from a session](../agent/usage/serving.md): the agent-mode lifecycle
- [Exec networking](../exec/networking.md): `--tcplisten` and `--allow-net`
