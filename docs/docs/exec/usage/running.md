---
sidebar_position: 2
title: Running WASM Files
---

# Running WASM Files

## Basic Execution

Point `exec` at any valid `.wasm` file:

```sh
wasmrun exec ./hello.wasm
```

wasmrun will:
1. Read and parse the WASM binary
2. Initialize linear memory and globals
3. Link WASI host functions
4. Find an entry point and execute it
5. Print output to the terminal and return the exit code

## Entry Point Detection

When no `--call` flag is given, the executor searches for an entry point in this order:

### 1. Start Section

The WASM spec allows a module to declare a start function that runs automatically on instantiation. If the module has one, it's used.

### 2. `_start` Export

The WASI convention. Programs compiled with `--target wasm32-wasi` export a `_start` function:

```sh
# Rust compiled to WASI
cargo build --target wasm32-wasi --release
wasmrun exec ./target/wasm32-wasi/release/my_program.wasm
# → calls _start
```

### 3. `main` Export

Some compilers export `main` instead of `_start`. The executor checks for this as a fallback.

### No Entry Point

If none of the above are found:

```
❌ No entry point found (checked: start section, main, _start)
```

Use `--call` to specify a function explicitly, or use `wasmrun inspect` to see what the module exports.

## Execution Output

Standard output and error go directly to the terminal:

```sh
wasmrun exec ./hello.wasm
# Hello, World!
```

- **stdout** (fd 1) — printed normally
- **stderr** (fd 2) — printed to stderr
- **Exit code** — returned as the process exit code

## File Validation

The executor validates the file before running:

```sh
# Not a .wasm file
wasmrun exec ./script.py
# ❌ Expected a .wasm file, got: script.py

# File doesn't exist
wasmrun exec ./missing.wasm
# ❌ WASM file not found: missing.wasm

# Corrupt binary
wasmrun exec ./corrupt.wasm
# ❌ Failed to parse WASM module: Invalid magic number
```

## Examples

### Rust WASI Program

```sh
# Compile
cargo build --target wasm32-wasi --release

# Run
wasmrun exec ./target/wasm32-wasi/release/my_cli.wasm
```

### Go WASM Program

```sh
# Compile with TinyGo
tinygo build -target wasi -o hello.wasm .

# Run
wasmrun exec ./hello.wasm
```

### Verify Then Run

```sh
wasmrun verify ./program.wasm && wasmrun exec ./program.wasm
```

### Inspect Before Running

```sh
# Check what's exported
wasmrun inspect ./module.wasm

# Then run the right function
wasmrun exec ./module.wasm --call process_data
```

## See Also

- [Function Calling](./functions.md) — call specific exports
- [Argument Passing](./arguments.md) — pass data to programs
- [WASI Support](../wasi.md) — available syscalls
