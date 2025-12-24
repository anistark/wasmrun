# run

Compile and run a WebAssembly project with a live development server.

## Synopsis

```bash
wasmrun run [PROJECT] [OPTIONS]
```

**Aliases:** `dev`, `serve`

## Description

The `run` command starts a development server that compiles your project and serves it with live-reloading capabilities. It's the primary command for development workflows.

When you run this command, Wasmrun will:
1. Detect the project language (or use the specified language)
2. Compile the project to WebAssembly
3. Start a development server
4. Optionally open the UI in your browser
5. Watch for file changes (if `--watch` is enabled)

## Options

### `-p, --path <PATH>`

Path to the project directory.

```bash
wasmrun run --path ./my-project
wasmrun run -p ./my-project
```

Default: Current directory (`.`)

:::tip
You can also use a positional argument: `wasmrun run ./my-project`
:::

### `-P, --port <PORT>`

Port number for the development server.

```bash
wasmrun run --port 3000
wasmrun run -P 8080
```

- Default: `8420`
- Valid range: `1-65535`

### `-l, --language <LANGUAGE>`

Force a specific language for compilation instead of auto-detection.

```bash
wasmrun run --language rust
wasmrun run -l go
```

Supported values:
- `rust` - Rust projects
- `go` - Go projects (using TinyGo)
- `c` - C/C++ projects
- `asc` - AssemblyScript projects
- `python` - Python projects

:::info Auto-Detection
If not specified, Wasmrun auto-detects the language by looking for:
- `Cargo.toml` → Rust
- `go.mod` → Go
- `Makefile` or `.c`/`.cpp` files → C/C++
- `package.json` with AssemblyScript → AssemblyScript
- `.py` files → Python
:::

### `--watch`

Enable watch mode for live-reloading on file changes.

```bash
wasmrun run --watch
```

When enabled, Wasmrun monitors your source files and automatically recompiles when changes are detected.

### `-v, --verbose`

Show detailed build output during compilation.

```bash
wasmrun run --verbose
wasmrun run -v
```

### `-s, --serve`

Automatically open the UI in your default browser when the server starts.

```bash
wasmrun run --serve
wasmrun run -s
```

## Examples

### Basic Usage

Start development server in current directory:

```bash
wasmrun run
```

### Specific Project

Run a specific project:

```bash
wasmrun run ./my-rust-project
```

### Custom Port

Use a different port:

```bash
wasmrun run --port 3000
```

### With Watch Mode

Enable live-reloading:

```bash
wasmrun run --watch
```

### Full Development Setup

Start server with watch mode and auto-open browser:

```bash
wasmrun run ./my-project --watch --serve --port 3000
```

### Force Language

Override auto-detection:

```bash
wasmrun run ./my-project --language rust
```

### Verbose Output

See detailed compilation logs:

```bash
wasmrun run --verbose
```

## Development Workflow

Typical development workflow:

```bash
# 1. Create or navigate to your project
cd my-wasm-project

# 2. Start development server with watch mode
wasmrun run --watch --serve

# 3. Edit your source files
# 4. Changes are automatically detected and recompiled
# 5. Browser refreshes automatically
```

## Server Behavior

When the server starts, you'll see output like:

```
🚀 Starting Wasmrun development server...
📦 Compiling project...
✓ Compilation successful
🌐 Server running at http://localhost:8420
```

Access your application at the provided URL.

## Language-Specific Notes

### Rust Projects

Requires `wasm32-unknown-unknown` target:

```bash
rustup target add wasm32-unknown-unknown
```

For web applications with wasm-bindgen:

```bash
wasmrun plugin install wasmrust
wasmrun run
```

### Go Projects

Requires TinyGo:

```bash
wasmrun plugin install wasmgo
wasmrun run
```

### Python Projects

Requires waspy plugin:

```bash
wasmrun plugin install waspy
wasmrun run
```

### C/C++ Projects

Built-in support, requires Emscripten:

```bash
wasmrun run
```

### AssemblyScript Projects

Requires wasmasc plugin:

```bash
wasmrun plugin install wasmasc
wasmrun run
```

## Stopping the Server

To stop the development server:

1. Press `Ctrl+C` in the terminal
2. Or use: `wasmrun stop`

## Troubleshooting

### Port Already in Use

If the port is already occupied:

```bash
# Use a different port
wasmrun run --port 8421

# Or stop existing server
wasmrun stop
```

### Compilation Errors

Enable verbose mode to see detailed errors:

```bash
wasmrun run --verbose
```

### Plugin Not Found

Install the required language plugin:

```bash
# For Rust
wasmrun plugin install wasmrust

# For Go
wasmrun plugin install wasmgo

# For Python
wasmrun plugin install waspy

# For AssemblyScript
wasmrun plugin install wasmasc
```

## See Also

- [exec](./exec.md) - Execute WASM files directly
- [compile](./compile.md) - Build without starting server
- [stop](./stop.md) - Stop running server
- [plugin](./plugin.md) - Manage language plugins
