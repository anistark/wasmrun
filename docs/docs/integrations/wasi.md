---
sidebar_position: 4
title: WASI Support
description: WebAssembly System Interface for system-level access
---

# WASI Support

Wasmrun provides comprehensive support for WASI (WebAssembly System Interface), enabling WASM modules to interact with the host system for file I/O, environment variables, command-line arguments, and more.

## What is WASI?

WASI is a standardized system interface for WebAssembly that provides:
- **Portable syscalls** across different platforms
- **Security** through capability-based security model
- **System access** (files, network, time, random numbers)
- **Standard input/output** for CLI applications

## Supported WASI Syscalls

Wasmrun's native executor implements WASI preview1 syscalls:

### File System Operations
- `fd_read` - Read from file descriptors
- `fd_write` - Write to file descriptors
- `fd_close` - Close file descriptors
- `fd_seek` - Seek within files
- `path_open` - Open files and directories
- `fd_prestat_get` - Get preopened directory info
- `fd_prestat_dir_name` - Get preopened directory names

### Standard I/O
- **stdin** (fd 0) - Standard input
- **stdout** (fd 1) - Standard output
- **stderr** (fd 2) - Standard error

All stdio operations work seamlessly with terminal:
```bash
wasmrun exec app.wasm < input.txt > output.txt
```

### Environment & Arguments
- `environ_get` - Get environment variables
- `environ_sizes_get` - Get environment variable count/size
- `args_get` - Get command-line arguments
- `args_sizes_get` - Get argument count/size

### Time Operations
- `clock_time_get` - Get current time
- `clock_res_get` - Get clock resolution

### Random Numbers
- `random_get` - Generate cryptographically secure random numbers

### Process Management
- `proc_exit` - Exit process with code

## Usage Examples

### Command-Line Arguments

```rust
// Rust with WASI
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("Arguments: {:?}", args);
}
```

```bash
# Pass arguments to WASM
wasmrun exec app.wasm arg1 arg2 arg3
```

### Environment Variables

```rust
use std::env;

fn main() {
    match env::var("HOME") {
        Ok(val) => println!("HOME: {}", val),
        Err(e) => println!("Couldn't read HOME: {}", e),
    }
}
```

### File I/O

```rust
use std::fs;

fn main() {
    let content = fs::read_to_string("input.txt")
        .expect("Failed to read file");
    println!("Content: {}", content);
}
```

```bash
# File I/O works with native execution
wasmrun exec file-tool.wasm
```

### Standard Input/Output

```rust
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    println!("You entered: {}", input);
}
```

```bash
# Pipe input/output
echo "Hello" | wasmrun exec stdin-reader.wasm
```

## WASI Preview Versions

### WASI Preview 1 (Supported)
- Current stable API
- Fully implemented in wasmrun
- Used by most language toolchains

### WASI Preview 2 (Future)
- Not yet implemented
- Will include async I/O
- Broader system access

## Language Support

### Rust

```toml
# Cargo.toml
[dependencies]
# No additional dependencies needed

[profile.release]
strip = true
opt-level = "z"
lto = true
```

```bash
# Compile with wasm32-wasi target
rustup target add wasm32-wasi
cargo build --target wasm32-wasi --release

# Run with WASI support
wasmrun exec target/wasm32-wasi/release/app.wasm
```

### Go/TinyGo

```bash
# Compile with WASI target
tinygo build -target wasi -o app.wasm main.go

# Run with full WASI support
wasmrun exec app.wasm
```

### C/C++

```bash
# Compile with Emscripten WASI support
emcc -o app.wasm main.c -s STANDALONE_WASM=1

# Run natively
wasmrun exec app.wasm
```

## Security Model

WASI uses capability-based security:

### Preopened Directories
Only directories explicitly opened are accessible:
```bash
# Current directory is preopened by default
wasmrun exec app.wasm
```

### No Ambient Authority
- WASM modules can't access arbitrary files
- Must use provided file descriptors
- Network access controlled by namespace isolation

## Limitations

### Not Supported (Yet)
- **WASI Preview 2** features
- **Socket APIs** (use network isolation feature instead)
- **Async I/O** operations
- **Directory iteration** (limited support)

### Workarounds
For features not in WASI:
1. Use **OS Mode** for full runtime access
2. Use **dev server** for browser-based features
3. Implement in host and expose via WASI

## See Also

- [Native Execution](./native-execution.md) - WASM interpreter details
- [Network Isolation](./network-isolation.md) - Network capabilities
- [CLI exec Command](../cli/exec.md) - Execution reference
- [Language Guides](../guides/rust.md) - Language-specific WASI usage
