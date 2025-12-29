---
sidebar_position: 7
title: OS Mode
description: Browser-based multi-language execution environment
---

# OS Mode

OS Mode provides a browser-based execution environment for running multiple language runtimes (Node.js, Python, and more) with system-level capabilities and real-time logging.

## Overview

OS Mode enables:
- **Browser-based execution** of Node.js and Python
- **Real-time logs panel** for stdout/stderr monitoring
- **Network isolation** for secure multi-process execution
- **Port forwarding** for external service access
- **Public tunneling** via bore.pub for internet exposure
- **Live reload** for development
- **Multiple runtimes** in parallel

This feature uses WebContainers technology to run full runtimes in the browser.

## Supported Languages

### Node.js
Full Node.js runtime with npm package support:

```bash
# Run Node.js application
wasmrun os ./node-app --language nodejs

# With live reload
wasmrun os ./node-app --language nodejs --watch

# With port forwarding
wasmrun os ./node-app --language nodejs --forward 8080:3000
```

**Supported:**
- Express.js, Fastify, Koa servers
- npm packages and dependencies
- File system access
- Network operations
- Child processes

### Python
Python runtime with pip package support:

```bash
# Run Python application
wasmrun os ./python-app --language python

# Flask/FastAPI apps with forwarding
wasmrun os ./api --language python --forward 8080:5000
```

**Supported:**
- Flask, FastAPI, Django apps
- pip packages
- Standard library modules
- File I/O
- HTTP servers

## Real-Time Logs Panel

OS Mode includes a built-in real-time logs viewer:

### Features
- **Live stdout/stderr** streaming
- **Colored output** for better readability
- **Auto-scroll** to latest logs
- **Search and filter** capabilities
- **Log persistence** across refreshes
- **Clear logs** button

### Access

The logs panel is automatically shown when running in OS mode:
```bash
wasmrun os ./app --language nodejs
# Browser opens with split view:
# - Top: Application output
# - Bottom: Real-time logs panel
```

### Log Categories

Logs are categorized by stream:
- 🟦 **stdout**: Standard output (blue)
- 🟥 **stderr**: Error output (red)
- 🟨 **system**: Wasmrun system messages (yellow)

## Network Capabilities

### Network Isolation

Each OS mode process runs in an isolated network namespace:

```bash
# Process A
wasmrun os ./app-a --language nodejs

# Process B (different namespace, no conflicts)
wasmrun os ./app-b --language nodejs
```

See [Network Isolation](./network-isolation.md) for details.

### Port Forwarding

Expose services to host/external networks:

```bash
# Forward host :8080 to process :3000
wasmrun os ./node-server --language nodejs --forward 8080:3000
```

See [Port Forwarding](./port-forwarding.md) for details.

### Public Tunneling

Expose your local WASM applications to the internet using bore.pub:

```bash
# Start tunnel via API
# POST /api/tunnel/start

# Your app gets a public URL like:
# http://bore.pub:12345
```

**Features:**
- Public bore.pub server (default) or custom self-hosted servers
- Automatic reconnection on disconnect
- Connection status monitoring
- Optional authentication for private servers

See [Public Tunneling](./public-tunneling.md) for details.

## Use Cases

### Web Development

```bash
# Next.js development server
wasmrun os ./nextjs-app --language nodejs --forward 3000:3000 --watch

# Flask API development
wasmrun os ./api --language python --forward 5000:5000 --watch
```

### API Development

```bash
# Express API with auto-reload
wasmrun os ./express-api --language nodejs --watch --forward 8080:3000

# FastAPI with uvicorn
wasmrun os ./fastapi-app --language python --watch --forward 8000:8000
```

### Testing & Debugging

```bash
# Run with verbose logging
wasmrun os ./app --language nodejs --verbose

# Debug mode
wasmrun os ./app --language python --debug
```

## CLI Options

### Language Selection

