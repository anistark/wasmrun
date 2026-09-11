---
sidebar_position: 5
title: Networking
---

# Networking

Sandboxed code can reach the network and can serve on a port, and neither is available unless the host turns it on.

This page covers `wasmrun exec` and agent mode, which share one interpreter and one WASI layer. OS mode runs code in a browser and reaches the network a different way; see [Network Policy](../os/network-isolation.md).

## Nothing is allowed by default

A program run by `wasmrun exec` has no network. `sock_open` fails, no connection can be made, and no flag needs to be passed to get that behavior. Egress is something you turn on:

```sh
# No network at all.
wasmrun exec ./fetcher.wasm

# May reach api.github.com on 443, and nothing else.
wasmrun exec --allow-net "api.github.com:443" ./fetcher.wasm
```

`--allow-net` is repeatable. Each rule is a hostname, a `*.domain` wildcard, or a CIDR, each with an optional `:port`.

```sh
wasmrun exec \
  --allow-net "*.githubusercontent.com:443" \
  --allow-net "10.0.5.0/24:5432" \
  ./job.wasm
```

### How a rule matches

- A bare domain covers its subdomains. `example.com` allows `api.example.com`, and is equivalent to `*.example.com`.
- A rule's port is enforced. `api.example.com:443` opens that port and no other; a rule with no port covers every port.
- Ports work on CIDR and bare-IP rules too: `10.0.0.0/8:22`, `[::1]:80`.
- A trailing dot does not escape a rule: `evil.com.` is matched by `evil.com`.

### Names are resolved by the host, and every address is checked

A policy checked only against the string the guest supplied is a policy the guest can talk its way around, because a name it controls can resolve into a range you meant to block. `localhost` and a cloud metadata endpoint are the everyday cases.

So the order is: check the name against the domain rules, resolve it here, check every resolved address against the deny rules, then connect to an address that passed rather than resolving again and possibly getting a different answer.

**One refused address refuses the whole name.** A name maps to a set the guest does not choose from, so accepting the subset that passed would still let the connection land somewhere the policy refuses. `localhost` resolving to both `127.0.0.1` and `::1` under an IPv4-only deny list is exactly that case.

The allow decision is made on the name only. Re-checking the allow list against a resolved address would refuse every domain rule ever written, since an address never matches a pattern like `*.github.com`.

A refusal is logged with its reason for whoever is operating the server. The program itself only ever sees an errno.

## Serving a port

WASI Preview 1 has no call that creates a socket, so a guest cannot ask for a port. The host binds one and passes the listening socket in as a file descriptor, the way a preopened directory arrives:

```sh
wasmrun exec --tcplisten 127.0.0.1:8080 ./server.wasm
```

```
🔌 Listening on 127.0.0.1:8080, passed to the program as fd 3
```

Preopened descriptors start at 3 and are handed out in order, so the first `--tcplisten` is fd 3. The flag is repeatable and each one takes the next descriptor.

This works with a stock toolchain. A `cargo build --target wasm32-wasip1` binary calling `TcpListener::accept` imports `sock_accept` and then reads and writes the connection with the ordinary file calls, so no WASIX build and no custom SDK is involved:

```rust
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::fd::FromRawFd;

fn main() {
    // fd 3 is the listener the host bound and passed in.
    let listener = unsafe { TcpListener::from_raw_fd(3) };
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi");
    }
}
```

In JavaScript nothing is unsafe or manual: wasmhub's `net` and `http` read the descriptor from `WASMHUB_LISTEN_FD` and the address from `WASMHUB_LISTEN_ADDR`, so an ordinary `http.createServer(...).listen()` works. Agent mode sets both for you, see [Serving](../agent/usage/serving.md).

### What is implemented

| Call | State |
|---|---|
| `sock_accept`, `sock_recv`, `sock_send`, `sock_shutdown` | Implemented. Standard Preview 1 |
| `fd_read`, `fd_write` on an accepted connection | Implemented |
| `sock_open`, `sock_connect` | Implemented as wasmrun extensions. Preview 1 has neither |
| `sock_bind`, `sock_listen` | **Not implemented.** A guest cannot bind its own port; `--tcplisten` is how a listener arrives |
| `poll_oneoff` on a socket | Implemented, reporting real readiness rather than always-ready |

`sock_open` and `sock_connect` take a host string rather than a packed address on purpose. A packed address would force the guest to resolve the name first, which puts DNS outside the policy entirely; the policy is written in terms of names as much as addresses, so the host has to see the name that was asked for.

### Blocking calls and timeouts

A blocking `accept` or `recv` watches the execution's cancellation flag in slices. Without that, a wall-clock timeout could not interrupt a program waiting for a connection that never comes: the interpreter only checks that flag between instructions, which never happens while a host function is blocked.

A listener cannot be asked whether a connection is waiting without accepting it, so a `poll_oneoff` probe that finds one parks it for the `sock_accept` that follows rather than dropping the connection.

## Agent mode

The same policy engine, with the policy resolved per execution rather than per process.

Set a default for every tenant when starting the server:

```sh
wasmrun agent --allow-net "api.example.com:443"
```

Give a tenant its own, in the auth config, beside the `[tenants.rate]` and `[tenants.limits]` tables:

```toml
[[tenants]]
id = "acme"
key_sha256 = "..."

  [tenants.network]
  allow = ["*.acme-internal.com:443"]
  deny = ["10.0.0.0/8"]
```

A tenant with no `[tenants.network]` table gets the server default, which is no network unless the operator set one. The policy is read on every exec, so an auth reload reaches the next execution rather than only new sessions. A tenant policy that cannot be honored stops the server starting.

Upgrading from a policy written before wasmrun 0.24: a bare domain now covers subdomains and a rule's port is now enforced, so an existing `allow` list may grant both more and less than it did. Review the rules against the matching notes above.

## What JavaScript can do

wasmhub's nodejs runtime implements `net` and `http` **servers**. Outbound is not there yet:

| API | State |
|---|---|
| `http.createServer`, `net.createServer` | Works, on a host-bound listener |
| `net.connect`, `http.request`, `fetch` | Throws `ERR_NOT_SUPPORTED` |
| `https`, `dgram`, `tls` | Present but throwing |

The syscalls to support outbound exist in wasmrun; the runtime has not been built against them. Tracked in [wasmhub#22](https://github.com/anistark/wasmhub/issues/22).

## See also

- [WASI support](./wasi.md): the rest of the syscall surface
- [Serving from a session](../agent/usage/serving.md): the agent-mode lifecycle
- [OS mode network policy](../os/network-isolation.md): the browser VM's separate path
