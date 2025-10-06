# Wasmrun Examples

This directory contains example projects demonstrating how to create WebAssembly modules using different programming languages with Wasmrun.

> **Note**: These examples are standalone projects for learning and testing. They are not integrated into the main wasmrun CLI but can be run directly using standard wasmrun commands.

## Available Examples

### 🦀 Rust (`rust-hello/`)
- **Features**: wasm-bindgen integration, console logging, function exports
- **Functions**: `greet()`, `fibonacci()`, `sum_array()`
- **Build**: Uses Cargo with wasm-bindgen

### 🐹 Go (`go-hello/`)
- **Features**: syscall/js integration, JavaScript interop
- **Functions**: `greet()`, `fibonacci()`, `sumArray()`, `getCurrentTime()`
- **Build**: Uses Go with GOOS=js GOARCH=wasm

### 🔧 C (`c-hello/`)
- **Features**: Emscripten integration, memory management, math library
- **Functions**: `greet()`, `fibonacci()`, `factorial()`, `is_prime()`, `square_root()`
- **Build**: Uses Emscripten with exported functions

### 🚀 AssemblyScript (`asc-hello/`)
- **Features**: TypeScript-like syntax, typed arrays, performance optimization
- **Functions**: `greet()`, `fibonacci()`, `isPrime()`, `reverseString()`, `power()`
- **Build**: Uses AssemblyScript compiler

### 🐍 Python (`python-hello/`)
- **Features**: Python-to-WebAssembly compilation, type annotations, no runtime required
- **Functions**: `greet()`, `add()`, `fibonacci()`, `factorial()`
- **Build**: Uses waspy (Python to WASM compiler)

### 🌐 AssemblyScript Web App (`web-asc/`)
- **Features**: Interactive web application, DOM manipulation, real-time calculations
- **Functions**: `greet()`, `fibonacci()`, `isPrime()`, `reverseString()`, `power()`, `validateEmail()`, `calculateArea()`
- **Build**: AssemblyScript with web interface and performance testing

### 🦀 Leptos Web App (`web-leptos/`)
- **Features**: Reactive components, client-side routing, state management
- **Functions**: Counter, todo list, interactive forms with Leptos framework
- **Build**: Rust Leptos framework compiled to WebAssembly

## Quick Start

Examples work with standard wasmrun commands:

```sh
# Run any example (from wasmrun project root)
wasmrun run examples/rust-hello
wasmrun run examples/go-hello
wasmrun run examples/python-hello
wasmrun run examples/c-hello
wasmrun run examples/asc-hello
wasmrun run examples/web-asc
wasmrun run examples/web-leptos

# Compile only
wasmrun compile examples/rust-hello
wasmrun compile examples/python-hello

# Run with options
wasmrun run examples/go-hello --port 3000 --watch
```

## Example Structure

Each example contains:
- **Source code** in the appropriate language
- **Build configuration** (Cargo.toml, go.mod, package.json, etc.)
- **README.md** with specific usage instructions
- **Function descriptions** and JavaScript usage examples

## Language-Specific Notes

### Rust
- Requires `wasm-pack` or `wasm-bindgen-cli`
- Produces `.wasm` and `.js` files
- Best for performance-critical applications

### Go
- Requires Go 1.21+ with WebAssembly support
- Produces single `.wasm` file with `wasm_exec.js`
- Good for concurrent operations and system programming

### C
- Uses Emscripten for compilation
- Full control over memory management
- Best for system-level programming and existing C libraries

### AssemblyScript
- TypeScript-like syntax with WebAssembly performance
- Produces optimized `.wasm` files
- Good balance between ease of use and performance
- `web-asc` demonstrates interactive web applications with real-time calculations

### Leptos (Rust Web Framework)
- Reactive component system with fine-grained updates
- Client-side routing and state management
- Full-stack Rust development compiled to WebAssembly
- `web-leptos` showcases modern web app architecture with Rust

### Python
- Requires `waspy` plugin: `wasmrun plugin install waspy`
- Pure Rust-based compiler (no Python runtime needed)
- Produces optimized `.wasm` files
- Supports type annotations and basic Python syntax
- Good for educational purposes and lightweight Python functions

## Contributing

To add a new example:

1. Create a new directory in `examples/`
2. Add source code and build configuration
3. Create a `README.md` with usage instructions
4. Update this main `README.md` file
5. Test with `wasmrun run examples/your-example`

## Common Functions

All examples implement these common functions for consistency:

- `greet(name)` - Basic greeting with the provided name
- `fibonacci(n)` - Calculate the nth Fibonacci number
- `add(a, b)` / `sum_array(arr)` - Addition/sum operations

Additional language-specific functions showcase unique capabilities of each platform:
- Python: `factorial()` for demonstrating recursion
- C: `is_prime()`, `square_root()` for math operations
- AssemblyScript: `reverseString()`, `power()` for string and numeric operations