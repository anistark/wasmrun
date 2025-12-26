---
sidebar_position: 2
title: Live Reload
description: Instant development feedback with file watching
---

# Live Reload

Wasmrun provides built-in file watching and live reload capabilities for instant development feedback. Changes to your source files automatically trigger recompilation and browser refresh.

## How It Works

The live reload system monitors your project directory for changes and:

1. **Detects** file modifications using the file system watcher
2. **Recompiles** your project when source files change
3. **Refreshes** the browser automatically with the new build
4. **Preserves** browser state when possible (hot module replacement)

## Enabling Live Reload

### Using the `--watch` Flag

```bash
# Enable live reload during development
wasmrun run ./my-project --watch

# With custom port
wasmrun run ./my-project --watch --port 3000

# Specify language explicitly
wasmrun run ./my-project --watch --language rust
```

### Default Behavior

By default, `wasmrun run` starts the development server **without** live reload. You must explicitly enable it with `--watch`.

```bash
# No live reload (manual refresh needed)
wasmrun run ./my-project

# With live reload (auto refresh on changes)
wasmrun run ./my-project --watch
```

## Supported File Types

Live reload monitors changes to:

### Source Files
- **Rust**: `*.rs`
- **Go**: `*.go`
- **Python**: `*.py`
- **C/C++**: `*.c`, `*.cpp`, `*.h`, `*.hpp`
- **AssemblyScript**: `*.ts`

### Configuration Files
- `Cargo.toml` (Rust)
- `go.mod`, `go.sum` (Go)
- `Makefile` (C/C++)
- `package.json`, `tsconfig.json` (AssemblyScript)
- `.wasmrun.toml` (Project config)

### Asset Files
- `*.html`, `*.css`, `*.js`
- `*.wasm` (pre-built modules)
- `*.json`, `*.toml`, `*.yaml`

## Performance Considerations

### Fast Recompilation

Live reload is most effective with:

- **Incremental compilation** (Rust, Go)
- **Small projects** (< 10,000 LOC)
- **Fast compilers** (TinyGo, waspy)

### Optimization Strategies

```bash
# Use debug builds during development (faster compilation)
wasmrun run ./my-project --watch --optimization debug

# Use release builds for final testing
wasmrun run ./my-project --optimization release
```

### Large Projects

For large projects:

1. **Split into modules**: Smaller compile units rebuild faster
2. **Use debug mode**: Release mode optimization takes longer
3. **Exclude unnecessary files**: Reduce watcher overhead
4. **Disable in production**: Use `compile` command for final builds

## Browser Integration

### Auto-Refresh

When live reload is enabled, wasmrun injects a WebSocket client into your page that:
- Connects to the wasmrun WebSocket server
- Listens for rebuild events
- Refreshes the page when builds complete
- Shows build errors in the browser console

### Build Status

The browser console shows live reload status:

```
[Wasmrun] Connected to live reload server
[Wasmrun] File changed: src/lib.rs
[Wasmrun] Rebuilding...
[Wasmrun] Build complete (2.3s)
[Wasmrun] Reloading...
```

## Error Handling

### Build Failures

When compilation fails:

1. **Error displayed** in terminal with detailed output
2. **Browser not refreshed** (keeps current working version)
3. **Notification shown** in browser console
4. **Retry automatic** when files change again

```
[Wasmrun] Build failed: compilation error
[Wasmrun] Fix the errors and save to rebuild
```

### Recovery

Fix the error and save the file - wasmrun automatically:
1. Detects the change
2. Attempts rebuild
3. Refreshes on success

## CLI Examples

```bash
# Basic live reload
wasmrun run ./rust-project --watch

# With specific port and language
wasmrun run ./my-project --watch --port 8080 --language go

# Verbose output to see all file changes
wasmrun run ./my-project --watch --verbose

# Debug mode for faster rebuilds
wasmrun run ./my-project --watch --optimization debug
```

## OS Mode with Live Reload

Live reload also works in OS mode:

```bash
# Node.js with live reload
wasmrun os ./node-app --watch --language nodejs

# Python with live reload
wasmrun os ./python-app --watch --language python
```

See [OS Mode](./os-mode.md) for more details.

## Troubleshooting

### Changes Not Detected

```bash
# Check if watcher is running (verbose mode shows file changes)
wasmrun run ./my-project --watch --verbose

# Verify file is in watched directories
```

### Slow Rebuilds

```bash
# Use debug optimization for faster builds
wasmrun run ./my-project --watch --optimization debug

# Check project size and split into modules
# Consider using incremental compilation in Cargo.toml
```

### Browser Not Refreshing

1. **Check WebSocket connection** in browser console
2. **Verify port is not blocked** by firewall
3. **Try different browser** (Chrome/Firefox recommended)
4. **Check terminal** for build errors

## See Also

- [CLI run Command](../cli/run.md) - Full `run` command reference
- [Quick Start](../quick-start.md) - Getting started guide
- [Troubleshooting](../troubleshooting.md) - Common issues and solutions
