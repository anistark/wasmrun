---
sidebar_position: 5
title: inspect
---

# wasmrun inspect

Perform detailed inspection of a WebAssembly module's internals.

## Synopsis

```sh
wasmrun inspect [WASM_FILE] [OPTIONS]
```

## Description

The `inspect` command provides a deep analysis of a WASM binary's structure. While `verify` checks validity, `inspect` gives you a comprehensive view of what's inside the module — exports, imports, function signatures, memory layout, data sections, and more.

## Options

### `-p, --path <PATH>`

Path to the WASM file.

```sh
wasmrun inspect --path ./module.wasm
wasmrun inspect -p ./module.wasm
```

Positional argument:

```sh
wasmrun inspect ./module.wasm
```

## Output

Inspect produces a detailed breakdown:

```
🔍 Inspecting: module.wasm

📊 Overview:
   File size:       128,450 bytes
   WASM version:    1
   Sections:        11

📦 Type Section (12 types):
   [0] (i32, i32) → i32
   [1] (i32) → void
   [2] () → i32
   ...

📥 Import Section (4 imports):
   wasi_snapshot_preview1.fd_write    : function [type 3]
   wasi_snapshot_preview1.fd_read     : function [type 3]
   wasi_snapshot_preview1.proc_exit   : function [type 1]
   wasi_snapshot_preview1.args_get    : function [type 4]

📤 Export Section (3 exports):
   _start    : function [index 12]
   memory    : memory [index 0]
   alloc     : function [index 45]

💾 Memory Section:
   Initial: 16 pages (1 MB)
   Maximum: 256 pages (16 MB)

📊 Function Section: 89 functions
📊 Code Section: 89 function bodies (124,200 bytes)
📊 Data Section: 3 segments (4,100 bytes)

🏷️ Custom Sections:
   name         : 2,048 bytes
   producers    : 128 bytes
```

## Examples

### Basic Inspection

```sh
wasmrun inspect ./hello.wasm
```

### Compare Two Builds

```sh
# Debug build
wasmrun compile --optimization debug --output ./debug
wasmrun inspect ./debug/output.wasm

# Release build
wasmrun compile --optimization release --output ./release
wasmrun inspect ./release/output.wasm

# Compare sizes, function counts, etc.
```

### Check WASI Dependencies

Inspect the import section to see which WASI syscalls a module requires:

```sh
wasmrun inspect ./program.wasm
# Look at "Import Section" to see required host functions
```

### Identify Entry Points

```sh
wasmrun inspect ./module.wasm
# Check "Export Section" for _start, main, or other entry points
```

## Difference from verify

| | `verify` | `inspect` |
|---|---|---|
| **Purpose** | Is this a valid WASM file? | What's inside this WASM file? |
| **Output** | Pass/fail with error details | Full structural breakdown |
| **Use case** | CI checks, pre-deployment | Debugging, analysis, learning |
| **Speed** | Very fast (header + section scan) | Slightly slower (full parse) |

## See Also

- [verify](./verify.md) — quick validity check
- [compile](./compile.md) — compile projects to WASM
