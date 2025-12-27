# os

Run projects in browser-based multi-language OS mode.

## Synopsis

```bash
wasmrun os [PROJECT] [OPTIONS]
```

## Description

The `os` command starts a browser-based execution environment that runs Node.js and Python code directly in the browser using WebContainers technology. This provides a full operating system-like experience with:

- Multi-language support (Node.js, Python)
- Real-time logs and output
- Interactive terminal
- File system access
- Network capabilities
- Process isolation

OS mode is ideal for:
- Web-based development environments
- Interactive demos
- Educational projects
- Full-stack WebAssembly applications
- Projects requiring Node.js or Python runtime in the browser

## Options

### `-p, --path <PATH>`

Path to the project directory.

```bash
wasmrun os --path ./my-project
wasmrun os -p ./my-project
```

Default: Current directory (`.`)

:::tip
You can also use a positional argument: `wasmrun os ./my-project`
:::

### `-P, --port <PORT>`

Port number for the OS mode server.

```bash
wasmrun os --port 3000
wasmrun os -P 8080
```

- Default: `8420`
- Valid range: `1-65535`

### `-l, --language <LANGUAGE>`

Force a specific language for OS mode execution.

```bash
wasmrun os --language nodejs
wasmrun os -l python
```

Supported languages:
- `nodejs` - Node.js runtime
- `python` - Python runtime

:::info Auto-Detection
If not specified, Wasmrun auto-detects based on project files:
- `package.json` → Node.js
- `*.py` files → Python
:::

### `--watch`

Enable watch mode for live-reloading on file changes.

```bash
wasmrun os --watch
```

### `-v, --verbose`

Show detailed build output.

```bash
wasmrun os --verbose
wasmrun os -v
```

## Examples

### Basic Usage

Start OS mode for current project:

```bash
wasmrun os
```

### Specific Project

Run a specific project in OS mode:

```bash
wasmrun os ./nodejs-api
```

### With Custom Port

```bash
wasmrun os --port 3000
```

### Node.js Project

```bash
wasmrun os ./my-express-app --language nodejs
```

### Python Project

```bash
wasmrun os ./my-python-app --language python
```

### With Watch Mode

Enable live-reloading:

```bash
wasmrun os --watch
```

### Full Configuration

```bash
wasmrun os ./my-project \
  --port 3000 \
  --language nodejs \
  --watch \
  --verbose
```

## What is OS Mode?

OS mode provides a complete browser-based operating system environment using WebContainers:

### WebContainers Technology

- Runs Node.js and Python in the browser
- No server-side execution required
- Full file system access
- Network capabilities
- Process isolation
- Real POSIX-like environment

### Features

- **Multi-Language Runtime**: Execute Node.js and Python code
- **Interactive Terminal**: Command-line access in browser
- **File System**: Read/write files directly
- **Package Managers**: npm, pip support
- **Network Access**: Make HTTP requests
- **Real-time Logs**: See output as it happens
- **Process Management**: Run multiple processes

## Supported Languages

### Node.js

Full Node.js runtime in the browser:

```javascript
// server.js
const express = require('express');
const app = express();

app.get('/', (req, res) => {
  res.send('Hello from WebAssembly OS mode!');
});

app.listen(3000, () => {
  console.log('Server running on port 3000');
});
```

Run:

```bash
wasmrun os --language nodejs
```

Features:
- Express.js support
- npm packages
- File I/O
- HTTP server
- WebSocket support

### Python

Python runtime in the browser:

```python
# app.py
from flask import Flask

app = Flask(__name__)

@app.route('/')
def hello():
    return 'Hello from Python in WebAssembly!'

if __name__ == '__main__':
    app.run(port=5000)
```

Run:

```bash
wasmrun os --language python
```

Features:
- Flask support
- pip packages
- File I/O
- Standard library

## UI Interface

When you start OS mode, you get a browser-based interface with:

### Code Editor

- Syntax highlighting
- File explorer
- Edit project files directly

