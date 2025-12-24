---
sidebar_position: 5
title: Network Isolation
description: Per-process network namespace isolation for WASM processes
---

# Network Isolation

Wasmrun provides per-process network namespace isolation for WASM processes, ensuring each process runs in its own isolated network environment with full socket API support.

## Overview

Network isolation provides:
- **Per-process namespaces** preventing cross-process interference
- **Full WASI socket API** implementation
- **Isolated network stacks** for each WASM process
- **Security** through namespace separation

This feature was introduced in v0.15.x and is part of the OS Mode capabilities.

## WASI Socket API Support

Wasmrun implements the complete WASI socket API:

### Socket Operations
- `sock_open` - Create new sockets
- `sock_bind` - Bind socket to address
- `sock_listen` - Listen for connections
- `sock_accept` - Accept incoming connections
- `sock_connect` - Connect to remote hosts
- `sock_send` - Send data
- `sock_recv` - Receive data
- `sock_shutdown` - Shutdown socket

### Supported Socket Types
- **TCP sockets** for reliable connections
- **UDP sockets** for datagram communication
- **Unix domain sockets** (local communication)

## How It Works

### Network Namespace Creation

When a WASM process starts:
1. **New namespace** created for the process
2. **Isolated network stack** initialized
3. **Loopback interface** configured
4. **Socket syscalls** routed to the namespace

### Process Isolation

Each process gets:
- Its own network interfaces
- Separate routing tables
- Isolated port bindings
- Independent firewall rules

## Usage

### Basic Network Operations

```rust
// Rust example with WASI sockets
use std::net::{TcpListener, TcpStream};

fn main() {
    // Listen on isolated namespace
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        handle_connection(stream);
    }
}
```

```bash
# Run with network isolation
wasmrun os ./network-app
```

### OS Mode Integration

Network isolation is automatically enabled in OS mode:

```bash
# Node.js server with isolated network
wasmrun os ./node-server --language nodejs

# Python web app with isolated network
wasmrun os ./python-app --language python
```

## Security Benefits

### Prevents Cross-Process Interference

Without isolation:
```
Process A binds to :8080 → Success
Process B binds to :8080 → Port conflict error
```

With isolation:
```
Process A (namespace 1) binds to :8080 → Success
Process B (namespace 2) binds to :8080 → Success (different namespace)
```

### Attack Surface Reduction
- Processes can't sniff other processes' traffic
- Network attacks limited to single namespace
- Easier to apply per-process firewall rules

## Port Forwarding

To expose services running in isolated namespaces to the host or external networks, use the port forwarding feature:

```bash
# Forward host port to WASM process port
wasmrun os ./app --forward 8080:3000
```

See [Port Forwarding](./port-forwarding.md) for details.

## Configuration

### Default Isolation

Network isolation is enabled by default in OS mode:

```bash
# Isolation automatically enabled
wasmrun os ./app
```

### Custom Network Settings

Configure network behavior in `.wasmrun.toml`:

```toml
[network]
# Enable/disable network isolation (default: true)
isolation = true

# Namespace configuration
namespace_prefix = "wasmrun"

# Network interface settings
loopback_enabled = true
```

## Compatibility

### Supported Platforms
- **Linux** - Full support with kernel namespaces
- **macOS** - Limited support (simulated isolation)
- **Windows** - Limited support (simulated isolation)

### Requirements
- Linux kernel 3.8+ for full namespace support
- Root/CAP_NET_ADMIN for namespace creation (or unprivileged user namespaces)

## Troubleshooting

### Permission Denied

```bash
# If namespace creation fails due to permissions
sudo wasmrun os ./app

# Or configure unprivileged user namespaces (Linux)
sudo sysctl -w kernel.unprivileged_userns_clone=1
```

### Connection Refused

```bash
# Ensure service is listening in the namespace
# Check logs for bind errors
wasmrun os ./app --verbose
```

### Port Conflicts

If you see port conflicts even with isolation:
- Check if isolation is actually enabled
- Verify namespace creation succeeded
- Review logs with `--verbose`

## Examples

### HTTP Server

```javascript
// Node.js server in isolated namespace
const http = require('http');

const server = http.createServer((req, res) => {
    res.writeHead(200);
    res.end('Hello from isolated namespace!');
});

server.listen(3000, () => {
    console.log('Server running on port 3000');
});
```

```bash
# Run with network isolation
wasmrun os ./server --language nodejs

# Access via port forwarding
wasmrun os ./server --language nodejs --forward 8080:3000
```

### Multiple Isolated Processes

```bash
# Terminal 1: Process A on port 8080 in namespace A
wasmrun os ./app-a --forward 8080:8080

# Terminal 2: Process B on port 8080 in namespace B
wasmrun os ./app-b --forward 8081:8080

# No port conflict - different namespaces!
```

## See Also

- [OS Mode](./os-mode.md) - Full OS mode documentation
- [Port Forwarding](./port-forwarding.md) - Exposing isolated services
- [WASI Support](./wasi-support.md) - WASI socket APIs
- [CLI os Command](../cli/os.md) - OS mode command reference
