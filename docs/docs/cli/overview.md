# CLI Overview

Wasmrun provides a comprehensive command-line interface for working with WebAssembly projects.

## Command Structure

```bash
wasmrun [COMMAND] [OPTIONS] [ARGS]
```

### Basic Usage

```bash
# Run a project in current directory
wasmrun

# Run a specific project
wasmrun ./my-project

# Run with a specific command
wasmrun run ./my-project --port 3000
```

## Available Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `run` | `dev`, `serve` | Compile and run project with live development server |
| `exec` | - | Execute a WASM file directly with arguments |
| `compile` | `build`, `c` | Compile project to WebAssembly |
| `plugin` | - | Manage plugins (install, list, update, etc.) |
| `verify` | - | Verify WebAssembly file format and structure |
| `inspect` | - | Perform detailed inspection on a WASM file |
| `clean` | `clear`, `reset` | Clean build artifacts and temporary files |
| `stop` | `kill` | Stop any running Wasmrun server instance |
| `os` | - | Run projects in browser-based multi-language OS mode |

## Global Options

These options work with any command:

### `--debug`

Enable detailed debug output for troubleshooting.

```bash
wasmrun --debug run ./my-project
```

### `--help, -h`

Show help information for a command.

```bash
wasmrun --help
wasmrun run --help
```

### `--version, -V`

Display version information.

```bash
wasmrun --version
```

## Common Options

### Path Resolution

Wasmrun supports both positional and flag-based path arguments:

```bash
# Positional argument (recommended)
wasmrun run ./my-project

# Using --path flag
wasmrun run --path ./my-project
wasmrun run -p ./my-project
```

:::tip
Positional arguments take precedence over the `--path` flag if both are provided.
:::

### Port Configuration

Specify the server port for development:

```bash
wasmrun run --port 3000
wasmrun run -P 3000
```

Default port: `8420`

Valid range: `1-65535`

### Language Selection

Force a specific language for compilation:

```bash
wasmrun run --language rust
wasmrun run -l go
```

Supported languages:
- `rust` - Rust projects
- `go` - Go projects (using TinyGo)
- `c` - C/C++ projects
- `asc` - AssemblyScript projects
- `python` - Python projects

:::info
If not specified, Wasmrun will auto-detect the language based on project files.
:::

### Watch Mode

Enable live-reloading on file changes:

```bash
wasmrun run --watch
wasmrun run -W
```

### Serve Mode

Automatically open the UI in your default browser:

```bash
wasmrun run --serve
wasmrun run -s
```

## Exit Codes

Wasmrun uses standard exit codes:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (compilation, runtime, etc.) |

## Examples

### Start Development Server

```bash
# Current directory
wasmrun

# Specific project with custom port
wasmrun run ./my-rust-project --port 3000

# With watch mode and browser
wasmrun run ./my-project --watch --serve
```

### Execute WASM File

```bash
# Run a WASM file
wasmrun exec ./output.wasm

# With arguments
wasmrun exec ./calculator.wasm -- 5 10

# Call specific function
wasmrun exec ./math.wasm --call multiply -- 4 7
```

### Build for Production

```bash
# Compile with release optimization
wasmrun compile ./my-project --optimization release

# Compile to specific output directory
wasmrun compile ./my-project --output ./dist
```

### Plugin Management

```bash
# List all plugins
wasmrun plugin list

# Install a plugin
wasmrun plugin install wasmrust

# Get plugin info
wasmrun plugin info wasmgo
```

## Configuration Files

Wasmrun looks for configuration in:

- `wasmrun.toml` - Project-specific configuration
- `~/.wasmrun/` - Global plugin installation directory

## Getting Help

For detailed help on any command:

```bash
wasmrun [COMMAND] --help
```

For general help:

```bash
wasmrun --help
```

## Next Steps

- [run command](./run.md) - Development server
- [exec command](./exec.md) - Native execution
- [compile command](./compile.md) - Build projects
- [plugin command](./plugin.md) - Manage plugins
