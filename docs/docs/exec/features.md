---
sidebar_position: 2
title: Features
---

# Exec Mode Features

## Native WASM Interpreter

A self-hosted WebAssembly interpreter written in Rust. No external runtime dependency — the interpreter is part of the wasmrun binary.

### Instruction Support

| Category | Instructions | Status |
|---|---|---|
| **i32 arithmetic** | add, sub, mul, div, rem, clz, ctz, popcnt | ✅ Complete |
| **i64 arithmetic** | add, sub, mul, div, rem, clz, ctz, popcnt | ✅ Complete |
| **f32 arithmetic** | add, sub, mul, div, sqrt, min, max, ceil, floor, trunc, nearest, abs, neg, copysign | ✅ Complete |
| **f64 arithmetic** | add, sub, mul, div, sqrt, min, max, ceil, floor, trunc, nearest, abs, neg, copysign | ✅ Complete |
| **Comparison** | eq, ne, lt, gt, le, ge (all types, signed/unsigned) | ✅ Complete |
| **Logic** | and, or, xor, shl, shr, rotl, rotr (i32/i64) | ✅ Complete |
| **Memory** | load, store (all widths: 8/16/32/64, signed/unsigned), memory.size, memory.grow | ✅ Complete |
| **Control flow** | block, loop, if/else, br, br_if, br_table, return, call, call_indirect, select, nop, unreachable | ✅ Complete |
| **Variables** | local.get/set/tee, global.get/set | ✅ Complete |
| **Type conversions** | wrap, extend, trunc, convert, demote, promote, reinterpret | ✅ Complete |
| **Data sections** | Active data segment initialization into linear memory | ✅ Complete |

### Linear Memory

- 64KB pages with configurable initial and max sizes
- Bounds checking on every access
- Little-endian byte order (WASM standard)
- Support for memory.grow

## WASI Preview 1

Basic WASI syscall support for system interaction:

- `fd_write` — write to stdout/stderr
- `fd_read` — read from stdin
- `environ_get` / `environ_sizes_get` — environment variables
- `args_get` / `args_sizes_get` — command-line arguments
- `clock_time_get` — real-time and monotonic clocks
- `random_get` — random bytes
- `proc_exit` — process exit

## Entry Point Detection

The executor automatically finds the entry point by checking (in order):

1. **Start section** — WASM module's designated start function
2. **`_start` export** — WASI convention
3. **`main` export** — common convention

Or you can specify a function explicitly with `--call`.

## Argument Passing

Arguments are parsed and converted to WASM values:

- Integer strings → `Value::I32` or `Value::I64`
- Everything else → `Value::I32(0)` (fallback)

Arguments are also available to the WASM program via WASI's `args_get` syscall.

## Function Selection

Call any exported function by name:

```sh
wasmrun exec ./math.wasm --call multiply 6 7
```

The executor looks up the export, validates the signature, and invokes it with the provided arguments.
