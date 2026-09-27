---
sidebar_position: 7
title: Public Tunneling
---

# Public Tunneling

`--expose` puts the port a program in the browser VM serves on onto the internet, through a [bore](https://github.com/ekzhang/bore) server. The public server `bore.pub` is the default; a private one works the same way.

```sh
wasmrun os examples/nodejs-http-api --expose
```

```
🌍 Opening a public tunnel through bore.pub:7835…
🌍 Public URL: http://bore.pub:41234
   It forwards to the program's port once the program listens
```

Press **Run** in the Console panel. Once the program has its port, the Console says where it is reachable:

```
Port bound for the program: http://127.0.0.1:3000
Public URL: http://bore.pub:41234 → 127.0.0.1:3000
```

```sh
curl http://bore.pub:41234/health
```

The Console panel also shows both addresses above the output, with the tunnel's state and a button to copy the public URL.

## What gets published

The **program's port**, and only that: the one the page binds for it from the policy's `bind_ports` range (see [Serving a Port](./port-forwarding.md)). The OS mode server, its APIs, and the network proxy are never published, even though both sit inside the default `3000-9999` range; the server refuses to point the tunnel at either.

## Lifecycle

- **The tunnel opens when `wasmrun os` starts** and holds one public port for the whole session.
- **Run points it at the program.** Until then, and after **Stop**, a visitor is connected and closed straight away rather than left waiting.
- **The URL survives Stop and Run.** The public port belongs to the tunnel, not to the program, so a restarted program is reachable at the same address.
- **A dropped link reconnects on its own**, retrying with backoff up to 30 seconds apart, and asks for the public port it had. It gets it back unless someone else took it in the meantime.
- **The tunnel closes when `wasmrun os` exits**, which releases the public port on the server.

## Options

| Flag | Default | What it does |
|---|---|---|
| `--expose` | off | Open the tunnel |
| `--tunnel-server <HOST[:PORT]>` | `bore.pub` | The bore server. The port is its control port, `7835` unless given. IPv6 goes in brackets: `[::1]:7835` |
| `--tunnel-secret <SECRET>` | none | Secret for a server started with `bore server --secret`. `WASMRUN_TUNNEL_SECRET` works too, and keeps the secret out of shell history and the process list |

`--tunnel-server` and `--tunnel-secret` need `--expose`.

## A private bore server

```sh
# On a host the internet can reach
cargo install bore-cli
bore server --secret mysecret123
```

```sh
WASMRUN_TUNNEL_SECRET=mysecret123 \
  wasmrun os ./my-app --expose --tunnel-server tunnel.example.com
```

The server needs its control port (`7835`) reachable, and the range it hands out public ports from (`--min-port`/`--max-port`, `1024-65535` by default). Visitors connect to `tunnel.example.com:<public port>` directly.

A secret is checked on every connection the client makes: the control connection, and the one it opens for each visitor. A wrong secret shows up as a `Failed` tunnel with the server's reason, and so does a server that wants a secret when none was given.

## REST API

The UI drives these; they are documented for scripting.

| Endpoint | Method | What it does |
|---|---|---|
| `/api/tunnel/status` | GET | State, public URL, and where it forwards |
| `/api/tunnel/target` | POST | Point the tunnel at the program's port, `{"port": 3000}`, or at nothing, `{"port": null}` |
| `/api/tunnel/stop` | POST | Close the tunnel |
| `/api/tunnel/start` | POST | Open it again after a stop |

```json
{
  "success": true,
  "enabled": true,
  "status": "Connected",
  "server": "bore.pub:7835",
  "public_url": "http://bore.pub:41234",
  "public_port": 41234,
  "target_port": 3000,
  "error": null
}
```

`status` is one of `Connecting`, `Connected`, `Reconnecting`, `Failed` (retrying; `error` says why), or `Not started` after a stop. Without `--expose`, status reports `"enabled": false` and the other three answer `409`.

`target` only accepts a port the policy lets the VM bind, with something listening on it, and never the OS server's or the proxy's own port. All three POSTs are refused with `403` when the request carries an `Origin` other than the OS page's own, so another site open in the same browser cannot publish a port. A request with no `Origin`, like `curl`, is accepted.

## Security

- **Plain TCP.** bore carries bytes and nothing else: no TLS, so whatever the program serves crosses the internet unencrypted.
- **Anyone with the URL gets in.** A public port on `bore.pub` is not a secret; ports are easy to scan. Put authentication in the program if what it serves is not public.
- **The tunnel's secret authenticates *you* to the bore server.** It does nothing for visitors.
- The program is still inside the VM and its [network policy](./network-isolation.md): publishing its port changes who can reach it, not what it can reach.

## Troubleshooting

**`Failed`: could not reach bore.pub:7835.** Outbound connections to port 7835 are blocked where you are, or the server is down. The tunnel keeps retrying.

**`Failed`: the server requires a secret.** The server was started with `--secret`; pass `--tunnel-secret` or set `WASMRUN_TUNNEL_SECRET`.

**The public URL closes the connection immediately.** Nothing is running: press **Run**. Once the Console shows `Public URL: … → 127.0.0.1:<port>`, it forwards.

**The public URL changed.** The link dropped and the old public port was taken by the time it reconnected. The new one is in the Console panel and in `/api/tunnel/status`.

## What changed

Before 0.24, OS mode's built-in client did not speak bore's protocol, so it could not connect to `bore.pub` or any other bore server, and it forwarded no traffic. It was also aimed at the OS mode server's own port, the dev UI and its APIs, rather than at the program. `--expose` did not exist as a flag.

## See also

- [Serving a Port](./port-forwarding.md): how the program gets the port the tunnel publishes
- [Network Policy](./network-isolation.md): `bind_ports`, and what the program may connect to
- [bore](https://github.com/ekzhang/bore): the protocol and the server
