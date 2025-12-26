---
sidebar_position: 1
title: Plugin System
description: Wasmrun's modular plugin architecture for multi-language support
---

# Plugin System

Wasmrun's modular plugin architecture enables seamless integration of different programming languages and compilation toolchains into a unified development experience.

## Architecture Overview

The plugin system is designed with these core principles:

- **Trait-based**: All plugins implement the same `Plugin` and `WasmBuilder` traits
- **Dynamic Loading**: External plugins are loaded at runtime via dynamic libraries
- **Configuration-driven**: Plugin behavior is controlled through structured configuration
- **Consistent Interface**: Both built-in and external plugins use identical interfaces

Read more in the detailed [wasmrun plugin architecture](https://blog.anirudha.dev/wasmrun-plugin-architecture) guide.

## Plugin Types

### Built-in Plugins

Built-in plugins are compiled directly into Wasmrun and provide core language support.

| Plugin | Language | Compiler | Status | Capabilities |
|--------|----------|----------|---------|--------------|
| **C/C++** | C, C++ | Emscripten | ✅ Stable | Full WASM + Web Apps + Makefiles |

**Advantages:**
- No installation required
- Always available
- Guaranteed compatibility
- Direct integration with core

### External Plugins

External plugins are distributed via crates.io and installed dynamically to `~/.wasmrun/`.

| Plugin | Language | Compiler | Installation | Capabilities |
|--------|----------|----------|-------------|--------------|
| **wasmasc** | AssemblyScript | `asc` | `wasmrun plugin install wasmasc` | WASM + Optimization + npm/yarn/pnpm/bun |
| **wasmrust** | Rust | `rustc` + `wasm-pack` | `wasmrun plugin install wasmrust` | Full WASM + Web Apps + Optimization |
| **wasmgo** | Go | TinyGo | `wasmrun plugin install wasmgo` | WASM + Optimization + Package Support |
| **waspy** | Python | waspy | `wasmrun plugin install waspy` | WASM + Python-to-WASM Compilation |

**How External Plugins Work:**
- 📦 **Cargo-like Installation**: Similar to `cargo install`, plugins are downloaded and compiled to `~/.wasmrun/`
- 🔗 **Dynamic Loading**: Plugins are loaded as shared libraries (FFI) at runtime
- 🎯 **Same Interface**: External plugins use identical traits as built-in plugins
- 🔧 **Auto-detection**: Once installed, plugins automatically handle their supported project types

## Installation and Management

### Installing Plugins

```bash
# Install external plugins
wasmrun plugin install wasmrust   # Rust support
wasmrun plugin install wasmgo     # Go support
wasmrun plugin install waspy      # Python support
wasmrun plugin install wasmasc    # AssemblyScript support
```

**Installation Process:**

1. **Discovery**: Searches crates.io for the plugin
2. **Download**: Uses `cargo install` to build the plugin
3. **Storage**: Installs to `~/.wasmrun/plugins/{plugin_name}/`
4. **Registration**: Updates wasmrun config with plugin capabilities
5. **Ready**: Plugin automatically handles supported projects

### Listing Plugins

```bash
# List all available plugins (built-in + installed)
wasmrun plugin list

# Show detailed information about a specific plugin
wasmrun plugin info wasmrust
```

Output includes:
- Plugin name and version
- Type (built-in or external)
- Supported file extensions
- Required system dependencies
- Installation status
- Capabilities

### Managing Plugins

```bash
# Uninstall a plugin
wasmrun plugin uninstall <plugin-name>

# Update installed plugins
wasmrun plugin update <plugin-name>

# Update all plugins
wasmrun plugin update --all
```

## Plugin Installation Location

External plugins are installed to:
```
~/.wasmrun/
├── config.toml              # Global configuration
├── plugins/                 # Plugin installations
│   ├── wasmrust/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   ├── target/release/
│   │   │   └── libwasmrust.dylib
│   │   └── .wasmrun_metadata
│   └── wasmgo/
│       └── ...
├── cache/                   # Build cache
└── logs/                    # Plugin logs
```

## Plugin Capabilities

Each plugin declares its capabilities:

- **compile_wasm**: Can compile to standard .wasm files
- **compile_webapp**: Can bundle web applications
- **live_reload**: Supports hot reload during development
- **optimization**: Supports size/speed optimization passes
- **custom_targets**: Additional compilation targets

## Auto-detection

Once installed, plugins automatically detect their supported projects based on:

- **File extensions** (`.rs`, `.go`, `.py`, `.c`, `.cpp`, `.ts`)
- **Configuration files** (`Cargo.toml`, `go.mod`, `Makefile`, `package.json`)
- **Entry point files** (`main.rs`, `main.go`, `main.py`, etc.)

You can override detection with the `--language` flag:

```bash
wasmrun --language rust ./my-project
```

## Creating Custom Plugins

See [Creating Plugins](../development/creating-plugins.md) for a comprehensive guide on:

- Plugin trait implementation
- WasmBuilder trait
- External plugin distribution
- Best practices
- Testing strategies

## Troubleshooting

### Plugin Not Available

```bash
# For built-in plugins
wasmrun --language c        # C/C++ (built-in)

# For external plugins, install first
wasmrun plugin install wasmrust
```

### Plugin Dependencies Missing

```bash
# Check what dependencies are required
wasmrun plugin info wasmrust

# Install missing tools
rustup target add wasm32-unknown-unknown  # For wasmrust
```

### Wrong Plugin Selected

```bash
# Force a specific plugin
wasmrun --language rust
wasmrun --language go
```

## See Also

- [Language Guides](../guides/rust.md) - Detailed guides for each language
- [Creating Plugins](../development/creating-plugins.md) - Building custom plugins
- [CLI Reference](../cli/plugin.md) - Complete plugin command reference
