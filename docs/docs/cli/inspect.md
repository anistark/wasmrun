# inspect

Perform detailed inspection on a WebAssembly file.

## Synopsis

```bash
wasmrun inspect <WASM_FILE>
```

## Description

The `inspect` command provides deep analysis of a WebAssembly module's structure, exports, imports, and metadata. Unlike `verify`, which checks validity, `inspect` shows detailed information about the module's contents.

Use this to:
- Understand module structure
- Find exported functions
- View imports and their types
- Analyze memory/table layout
- Debug integration issues
- Explore third-party WASM files

## Arguments

### `<WASM_FILE>`

Path to the WebAssembly file to inspect (required).

```bash
wasmrun inspect ./module.wasm
wasmrun inspect /path/to/output.wasm
```

You can also use the `--path` flag:

```bash
wasmrun inspect --path ./module.wasm
wasmrun inspect -p ./module.wasm
```

## Examples

### Basic Inspection

Inspect a WASM module:

```bash
wasmrun inspect ./output.wasm
```

Example output:

```
📦 WebAssembly Module: output.wasm

📊 Overview:
  Size: 245 KB (251,392 bytes)
  Format: WebAssembly 1.0
  Functions: 45
  Imports: 3
  Exports: 8
  Memory: 1 (min: 16 pages, max: 256 pages)
  Tables: 1 (min: 12 elements)
  Globals: 2

📤 Exports:
  • add (func) - Function index 0, signature: (i32, i32) -> i32
  • multiply (func) - Function index 1, signature: (i32, i32) -> i32
  • memory (memory) - Memory index 0
  • __data_end (global) - Global index 0, type: i32
  • __heap_base (global) - Global index 1, type: i32

📥 Imports:
  • env.abort (func) - signature: (i32, i32, i32, i32) -> ()
  • env.seed (func) - signature: () -> f64
  • env.memory (memory) - min: 0 pages

🔧 Functions:
  [0] add: (i32, i32) -> i32
  [1] multiply: (i32, i32) -> i32
  [2] subtract: (i32, i32) -> i32
  [3] divide: (i32, i32) -> i32
  ... (41 more functions)

💾 Memory:
  [0] min: 16 pages (1 MB), max: 256 pages (16 MB)

🗂️ Tables:
  [0] funcref, min: 12, max: unlimited

🌍 Globals:
  [0] __data_end: i32 (immutable)
  [1] __heap_base: i32 (immutable)

📝 Custom Sections:
  • name - Debug symbol names
  • producers - Toolchain information
```

### Find Specific Function

```bash
wasmrun inspect ./module.wasm | grep "my_function"
```

### List All Exports

```bash
wasmrun inspect ./module.wasm | grep -A 20 "Exports:"
```

### Check Memory Configuration

```bash
wasmrun inspect ./module.wasm | grep -A 5 "Memory:"
```

## Output Sections

### Overview

Basic module statistics:
- File size
- WASM version
- Count of functions, imports, exports
- Memory and table summary
- Global variables count

### Exports

Functions, memory, tables, and globals exported by the module:
- Name
- Type (func, memory, table, global)
- Index
- Signature (for functions)

### Imports

External dependencies required by the module:
- Module name (e.g., "env", "wasi_snapshot_preview1")
- Field name
- Type and signature

### Functions

All functions in the module:
- Index
- Name (if available)
- Signature (parameters and return types)

### Memory

Linear memory configuration:
- Initial size (minimum pages)
- Maximum size (if specified)
- Calculated sizes in bytes/KB/MB

### Tables

Function reference tables:
- Element type (usually funcref)
- Minimum size
- Maximum size

### Globals

Global variables:
- Name
- Type (i32, i64, f32, f64)
- Mutability (mutable/immutable)

### Custom Sections

Non-standard sections:
- `name` - Debug symbol names
- `producers` - Compiler/toolchain info
- `sourceMappingURL` - Source maps
- Custom application data

## Common Use Cases

### Find Entry Point

Look for exported main function:

```bash
wasmrun inspect ./app.wasm | grep -E "(main|_start|start)"
```

### Check WASI Imports

Verify WASI dependencies:

```bash
wasmrun inspect ./app.wasm | grep "wasi_snapshot_preview1"
```

### Verify Exported Functions

Ensure required functions are exported:

```bash
wasmrun inspect ./library.wasm | grep "Exports:"
```

### Debug Missing Imports

Find what imports are needed:

```bash
wasmrun inspect ./module.wasm | grep -A 50 "Imports:"
```

### Analyze Memory Requirements

Check memory configuration:

```bash
wasmrun inspect ./app.wasm | grep -A 3 "Memory:"
```

## Integration with Other Commands

### After Compilation

```bash
wasmrun compile ./project
wasmrun inspect ./output.wasm
```

### Before Execution

Understand what function to call:

```bash
wasmrun inspect ./module.wasm
wasmrun exec ./module.wasm --call my_function
```

### With Verification

```bash
wasmrun verify ./module.wasm
wasmrun inspect ./module.wasm
```

## Comparison with Other Tools

### wasm-objdump (WABT)

```bash
# WABT tool
wasm-objdump -x module.wasm

# Wasmrun equivalent
wasmrun inspect module.wasm
```

Wasmrun provides:
- Clearer, formatted output
- Better organization
- Easier to read

### wasm2wat

For full disassembly:

```bash
# wasm2wat shows full WAT format
wasm2wat module.wasm

# wasmrun inspect shows summary
wasmrun inspect module.wasm
```

## Function Signatures

Function signatures use WebAssembly types:

| Type | Description |
|------|-------------|
| `i32` | 32-bit integer |
| `i64` | 64-bit integer |
| `f32` | 32-bit float |
| `f64` | 64-bit float |
| `v128` | 128-bit vector (SIMD) |
| `funcref` | Function reference |
| `externref` | External reference |

Example signatures:
- `() -> i32` - No parameters, returns i32
- `(i32, i32) -> i32` - Two i32 params, returns i32
- `(f64) -> ()` - One f64 param, no return
- `(i32, i64, f32) -> i64` - Mixed params, returns i64

## Memory Pages

WebAssembly memory is measured in pages:
- 1 page = 64 KB
- Minimum: Initial allocation
- Maximum: Upper limit (optional)

Examples:
- `min: 1, max: 16` = 64 KB to 1 MB
- `min: 16, max: 256` = 1 MB to 16 MB
- `min: 256, max: unlimited` = 16 MB+

## Troubleshooting

### No Exports Found

```
📤 Exports:
  (none)
```

Cause: Module doesn't export any functions.

Solution:
- For Rust: Use `#[no_mangle]` and `pub extern "C"`
- For C: Use proper extern declarations
- Check compiler flags

### No Entry Point

Can't find main, _start, or start function.

Solution: Use `--call` with `exec`:

```bash
wasmrun inspect ./module.wasm  # Find function name
wasmrun exec ./module.wasm --call my_function
```

### Large Import List

Many imports from unknown modules.

Cause: Missing polyfills or runtime dependencies.

Solution: Ensure you have the proper WASM runtime or shims.

### Memory Too Large

```
💾 Memory:
  [0] min: 1024 pages (64 MB)
```

Cause: Excessive initial memory allocation.

Solution: Reduce initial memory in compiler settings.

## See Also

- [verify](./verify.md) - Validate WASM format
- [exec](./exec.md) - Execute WASM files
- [compile](./compile.md) - Compile projects to WASM
