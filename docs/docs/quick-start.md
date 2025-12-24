---
sidebar_position: 3
---

# Quick Start

Get up and running with Wasmrun in just a few minutes. This guide will walk you through creating and running your first WebAssembly project.

## Prerequisites

Before you begin, make sure you have:

- Wasmrun installed (see [Installation](./installation.md))
- Basic knowledge of command-line tools

## Your First WASM Project

Let's create a simple WebAssembly project using Rust. Don't worry if you prefer another language - the process is similar for all supported languages.

### Step 1: Install the Rust Plugin

```bash
# Install the Rust plugin for Wasmrun
wasmrun plugin install wasmrust
```

This downloads and installs the `wasmrust` plugin, which adds Rust support to Wasmrun.

### Step 2: Create a New Project

Create a new directory for your project:

```bash
mkdir my-first-wasm
cd my-first-wasm
```

Create a `Cargo.toml` file:

```toml
[package]
name = "my-first-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
```

Create `src/lib.rs` with a simple function:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to WebAssembly.", name)
}

#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

### Step 3: Run the Development Server

Start the development server with live reload:

```bash
wasmrun run . --watch
```

Wasmrun will:
1. Detect that this is a Rust project
2. Compile your code to WebAssembly
3. Start a development server at `http://localhost:8420`
4. Watch for file changes and auto-reload

### Step 4: Test Your WASM Module

Open your browser to `http://localhost:8420` and you'll see the Wasmrun interface. You can test your functions in the browser console:

```javascript
// Load the WASM module
import init, { greet, add } from './my_first_wasm.js';

await init();

// Call your functions
console.log(greet('World'));  // "Hello, World! Welcome to WebAssembly."
console.log(add(5, 3));        // 8
```

### Step 5: Try Native Execution

You can also run WASM modules natively without a browser. First, compile a standalone WASM module:

```bash
wasmrun compile .
```

Then execute it natively:

```bash
wasmrun exec ./target/wasm32-unknown-unknown/release/my_first_wasm.wasm --call add 10 20
```

## Running Existing Examples

Wasmrun comes with several example projects that demonstrate different features and languages. Clone the repository to try them:

```bash
# Clone the repository
git clone https://github.com/anistark/wasmrun.git
cd wasmrun/examples
```

### Rust Examples

```bash
# Simple Rust hello world
cd rust-hello
wasmrun run .

# Rust web application with Leptos
cd ../web-leptos
wasmrun run . --watch
```

### Go Examples

```bash
# Install Go plugin first
wasmrun plugin install wasmgo

# Simple Go example
cd go-hello
wasmrun run .

# Native execution example
cd ../native-go
wasmrun run .
wasmrun exec ./path/to/compiled.wasm --call main
```

### Python Example

```bash
# Install Python plugin first
wasmrun plugin install waspy

# Python example
cd python-hello
wasmrun run .
```

### C/C++ Example

```bash
# C example (no plugin needed - built-in support)
cd c-hello
wasmrun run .
```

### AssemblyScript Examples

```bash
# Install AssemblyScript plugin first
wasmrun plugin install wasmasc

# Simple AssemblyScript example
cd asc-hello
wasmrun run .

# Web application example
cd ../web-asc
wasmrun run . --watch
```

## Common Commands

Here are the most common commands you'll use:

### Development Server

```bash
# Run with auto-reload
wasmrun run ./my-project --watch

# Run on a specific port
wasmrun run ./my-project --port 3000

# Specify language explicitly
wasmrun run ./my-project --language rust
```

### Compilation

```bash
# Compile project
wasmrun compile ./my-project

# Compile with optimization
wasmrun compile ./my-project --optimization release

# Verbose output
wasmrun compile ./my-project --verbose
```

### Native Execution

```bash
# Execute WASM file
wasmrun exec myfile.wasm

# Execute with arguments
wasmrun exec myfile.wasm arg1 arg2

# Call specific function
wasmrun exec myfile.wasm --call add 5 3
```

### Plugin Management

```bash
# List all plugins
wasmrun plugin list

# Get plugin info
wasmrun plugin info wasmrust

# Install plugin
wasmrun plugin install wasmrust

# Uninstall plugin
wasmrun plugin uninstall wasmrust
```

### Inspection & Verification

```bash
# Verify WASM file
wasmrun verify myfile.wasm

# Detailed verification
wasmrun verify myfile.wasm --detailed

# Inspect WASM structure
wasmrun inspect myfile.wasm
```

## Next Steps

Now that you've created your first WebAssembly project, here's what to explore next:

### Learn More About Your Language

- **[Rust Guide](./guides/rust.md)** - Deep dive into Rust and WebAssembly
- **[Go Guide](./guides/go.md)** - Using TinyGo for WebAssembly
- **[Python Guide](./guides/python.md)** - Python to WebAssembly with waspy
- **[C/C++ Guide](./guides/c-cpp.md)** - Traditional C/C++ compilation
- **[AssemblyScript Guide](./guides/assemblyscript.md)** - TypeScript-like WebAssembly

### Explore Features

- **[Plugin System](./features/plugin-system.md)** - Understand the plugin architecture
- **[Live Reload](./features/live-reload.md)** - Development workflow optimization
- **[Native Execution](./features/native-execution.md)** - Run WASM without a browser
- **[WASI Support](./features/wasi-support.md)** - System interface capabilities
- **[OS Mode](./features/os-mode.md)** - Multi-language browser environment

### Reference Documentation

- **[CLI Reference](./cli/overview.md)** - Complete command-line documentation
- **[Troubleshooting](./troubleshooting.md)** - Common issues and solutions

## Tips for Success

1. **Use Live Reload** - The `--watch` flag makes development much faster
2. **Start Simple** - Begin with basic examples before complex applications
3. **Check Plugin Status** - Use `wasmrun plugin list` to see what's installed
4. **Read Error Messages** - Wasmrun provides helpful error messages and suggestions
5. **Explore Examples** - The example projects demonstrate best practices

## Getting Help

If you run into issues:

- Check the [Troubleshooting](./troubleshooting.md) guide
- Review the [CLI Reference](./cli/overview.md) for command details
- Visit [GitHub Issues](https://github.com/anistark/wasmrun/issues)
- Join the [Discussions](https://github.com/anistark/wasmrun/discussions)

Happy WebAssembly development!
