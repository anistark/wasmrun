---
sidebar_position: 2
title: Features
---

# Exec Mode Features

## Native WASM Interpreter

A self-hosted WebAssembly interpreter written in Rust. No external runtime dependency; the interpreter is part of the wasmrun binary.

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
| **Multi-value** | Blocks with parameters and more than one result | ✅ Complete |
| **Sign extension** | i32/i64.extend8_s, extend16_s, extend32_s | ✅ Complete |
| **Non-trapping conversions** | trunc_sat (all eight forms) | ✅ Complete |
| **Bulk memory** | memory.copy/fill/init, data.drop, table.copy/fill/init, elem.drop | ✅ Complete |
| **Reference types** | funcref/externref, typed tables, table.get/set/size/grow | ✅ Complete |
| **SIMD, threads, WasmGC, typed function references, multi-memory** | - | ⬜ Not implemented |

### Components are detected, not misdiagnosed

The four bytes after a WebAssembly file's `\0asm` magic are a 16-bit version and a 16-bit *layer*. The layer is what separates a core module (`01 00 00 00`) from a Component Model binary (`0d 00 01 00`).

Wasmrun runs core modules. Handed a component, `exec`, `verify` and `inspect` all say so, name the version, and point at the `wasm32-wasip1` target. They previously read all four bytes as one number and reported "unsupported version 65549", or in `verify`'s case "missing magic bytes" for a file whose magic was perfectly fine.

Detection is all that ships here. The Component Model parser, the canonical ABI and the WASI 0.2/0.3 worlds are a separate milestone; see [issue #94](https://github.com/anistark/wasmrun/issues/94).

### Conformance

Wasmrun runs a subset of the [official WebAssembly spec test suite](https://github.com/WebAssembly/testsuite) in CI: 68 `.wast` files covering the core instruction set plus the proposals above, which is a little over 22,000 assertions. `just spec-suite` runs it locally and prints the per-file table.

The gate compares each file against a recorded baseline, so a regression fails the build and a fix is reported so the baseline gets tightened. Every remaining known failure is a proposal wasmrun does not implement (typed function references, multi-memory, WasmGC) or a `.wast` script that needs cross-module linking, which the harness does not do; those are listed with their reasons in `KNOWN_FAILURES` in `src/runtime/core/spec_suite.rs`.

The suite's `assert_invalid`, `assert_malformed` and `assert_unlinkable` directives are counted as skipped rather than passed. They assert that a *validator* rejects a bad module, and wasmrun has no validator: it assumes it is handed modules a toolchain already produced.

### Linear Memory

- 64KB pages with configurable initial and max sizes
- Bounds checking on every access
- Little-endian byte order (WASM standard)
- Support for memory.grow

## Limits

Guest code runs against ceilings the host sets, so a runaway program fails its own execution rather than the process:

- **Fuel**: an instruction budget, shared across the whole call tree
- **Wall clock**: a cancellation flag the interpreter checks between instructions, and that a sleeping `poll_oneoff` checks while it waits
- **Call depth**: 1024 nested guest calls by default. The interpreter runs each guest call on a host stack frame, so unbounded recursion would otherwise overflow the host stack, which is a process abort rather than a trap
- **Memory**: a page ceiling per module, on top of the 65536-page (4 GiB) limit a 32-bit memory has by definition

## WASI Preview 1

See [WASI Support](./wasi) for the full syscall table. In short: standard I/O with caller-supplied stdin, a real filesystem under the preopened directories, arguments and environment, clocks, random, `poll_oneoff` for timed waits, and `proc_exit`. `path_symlink` is the one syscall still unimplemented.

## Entry Point Detection

The executor automatically finds the entry point by checking (in order):

1. **Start section**: WASM module's designated start function
2. **`_start` export**: WASI convention
3. **`main` export**: common convention

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
