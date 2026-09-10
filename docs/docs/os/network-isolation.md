---
sidebar_position: 5
title: Network Policy
---

# Network Policy

Code running in a wasmrun sandbox has no network unless someone gives it one. When it does, every connection it opens is checked against a policy the host configured, and the sandbox has no way to see or change that policy.

:::info Where this applies
This page covers **OS mode**, where a project runs in the browser VM and its sockets leave through a local proxy.

For `wasmrun exec` and agent mode, where wasmrun runs the program itself, see [exec networking](../exec/networking.md). The policy syntax on this page is shared; the enforcement point is not.
:::

## What is implemented today

Two halves, and only one of them is connected in OS mode:

| Piece | State |
|---|---|
| The [wasmnet](https://github.com/anistark/wasmnet) proxy runs beside the OS server, under the project's policy | Working |
| `[os.network]` in `wasmrun.toml` configures that policy, and a malformed rule stops the server starting | Working |
| The browser's WASI shim calls the proxy, so a program in the VM can actually open a socket | **Not yet** |

The last row is the one that matters to a running project. Until it lands, a program inside the browser VM cannot open a socket at all: the proxy is up and speaks its protocol, but nothing in the VM talks to it. Progress is tracked in [wasmrun#99](https://github.com/anistark/wasmrun/issues/99).

The blocker is not the proxy. The WASI shim runs `_start()` synchronously on the browser's main thread, so an imported function has to return before the event loop turns again, and a socket call has nothing to wait on. Fixing it means giving the shim a way to suspend, either with JSPI or by moving the VM into a worker with `SharedArrayBuffer` and `Atomics.wait`.

## The proxy

Sockets in a browser do not exist. A WASM program in the VM that wants a TCP connection has to have one opened on its behalf, somewhere that can open one, and that is what the proxy is for: it runs on the host beside the OS server, accepts a WebSocket from the VM, and makes the real connection under the policy below.

It starts and stops with `wasmrun os`, and picks its own port:

```
OS server   http://localhost:8420
wasmnet     http://localhost:8440     (os_port + 20, scanning up if taken)
```

`GET /api/network/status` reports the URL, since the port is chosen at startup rather than fixed:

```sh
curl http://localhost:8420/api/network/status
```

```json
{
  "running": true,
  "url": "ws://127.0.0.1:8440"
}
```

The proxy binds loopback only. It opens connections on behalf of whoever reaches it, so it is not something to expose.

## Configuring the policy

Put an `[os.network]` table in a `wasmrun.toml` at your project root:

```toml
[os.network]
# Hosts and ranges the sandbox may reach. A rule is a hostname, a
# "*.domain" wildcard, or a CIDR, each with an optional ":port".
allow = ["api.example.com:443", "*.githubusercontent.com"]

# Ranges it may never reach, checked before the allow list.
deny = ["10.0.0.0/8", "192.168.0.0/16"]

# Ports a program in the VM may bind, once inbound sockets are wired up.
bind_ports = "3000-3999,8080"

# Ceilings.
max_connections = 100
max_bandwidth_mbps = 10
connection_timeout_secs = 30
```

Every field falls back on its own, so a table that sets only `allow` keeps the default `deny`. The default policy blocks RFC1918, loopback and link-local, which is what stops a sandboxed program from reaching your router, your database or a cloud metadata endpoint.

`wasmrun.toml` at the project root is the project's own config. It is not `~/.wasmrun/config.toml`, which is the global one, and not the `wasmrun.toml` inside a plugin directory, which is a plugin manifest.

### A rule you believe in and do not have

Rules are validated before the proxy binds anything, and a bad one is an error rather than a warning. This is deliberate. The policy engine parses a rule as a CIDR first and treats anything else as a *hostname*, so:

```toml
deny = ["10.0.0/8"]   # refused: one octet short of a range
```

would quietly become a hostname that no address ever matches. You would have a deny rule that reads correctly, appears in your config, and blocks nothing. A bare IP has the same problem and is refused with the CIDR you meant:

```toml
deny = ["203.0.113.7"]      # refused
deny = ["203.0.113.7/32"]   # what to write instead
```

An unknown key inside `[os.network]` is also an error, so a typo cannot silently disable a rule. Other tables in the file are ignored, so the config can grow later.

A policy that cannot be honored stops OS mode before it binds anything. The file was written to be obeyed, and running a project under settings nobody chose is worse than refusing to run it.

### How a rule is matched

- A bare domain covers its subdomains: `example.com` allows `api.example.com`. It is equivalent to `*.example.com`.
- A rule's port is enforced: `api.example.com:443` opens that port and no other. A rule without a port covers every port.
- Ports work on CIDR and bare-IP rules too: `10.0.0.0/8:22`.
- A trailing dot does not escape a rule: `evil.com.` is denied by `evil.com`.
- Deny is checked before allow.

## What this is not

This page used to describe per-process kernel network namespaces, `CAP_NET_ADMIN`, and a `[network]` table with `isolation`, `namespace_prefix` and `loopback_enabled` keys. None of that was ever implemented and none of those keys are read by anything.

Isolation here is a **policy** enforced at the point where a connection is opened, not a kernel namespace. There is no `--forward` flag and no per-process network stack. What a sandboxed program can reach is decided by the rules above, and nothing else.

## See also

- [Exec and agent networking](../exec/networking.md): sockets where wasmrun runs the program itself
- [Public tunneling](./public-tunneling.md): exposing the OS mode server itself
- [wasmrun#99](https://github.com/anistark/wasmrun/issues/99): the browser socket bridge