```bash
# Explicitly specify language
wasmrun os ./app --language nodejs
wasmrun os ./app --language python

# Auto-detect (looks for package.json, requirements.txt, etc.)
wasmrun os ./app
```

### Port Configuration

```bash
# Custom server port (default: 8420)
wasmrun os ./app --port 3000

# Port forwarding
wasmrun os ./app --forward 8080:3000
```

### Watch Mode

```bash
# Enable live reload
wasmrun os ./app --watch --language nodejs
```

### Verbose Output

```bash
# Show detailed logs
wasmrun os ./app --verbose
```

## Configuration

### Project Configuration

Create `.wasmrun.toml` in your project:

```toml
[os_mode]
# Default language runtime
language = "nodejs"

# Server settings
port = 8420

# Port forwarding rules
forwards = ["8080:3000"]

# Enable live reload
watch = true

# Log settings
[os_mode.logs]
max_lines = 1000
auto_scroll = true
```

### Environment Variables

```bash
# Set environment variables
PORT=3000 wasmrun os ./app --language nodejs

# Or in package.json scripts
{
  "scripts": {
    "dev": "wasmrun os . --language nodejs --watch"
  }
}
```

## WebContainers Integration

OS Mode uses WebContainers to provide full runtime environments in the browser:

### How It Works

1. **Boot Container**: Initializes virtual file system
2. **Mount Files**: Mounts your project files
3. **Install Dependencies**: Runs npm install / pip install
4. **Start Process**: Executes your application
5. **Stream Logs**: Captures and displays real-time output

### Capabilities

- Full Node.js/Python runtime
- File system operations
- Network access (with isolation)
- Process spawning
- Package installation

### Limitations

- Browser-only (requires modern browser)
- Some native modules not supported
- Performance depends on browser

## Examples

### Express Server

```javascript
// server.js
const express = require('express');
const app = express();

app.get('/', (req, res) => {
    res.send('Hello from OS Mode!');
});

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
    console.log(`Server running on port ${PORT}`);
});
```

```bash
# Run in OS mode
wasmrun os . --language nodejs --forward 8080:3000 --watch
```

### Flask API

```python
# app.py
from flask import Flask, jsonify

app = Flask(__name__)

@app.route('/api/data')
def get_data():
    return jsonify({'message': 'Hello from Python OS Mode!'})

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
```

```bash
# Run in OS mode
wasmrun os . --language python --forward 8080:5000 --watch
```

## Comparison: OS Mode vs Native Execution

| Feature | OS Mode | Native Execution |
|---------|---------|------------------|
| **Environment** | Browser | Terminal |
| **Languages** | Node.js, Python | WASM-compiled languages |
| **Runtime** | Full language runtime | WASM interpreter |
| **Use Case** | Web development | CLI tools, libraries |
| **Dependencies** | npm, pip supported | WASM-only |
| **Network** | Full TCP/UDP | WASI sockets |
| **Logs** | Real-time panel | Terminal output |

## Troubleshooting

### WebContainer Boot Failed

```bash
# Ensure modern browser (Chrome 88+, Firefox 89+)
# Check browser console for errors
# Try different browser
```

### Dependencies Install Failed

```bash
# Check package.json/requirements.txt syntax
# Verify network connectivity
# Check logs panel for error details
```

### Port Forwarding Not Working

```bash
# Verify process is listening on correct port
# Check firewall settings
# Ensure port is not already in use
```

### Live Reload Not Triggering

```bash
# Ensure --watch flag is set
# Check file watcher is detecting changes
# Use --verbose to see file change events
```

## See Also

- [Network Isolation](./network-isolation.md) - Network namespace details
- [Port Forwarding](./port-forwarding.md) - Port forwarding guide
- [Public Tunneling](./public-tunneling.md) - Expose apps to the internet
- [Live Reload](../server/live-reload.md) - Live reload details
- [CLI os Command](../cli/os.md) - Full command reference
