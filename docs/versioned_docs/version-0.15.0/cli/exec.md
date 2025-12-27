# exec

Execute a WebAssembly file directly with native execution.

## Synopsis

```bash
wasmrun exec <WASM_FILE> [OPTIONS] [-- ARGS...]
```

## Description

The `exec` command runs a WebAssembly file natively without starting a development server. This is ideal for:

- Command-line tools built with WebAssembly
- Running compiled WASM binaries
- Testing WASM modules
- Integrating WASM programs into scripts

The command uses Wasmtime as the runtime engine with full WASI (WebAssembly System Interface) support.

## Arguments

### `<WASM_FILE>`

Path to the WebAssembly file to execute (required).

```bash
wasmrun exec ./output.wasm
wasmrun exec /path/to/module.wasm
```

### `[ARGS...]`

Arguments to pass to the WASM program. Use `--` to separate Wasmrun options from program arguments.

```bash
wasmrun exec ./calculator.wasm -- 5 10
wasmrun exec ./tool.wasm -- --input file.txt --output result.txt
```

## Options

### `-c, --call <FUNCTION>`

Specify which exported function to call. If not provided, Wasmrun looks for standard entry points in this order:

1. `main`
2. `_start`
3. `start`

```bash
wasmrun exec ./math.wasm --call multiply -- 4 7
wasmrun exec ./module.wasm -c custom_entry
```

## Entry Point Detection

When `--call` is not specified, Wasmrun automatically detects the entry point:

```
Checking for entry points:
  ✓ Found: main
  → Calling main()
```

If no standard entry point is found, you must specify one with `--call`.

## Examples

### Basic Execution

Run a WASM file:

```bash
wasmrun exec ./hello.wasm
```

### With Arguments

Pass arguments to the program:

```bash
wasmrun exec ./calculator.wasm -- 5 + 3
```

Output:
```
8
```

### Call Specific Function

Execute a specific exported function:

```bash
wasmrun exec ./math.wasm --call multiply -- 6 7
```

Output:
```
42
```

### Complex Arguments

Pass flags and options to the WASM program:

```bash
wasmrun exec ./tool.wasm -- --input data.json --format yaml --output result.yaml
```

### Pipe Input

Use with Unix pipes:

```bash
echo "hello world" | wasmrun exec ./processor.wasm
```

### In Scripts

Integrate into shell scripts:

```bash
#!/bin/bash
result=$(wasmrun exec ./compute.wasm -- $1 $2)
echo "Result: $result"
```

## WASI Support

The exec command provides full WASI support including:

### Filesystem Access

Access files in the current directory:

```rust
// Rust example
use std::fs;
let contents = fs::read_to_string("input.txt")?;
```

### Standard I/O

Read from stdin, write to stdout/stderr:

```rust
use std::io::{self, Read};
let mut input = String::new();
io::stdin().read_to_string(&mut input)?;
println!("You entered: {}", input);
```

### Environment Variables

Access environment variables:

```rust
use std::env;
if let Ok(value) = env::var("MY_VAR") {
    println!("MY_VAR = {}", value);
}
```

```bash
MY_VAR=hello wasmrun exec ./app.wasm
```

### Command-Line Arguments

Access arguments via WASI:

```rust
use std::env;
for arg in env::args() {
    println!("arg: {}", arg);
}
```

```bash
wasmrun exec ./app.wasm -- arg1 arg2 arg3
```

## Language-Specific Examples

### Rust

```rust
// src/main.rs
fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("Hello from Rust WASM!");
    println!("Arguments: {:?}", args);
}
```

Compile and run:

```bash
cargo build --target wasm32-wasi --release
wasmrun exec ./target/wasm32-wasi/release/my-app.wasm -- hello world
```

### Go (TinyGo)

```go
package main

import (
    "fmt"
    "os"
)

func main() {
    fmt.Println("Hello from Go WASM!")
    fmt.Printf("Arguments: %v\n", os.Args)
}
```

Compile and run:

```bash
tinygo build -target=wasi -o app.wasm main.go
wasmrun exec ./app.wasm -- hello world
```

### C

```c
#include <stdio.h>

int main(int argc, char *argv[]) {
    printf("Hello from C WASM!\n");
    for (int i = 0; i < argc; i++) {
        printf("arg[%d]: %s\n", i, argv[i]);
    }
    return 0;
}
```

Compile and run:

```bash
emcc main.c -o app.wasm
wasmrun exec ./app.wasm -- hello world
```

## Exit Codes

The exec command returns the exit code from the WASM program:

```bash
wasmrun exec ./app.wasm
echo $?  # Shows WASM program's exit code
```

- `0` - Success
- `1-255` - Error codes from WASM program
- `1` - Wasmrun error (file not found, invalid WASM, etc.)

## Current Limitations

### Not Supported

- **Network Access**: WASI networking APIs are not yet supported
- **Threading**: Multi-threading in WASM is not available
- **GPU Access**: Graphics APIs are not supported

### Workarounds

For features not supported in native execution:

1. **Web APIs**: Use `wasmrun run` for browser-based execution
2. **Networking**: Run in OS mode: `wasmrun os`
3. **Complex I/O**: Consider using the development server

## Performance

Native execution with `exec` is fast because:

- No compilation overhead (runs pre-compiled WASM)
- Direct execution via Wasmtime
- Minimal startup time
- No browser overhead

## Debugging

Enable debug output to see detailed execution information:

```bash
wasmrun --debug exec ./app.wasm
```

Debug output includes:
- Entry point detection
- Function calls
- WASI syscalls
- Execution flow

## Troubleshooting

### No Entry Point Found

```
❌ No entry point found in WASM file
```

Solution: Specify function with `--call`:

```bash
wasmrun exec ./module.wasm --call my_function
```

### Invalid WASM File

```
❌ Invalid WebAssembly file
```

Solution: Verify the file:

```bash
wasmrun verify ./module.wasm --detailed
```

### Function Not Found

```
❌ Function 'foo' not found in WASM module
```

Solution: Inspect available functions:

```bash
wasmrun inspect ./module.wasm
```

### WASI Compatibility

If you get WASI-related errors, ensure your WASM was compiled for WASI target:

```bash
# Rust
cargo build --target wasm32-wasi

# TinyGo
tinygo build -target=wasi

# C (Emscripten)
emcc -s WASI=1 main.c -o app.wasm
```

## See Also

- [run](./run.md) - Development server for web projects
- [compile](./compile.md) - Compile projects to WASM
- [verify](./verify.md) - Verify WASM files
- [inspect](./inspect.md) - Inspect WASM structure
- [WASI Integration](/docs/integrations/wasi) - WASI support details
