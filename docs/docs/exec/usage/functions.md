---
sidebar_position: 3
title: Function Calling
---

# Calling Exported Functions

## The `--call` Flag

Use `-c` or `--call` to invoke a specific exported function by name instead of the default entry point:

```sh
wasmrun exec ./module.wasm --call <FUNCTION_NAME> [ARGS...]
wasmrun exec ./module.wasm -c <FUNCTION_NAME> [ARGS...]
```

## How It Works

1. The executor parses the module and looks up the export by name
2. Verifies the export is a function (not a memory, table, or global)
3. Reads the function's type signature (parameter types and return types)
4. Pops the provided arguments from the command line, converts them to WASM values
5. Calls the function and returns the result

## Examples

### Call a Simple Function

```sh
# Module exports: add(i32, i32) → i32
wasmrun exec ./math.wasm --call add 10 20
```

### Call Without Arguments

```sh
# Module exports: get_version() → i32
wasmrun exec ./lib.wasm --call get_version
```

### Call a Void Function

```sh
# Module exports: initialize() → void
wasmrun exec ./app.wasm --call initialize
```

### Multiple Functions in One Module

A single WASM module can export many functions. Use `inspect` to discover them:

```sh
# See all exports
wasmrun inspect ./math.wasm
# 📤 Exports:
#    add       : function [index 0]
#    multiply  : function [index 1]
#    factorial : function [index 2]
#    _start    : function [index 3]

# Call any of them
wasmrun exec ./math.wasm --call add 5 3
wasmrun exec ./math.wasm --call multiply 6 7
wasmrun exec ./math.wasm --call factorial 10
```

## Error Handling

### Function Not Found

```sh
wasmrun exec ./module.wasm --call nonexistent
# ❌ Exported function 'nonexistent' not found in WASM module
```

### Not a Function Export

If the name maps to a memory or table export rather than a function, you'll get an error. Use `wasmrun inspect` to verify the export type.

### Wrong Number of Arguments

The executor passes whatever arguments are provided. If the function signature requires more parameters than given, remaining parameters default to zero. Extra arguments are ignored after conversion.

## Discovering Exports

Before calling functions, inspect the module to see what's available:

```sh
wasmrun inspect ./module.wasm
```

Look at the **Export Section** for function names, and the **Type Section** for their signatures.

## Use Cases

### Testing Individual Functions

```sh
# Test each exported function during development
wasmrun exec ./lib.wasm --call validate_input 42
wasmrun exec ./lib.wasm --call process_data 100
wasmrun exec ./lib.wasm --call cleanup
```

### Library Modules

Modules that export utility functions rather than a `_start` entry point:

```sh
# No _start — must use --call
wasmrun exec ./crypto.wasm --call hash_sha256 12345
```

### Benchmarking

```sh
# Time a specific function
time wasmrun exec ./compute.wasm --call heavy_computation 1000000
```

## See Also

- [Running WASM Files](./running.md) — default entry point behavior
- [Argument Passing](./arguments.md) — how arguments are converted
