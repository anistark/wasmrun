---
sidebar_position: 1
title: Overview
---

# Exec Mode

Wasmrun's exec mode (`wasmrun exec`) runs WebAssembly files natively using a built-in interpreter — no browser, no server, just direct execution.

## What It Does

Exec mode is a native WASM runtime that:

1. **Parses** the WASM binary (module sections, types, functions, memory, data)
2. **Initializes** linear memory and globals
3. **Links** WASI syscalls as host functions
4. **Executes** the entry point (`_start`, `main`, or a specified function)
5. **Returns** the exit code

```sh
wasmrun exec ./my-program.wasm
```

## When to Use

- Running CLI tools compiled to WASM
- Executing WASM modules without a browser
- Testing WASM binaries with argument passing
- Calling specific exported functions from a module

## Quick Example

```sh
# Run a WASM file
wasmrun exec ./hello.wasm

# Pass arguments
wasmrun exec ./program.wasm arg1 arg2 arg3

# Call a specific exported function
wasmrun exec ./math.wasm --call add 5 3
```

## Runtime Architecture

```
┌─ wasmrun exec ─────────────────────────────────────┐
│                                                     │
│  Module Parser  →  reads .wasm binary sections      │
│       ↓                                             │
│  Executor       →  interprets WASM bytecode         │
│       ↓                                             │
│  Linear Memory  →  64KB pages, bounds-checked       │
│       ↓                                             │
│  WASI Linker    →  host functions (fd_write, etc.)  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

The interpreter supports all standard WASM instructions: i32/i64/f32/f64 arithmetic, comparison, logic, memory operations, control flow (block/loop/if/br), function calls, and indirect calls via tables.
