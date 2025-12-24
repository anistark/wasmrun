---
sidebar_position: 3
title: Native Execution
description: Run WASM files directly with the native interpreter
---

# Native Execution

Wasmrun includes a complete WebAssembly interpreter that can execute compiled WASM files directly, without a browser or JavaScript runtime. This is perfect for CLI tools, test binaries, and server-side WASM applications.

## Overview

The `wasmrun exec` command provides:
- Direct WASM module execution
- Full argument passing via WASI
- Function selection with `-c/--call` flag
- Automatic entry point detection
- WASI syscall support
- Direct stdout/stderr output

## Basic Usage

```bash
# Execute WASM file natively (runs entry point)
wasmrun exec myapp.wasm

# Execute with arguments
wasmrun exec myapp.wasm arg1 arg2 arg3

# Call a specific exported function
wasmrun exec mylib.wasm -c add 5 3
wasmrun exec mylib.wasm --call multiply 4 6

# Stdout goes directly to terminal
wasmrun exec cli-tool.wasm --help
```

## Default vs Exec Mode

**Default Behavior** (`wasmrun file.wasm`):
- Starts development server on port 8420
- Loads WASM in browser environment
- Supports wasm-bindgen and DOM APIs

**Exec Mode** (`wasmrun exec file.wasm`):
- Bypasses server, executes directly
- Pure WASM interpreter
- CLI tool friendly
- No browser needed

## Function Selection

### Entry Point Detection

If no function is specified with `-c`, the runtime automatically finds and calls the entry point in this order:

1. **Start section** function
2. **`main`** export
3. **`_start`** export

```bash
# Auto-detects entry point
wasmrun exec myapp.wasm
```

### Explicit Function Call

Use `-c` or `--call` to invoke a specific exported function:

```bash
# Call specific function
wasmrun exec calculator.wasm -c add 5 3
wasmrun exec math.wasm --call fibonacci 10
```

## Argument Passing

Arguments are passed to your WASM program through WASI syscalls, just like command-line arguments in native applications.

```bash
# Pass arguments to main/start function
wasmrun exec tool.wasm --input file.txt --output result.txt

# Pass arguments to specific function
wasmrun exec tool.wasm -c process data.json --format json
```

## Supported Features

### ✅ Fully Supported

- **All arithmetic operations** (i32/i64/f32/f64)
- **Control flow** (if/else, loops, blocks)
- **Branching** (br, br_if)
- **Function calls** (direct and recursive)
- **Memory operations** (load, store, grow, size)
- **Local and global variables**
- **All comparison and unary operations**
- **WASI syscalls** (file I/O, arguments, environment, time)

### ⚠️ Current Limitations

**Language Runtime Requirements:**
- **Rust**: Functions may fail if they require panic hooks, stack unwinding, or memory allocators
- **Go/TinyGo**: Some functions require scheduler initialization or asyncify state

**Not Supported:**
- **wasm-bindgen** modules (require JavaScript runtime)
- Complex runtime features (async/await, exceptions)
- JavaScript interop

## Compatibility by Language

### ✅ Works Well

**Pure computational functions:**
```bash
# ✅ Pure WASM - perfect compatibility
wasmrun exec pure_math.wasm -c fibonacci 10

# ✅ Simple Rust/Go functions
wasmrun exec simple.wasm -c add 5 3

# ✅ CLI tools with minimal runtime
wasmrun exec tinygo-cli.wasm --help
```

### ⚠️ May Have Issues

**Complex applications:**
```bash
# ⚠️ May fail - complex Rust with panic handling
wasmrun exec complex.wasm -c process_data

# ✅ Alternative - use dev server for complex code
wasmrun ./my-rust-project
```

## Recommended Use Cases

### ✅ Best For

- **CLI tools** compiled with minimal runtime (TinyGo, pure Rust)
- **Pure computational functions** (math, algorithms, data processing)
- **Test binaries** with simple I/O
- **Hand-written WAT** files
- **C/C++ with Emscripten** `--no-entry` flag

### ⚠️ Use Dev Server Instead

- **Web applications** with DOM manipulation
- **wasm-bindgen** projects (Rust web apps)
- **Complex Rust** with panic handling/unwinding
- **Full Go runtime** features

## Workarounds

### For Rust

```bash
# Compile with wasm32-wasi target (no wasm-bindgen)
rustc --target wasm32-wasi main.rs -o app.wasm

# Use pure WASM crates, avoid web-sys/js-sys
```

### For Go

```bash
# Use TinyGo with WASI target
tinygo build -target wasi -o app.wasm main.go

# For complex apps, use dev server
wasmrun run ./go-project
```

### For Web Applications

```bash
# Use the dev server for browser-based WASM
wasmrun run ./my-web-app

# Not recommended for native execution
# wasmrun exec my-web-app.wasm  # Won't work
```

## Examples

### Simple Calculator

```bash
# Call different math functions
wasmrun exec calculator.wasm -c add 10 20
wasmrun exec calculator.wasm -c multiply 5 6
wasmrun exec calculator.wasm -c fibonacci 15
```

### CLI Tool

```bash
# Run as CLI tool with arguments
wasmrun exec tool.wasm --version
wasmrun exec tool.wasm process input.txt --format json
wasmrun exec tool.wasm --help
```

### Test Runner

```bash
# Execute test suite
wasmrun exec tests.wasm
wasmrun exec tests.wasm -c test_math
wasmrun exec tests.wasm -c test_io --verbose
```

## Troubleshooting

### "No entry point found"

- Ensure your WASM has `main()`, `_start()`, or exported functions
- Use `wasmrun inspect` to see available exports
- Specify function explicitly with `-c`

### "wasm-bindgen module detected"

- wasm-bindgen modules require JavaScript runtime
- Use dev server: `wasmrun run ./project`
- Or compile without wasm-bindgen for native execution

### Function Execution Fails

- Check if function requires runtime features
- Try with simpler, pure functions
- Use dev server for complex applications
- Enable debug mode: `WASMRUN_DEBUG=1 wasmrun exec file.wasm`

## See Also

- [CLI exec Command](../cli/exec.md) - Full command reference
- [WASI Support](./wasi-support.md) - WASI syscall details
- [Language Guides](../guides/rust.md) - Language-specific guidance
- [Troubleshooting](../troubleshooting.md) - Common issues
