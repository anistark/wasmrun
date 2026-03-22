---
sidebar_position: 4
title: Argument Passing
---

# Argument Passing

Arguments can be passed to WASM programs in two ways: as WASM function parameters (with `--call`) and as WASI program arguments (via `args_get`).

## Command-Line Syntax

```sh
# Arguments after the WASM file path
wasmrun exec ./program.wasm arg1 arg2 arg3

# Arguments with --call go to the function parameters
wasmrun exec ./math.wasm --call add 10 20

# Use -- to separate wasmrun flags from program arguments
wasmrun exec ./program.wasm -- --flag value
```

## Two Argument Paths

### 1. Function Parameters (with `--call`)

When using `--call`, arguments are converted to WASM values and passed directly to the function:

```sh
wasmrun exec ./math.wasm --call add 5 3
# → calls add(i32(5), i32(3))
```

**Conversion rules:**

| Input | Parsed As | WASM Value |
|---|---|---|
| `42` | Integer | `Value::I32(42)` |
| `-7` | Negative integer | `Value::I32(-7)` |
| `9876543210` | Large integer | `Value::I64(9876543210)` |
| `hello` | Non-numeric | `Value::I32(0)` |
| `3.14` | Non-integer | `Value::I32(0)` |

The executor attempts `i32` first, then `i64`. Non-numeric strings become `i32(0)`.

### 2. WASI Program Arguments (without `--call`)

When running a program normally (no `--call`), arguments are available via WASI syscalls:

```sh
wasmrun exec ./echo.wasm hello world
```

Inside the WASM program, these are accessible through:
- `args_sizes_get` — returns argument count and total buffer size
- `args_get` — writes argument strings into the program's memory

This is the standard WASI mechanism, equivalent to `argc`/`argv` in C.

**Example in Rust (compiled to WASI):**

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    for arg in &args {
        println!("{}", arg);
    }
}
```

```sh
wasmrun exec ./args_demo.wasm one two three
# ./args_demo.wasm
# one
# two
# three
```

## Examples

### Numeric Arguments

```sh
# Parsed as i32
wasmrun exec ./calc.wasm --call add 100 200

# Negative numbers
wasmrun exec ./calc.wasm --call subtract 50 -30

# Large numbers (parsed as i64)
wasmrun exec ./calc.wasm --call big_add 5000000000 3000000000
```

### String Arguments via WASI

```sh
# Program reads args via args_get
wasmrun exec ./grep.wasm "search term" file.txt

# Flags and options
wasmrun exec ./tool.wasm -- --verbose --output result.json
```

### Many Arguments

```sh
# Up to any number of arguments
wasmrun exec ./program.wasm $(seq 1 50)
```

### Special Characters

```sh
# Spaces (quote the argument)
wasmrun exec ./program.wasm "hello world"

# Paths
wasmrun exec ./program.wasm /path/to/file.txt

# Mixed
wasmrun exec ./program.wasm --name "John Doe" --age 30
```

## Limitations

- **No float arguments** — string arguments like `3.14` are not parsed as `f32`/`f64`. They become `i32(0)` when used as function parameters. Float values can only be passed through WASI args as strings.
- **No string function parameters** — WASM functions don't have a string type. Strings are passed via WASI args, not function parameters.
- **Argument encoding** — WASI args are null-terminated UTF-8 strings in linear memory.

## See Also

- [Function Calling](./functions.md) — `--call` flag details
- [Running WASM Files](./running.md) — entry point and execution flow
- [WASI Support](../wasi.md) — `args_get` and other syscalls