### Terminal

- Interactive shell
- Run commands
- See output in real-time

### Logs Panel

- Real-time application logs
- Error messages
- Console output
- Color-coded by severity

### Preview Window

- See your application running
- Interactive preview
- Hot reload support

## Use Cases

### Express API Server

```bash
# Start Express app in OS mode
wasmrun os ./express-api --language nodejs --port 3000
```

Project structure:

```
express-api/
├── package.json
├── server.js
└── routes/
    └── api.js
```

### Python Flask App

```bash
wasmrun os ./flask-app --language python
```

Project structure:

```
flask-app/
├── app.py
├── requirements.txt
└── templates/
    └── index.html
```

### Full-Stack Application

```bash
wasmrun os ./fullstack-app --watch
```

With both frontend and backend:

```
fullstack-app/
├── package.json
├── server.js
├── public/
│   ├── index.html
│   └── app.js
└── api/
    └── routes.js
```

## Network Isolation

OS mode provides per-process network namespace isolation:

- Each WebContainer runs in isolated environment
- Secure execution
- No interference between processes
- Clean separation

See [Network Isolation](/docs/os/network-isolation) for details.

## Port Forwarding

OS mode supports port forwarding to access services:

```bash
# Forward port 3000 to access Express server
wasmrun os --port 8420  # UI on 8420, app on 3000
```

See [Port Forwarding](/docs/os/port-forwarding) for configuration.

## Performance

OS mode performance:

- **Startup**: 2-5 seconds
- **Hot reload**: < 1 second
- **File operations**: Near-native speed
- **Network**: Full browser network stack

## Limitations

### Current Limitations

- Browser-based execution only
- Limited to Node.js and Python
- Some native modules may not work
- File system is in-browser (not persistent across sessions)
- Resource limits based on browser

### Not Supported

- Native C/C++ libraries
- System-level operations
- Docker containers
- Database servers (use external services)

### Workarounds

- Use external databases (cloud-hosted)
- Leverage browser APIs where possible
- Package compatible npm/pip packages

## Comparison with `run` Command

| Feature | `run` | `os` |
|---------|-------|------|
| WebAssembly | Yes | Optional |
| Node.js | No | Yes |
| Python | WASM only | Native runtime |
| Browser execution | WASM modules | Full runtime |
| Server-side | Development server | Browser-based |
| Use case | WASM projects | Multi-language apps |

## Stopping OS Mode Server

Press `Ctrl+C` or use:

```bash
wasmrun stop
```

## Troubleshooting

### Port Already in Use

```bash
# Use different port
wasmrun os --port 8421

# Or stop existing server
wasmrun stop
```

### Language Not Detected

Force language selection:

```bash
wasmrun os --language nodejs
```

### Module Not Found

Ensure dependencies are in `package.json` or `requirements.txt`:

```bash
# Node.js
npm install

# Python
pip install -r requirements.txt
```

### WebContainer Not Loading

Check browser compatibility:
- Chrome/Edge 88+
- Firefox 89+
- Safari 15.4+

### Performance Issues

- Close unnecessary browser tabs
- Check browser console for errors
- Reduce file watch scope
- Disable browser extensions

## Browser Requirements

OS mode requires a modern browser with:

- WebAssembly support
- SharedArrayBuffer support
- Modern JavaScript (ES2020+)
- Sufficient memory (4GB+ recommended)

## Examples from Repository

### Node.js Express API

See `examples/nodejs-express-api/` for a complete example:

```bash
wasmrun os ./examples/nodejs-express-api
```

Features demonstrated:
- Express server
- REST API endpoints
- Port forwarding
- Real-time logs

## See Also

- [run](./run.md) - Standard development server for WASM
- [exec](./exec.md) - Native WASM execution
- [OS Mode](/docs/os) - Detailed feature guide
- [Network Isolation](/docs/os/network-isolation) - Security details
- [Port Forwarding](/docs/os/port-forwarding) - Configuration guide
