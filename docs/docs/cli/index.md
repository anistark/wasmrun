# CLI Overview

Wasmrun provides a comprehensive command-line interface for working with WebAssembly projects.

## Command Structure

```sh
wasmrun [COMMAND] [OPTIONS] [ARGS]
```

### Basic Usage

```sh
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

```sh
wasmrun --debug run ./my-project
```

### `--help, -h`

Show help information for a command.

```sh
wasmrun --help
wasmrun run --help
```

### `--version, -V`

Display version information.

```sh
wasmrun --version
```

## Common Options

### Path Resolution

Wasmrun supports both positional and flag-based path arguments:

```sh
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

```sh
wasmrun run --port 3000
wasmrun run -P 3000
```

Default port: `8420`

Valid range: `1-65535`

### Language Selection

Force a specific language for compilation:

```sh
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

```sh
wasmrun run --watch
wasmrun run -W
```

### Serve Mode

Automatically open the UI in your default browser:

```sh
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

```sh
# Current directory
wasmrun

# Specific project with custom port
wasmrun run ./my-rust-project --port 3000

# With watch mode and browser
wasmrun run ./my-project --watch --serve
```

### Execute WASM File

```sh
# Run a WASM file
wasmrun exec ./output.wasm

# With arguments
wasmrun exec ./calculator.wasm -- 5 10

# Call specific function
wasmrun exec ./math.wasm --call multiply -- 4 7
```

### Build for Production

```sh
# Compile with release optimization
wasmrun compile ./my-project --optimization release

# Compile to specific output directory
wasmrun compile ./my-project --output ./dist
```

### Plugin Management

```sh
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

```sh
wasmrun [COMMAND] --help
```

For general help:

```sh
wasmrun --help
```

## Next Steps

- [run command](./run.md) - Development server
- [exec command](./exec.md) - Native execution
- [compile command](./compile.md) - Build projects
- [plugin command](./plugin.md) - Manage plugins
