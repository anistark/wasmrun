---
sidebar_position: 6
title: Port Forwarding
description: Forward ports from host to isolated WASM processes
---

# Port Forwarding

Port forwarding enables external access to services running in isolated WASM network namespaces by mapping host ports to process ports.

## Overview

Port forwarding provides:
- **Host-to-namespace** port mapping
- **External access** to isolated services
- **Multiple port** forwarding support
- **Automatic setup** with OS mode

This feature works in conjunction with [Network Isolation](./network-isolation.md) to expose services running in isolated namespaces.

## Basic Usage

### Single Port Forward

```bash
# Forward host port 8080 to process port 3000
wasmrun os ./app --forward 8080:3000

# Access at http://localhost:8080
# Maps to process listening on port 3000
```

### Multiple Ports

```bash
# Forward multiple ports
wasmrun os ./app --forward 8080:3000 --forward 8081:3001
```

### Same Port

```bash
# Forward same port number
wasmrun os ./app --forward 3000:3000
```

## Syntax

The `--forward` flag accepts the format:

```
--forward <HOST_PORT>:<PROCESS_PORT>
```

Where:
- **HOST_PORT**: Port on the host machine (external access)
- **PROCESS_PORT**: Port in the WASM process namespace

## Use Cases

### Web Servers

```javascript
// Express server listening on port 3000
const express = require('express');
const app = express();

app.get('/', (req, res) => {
    res.send('Hello from WASM process!');
});

app.listen(3000);
```

```bash
# Forward host :8080 to process :3000
wasmrun os ./node-server --language nodejs --forward 8080:3000

# Access at http://localhost:8080
```

### API Services

```python
# Flask API on port 5000
from flask import Flask
app = Flask(__name__)

@app.route('/api/status')
def status():
    return {'status': 'running'}

if __name__ == '__main__':
    app.run(port=5000)
```

```bash
# Expose API on host port 3000
wasmrun os ./api --language python --forward 3000:5000

# Access at http://localhost:3000/api/status
```

### Database Services

```bash
# PostgreSQL in isolated namespace
wasmrun os ./postgres-app --forward 5432:5432

# Connect from host
psql -h localhost -p 5432 -U user database
```

## Multiple Process Example

Run multiple isolated processes on the same internal port, exposed via different host ports:

```bash
# Terminal 1: Service A (namespace A, internal :8080)
wasmrun os ./service-a --forward 3000:8080

# Terminal 2: Service B (namespace B, internal :8080)
wasmrun os ./service-b --forward 3001:8080

# Terminal 3: Service C (namespace C, internal :8080)
wasmrun os ./service-c --forward 3002:8080

# Access each service independently
curl http://localhost:3000  # Service A
curl http://localhost:3001  # Service B
curl http://localhost:3002  # Service C
```

## Security Considerations

### Binding to Localhost

By default, forwarded ports bind to localhost only:
- Safe for local development
- Not accessible from external networks
- Requires explicit configuration for external access

### Firewall Rules

Port forwarding respects host firewall rules:
```bash
# Host firewall still controls external access
# Even if forwarded, external access may be blocked
```

### Process Isolation

Each process is isolated:
- Can't interfere with other processes
- Separate network stacks
- Independent port bindings

## Configuration

### Via CLI

```bash
# Command-line forwarding
wasmrun os ./app \
    --forward 8080:3000 \
    --forward 8081:3001
```

### Via Configuration File

```toml
# .wasmrun.toml
[network]
# Port forwarding rules
forwards = [
    "8080:3000",
    "8081:3001"
]
```

```bash
# Reads from .wasmrun.toml
wasmrun os ./app
```

## Combined with Live Reload

Port forwarding works with live reload for development:

```bash
# Forward ports + watch for changes
wasmrun os ./app --forward 8080:3000 --watch

# Server auto-restarts on file changes
# Port forwarding maintained across restarts
```

## Troubleshooting

### Port Already in Use

```bash
# Error: host port 8080 already in use
# Solution: Use different host port
wasmrun os ./app --forward 8081:3000
```

### Connection Refused

```bash
# Ensure process is listening
# Check process logs
wasmrun os ./app --forward 8080:3000 --verbose

# Verify port forwarding is active
netstat -an | grep 8080
```

### Firewall Blocking

```bash
# On Linux, check firewall rules
sudo iptables -L -n

# On macOS, check firewall settings
sudo pfctl -s rules
```

## Platform Support

### Linux
✅ Full support with iptables/nftables

### macOS
✅ Full support with pf (packet filter)

### Windows
⚠️ Limited support (development mode only)

## Examples

### Node.js Express API

```javascript
// server.js
const express = require('express');
const app = express();

app.get('/api/hello', (req, res) => {
    res.json({ message: 'Hello from isolated process!' });
});

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
    console.log(`Server running on port ${PORT}`);
});
```

```bash
# Forward and run
wasmrun os . --language nodejs --forward 8080:3000

# Test API
curl http://localhost:8080/api/hello
```

### Python Flask App

```python
# app.py
from flask import Flask, jsonify

app = Flask(__name__)

@app.route('/health')
def health():
    return jsonify({'status': 'healthy'})

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
```

```bash
# Forward and run
wasmrun os . --language python --forward 3000:5000

# Check health
curl http://localhost:3000/health
```

## See Also

- [Network Isolation](./network-isolation.md) - Network namespace details
- [OS Mode](./os-mode.md) - Full OS mode documentation
- [CLI os Command](../cli/os.md) - Command reference
- Example: [Node.js Express API](https://github.com/anistark/wasmrun/tree/main/examples/nodejs-express-api) - Complete example
