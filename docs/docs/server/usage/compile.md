---
sidebar_position: 3
title: compile
---

# wasmrun compile

Compile a project to WebAssembly without starting a server.

## Synopsis

```sh
wasmrun compile [PROJECT] [OPTIONS]
```

**Aliases:** `build`, `c`

## Description

The `compile` command builds your project to WebAssembly using the detected language plugin. Unlike `run`, it does not start a dev server — it only produces the compiled output. Useful for CI/CD pipelines, production builds, and scripting.

## Options

### `-p, --path <PATH>`

Path to the project directory.

```sh
wasmrun compile --path ./my-project
wasmrun compile -p ./my-project
```

Default: current directory (`.`)

Positional argument also works:

```sh
wasmrun compile ./my-project
```

### `-o, --output <DIR>`

Output directory for the compiled WASM file.

```sh
wasmrun compile --output ./dist
wasmrun compile -o ./build
```

Default: current directory (`.`)

The directory is created automatically if it doesn't exist.

### `--optimization <LEVEL>`

Compilation optimization level.

```sh
wasmrun compile --optimization release
wasmrun compile --optimization debug
wasmrun compile --optimization size
```

| Level | Description | Use Case |
|---|---|---|
| `debug` | No optimizations, fast compile, larger output | Development, debugging |
| `release` | Full optimizations, slower compile, smaller output | Production (default) |
| `size` | Optimize for smallest binary size | Bandwidth-constrained deployments |

Default: `release`

### `-v, --verbose`

Show detailed compilation output.

```sh
wasmrun compile --verbose
```

Displays the underlying compiler commands (e.g., `cargo build`, `tinygo build`, `emcc`), timing information, and generated file paths.

## Examples

### Basic Compilation

```sh
# Compile current directory
wasmrun compile

# Compile a specific project
wasmrun compile ./my-rust-project
```

### Production Build

```sh
wasmrun compile ./my-project --optimization release --output ./dist
```

### Size-Optimized Build

```sh
wasmrun compile ./my-project --optimization size --output ./dist
# Output: dist/my_project.wasm (minimized)
```

### Debug Build

```sh
wasmrun compile ./my-project --optimization debug --verbose
# Faster compilation, includes debug info
```

### CI/CD Pipeline

```sh
#!/bin/bash
set -e

# Clean previous build
wasmrun clean

# Compile for production
wasmrun compile ./my-project \
  --optimization release \
  --output ./dist

# Verify the output
wasmrun verify ./dist/output.wasm --detailed

# Deploy
cp ./dist/output.wasm /deploy/path/
```

### Cross-Language

```sh
# Rust (requires wasmrust plugin)
wasmrun compile ./rust-project

# Go (requires wasmgo plugin)
wasmrun compile ./go-project

# C/C++ (built-in Emscripten support)
wasmrun compile ./c-project

# AssemblyScript (requires wasmasc plugin)
wasmrun compile ./asc-project
```

## Output

Compilation produces:

- **Standard WASM** — `output.wasm` (all languages)
- **wasm-bindgen** — `output_bg.wasm` + `output.js` (Rust with wasm-bindgen)

The exact output name depends on the project configuration (e.g., `Cargo.toml` package name for Rust).

## See Also

- [run](./run.md) — compile and serve in one step
- [verify](./verify.md) — validate compiled output
- [clean](./clean.md) — remove build artifacts
