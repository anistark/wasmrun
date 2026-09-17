---
sidebar_position: 6
title: Serving a Port
---

# Serving a Port

A program in a sandbox that binds a port and answers requests on it. All three modes can do this now, and in all three the host binds the port and hands it to the program.

## OS mode

When a project starts in the browser VM, the page asks the network proxy for a port from the policy's `bind_ports` range (`3000-9999` by default, see [Network Policy](./network-isolation.md)), taking the first one that is free. The address is printed in the Console panel:

```
Port bound for the program: http://127.0.0.1:3000
```

A Node project then serves on it with nothing special in the code:

```js
const http = require('http')
http.createServer((req, res) => res.end('hello from the browser VM\n')).listen()
```

```sh
curl http://127.0.0.1:3000
# hello from the browser VM
```

`server.listen()` needs no port, and passing one is ignored: the socket already exists, and `server.address()` reports the real one. Requests reach the VM through the proxy's WebSocket, and the program reads them from a shared buffer between the page and the VM worker, so a busy program and an idle one both see their connections without the page having to wait for either.

Stopping the program releases the port. One port is bound per running project, so a second `wasmrun os` on the same machine gets the next free one in the range.

There is no `--forward` flag. This page once documented one, with examples for Express, Flask and PostgreSQL; it never existed, and it is not needed: the port the program serves on is already a host port.

## The other two modes

Both run the program on wasmrun's own interpreter rather than in a browser.

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

In every mode the *host* binds and the program is handed the result. This is not a limitation to work around, it is the design.

WASI Preview 1 has no call that creates a socket. A guest cannot ask for a port, which means it cannot ask for one it should not have: a listener arrives the way a preopened directory does, already decided. A program that wants to serve calls `accept` on what it was given.

wasmhub's `net` and `http` modules read the descriptor number from `WASMHUB_LISTEN_FD` and the address from `WASMHUB_LISTEN_ADDR`, which is what makes an ordinary `server.listen()` and `server.address()` work in JavaScript without the program knowing any of this happened.

## See also

- [Network Policy](./network-isolation.md): what a sandbox may connect out to
- [Serving from a session](../agent/usage/serving.md): the agent-mode lifecycle
- [Exec networking](../exec/networking.md): `--tcplisten` and `--allow-net`
