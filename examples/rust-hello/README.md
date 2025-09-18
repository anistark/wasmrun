# Rust Hello WebAssembly Example

This is a simple Rust project that compiles to WebAssembly and demonstrates basic functionality with wasmrun.

## Functions Available

- `greet(name: string)` - Returns a greeting message
- `fibonacci(n: u32)` - Calculates the nth Fibonacci number
- `sum_array(numbers: i32[])` - Sums an array of numbers
- `add(a: i32, b: i32)` - Adds two numbers
- `get_memory_size()` - Returns memory size information
- `main()` - Entry point that runs when module loads

## Building

Make sure you have the required tools installed:

```bash
# Install wasm-pack if you haven't already
cargo install wasm-pack

# Build the WebAssembly module
wasm-pack build --target web --out-dir pkg
```

## Running with wasmrun

```bash
# From the project root directory
wasmrun --path examples/rust-hello/pkg/rust_hello.wasm
```

## Testing Functions

Once the module is loaded in wasmrun, you can test the functions in the console:

```javascript
// Test the greet function
greet("World")

// Test the fibonacci function
fibonacci(10)

// Test the sum_array function
sum_array([1, 2, 3, 4, 5])

// Test the add function
add(5, 3)

// Test memory information
get_memory_size()

// Call main function manually
main()
```

## Example Output

```
> greet("Wasmrun")
"Hello, Wasmrun! 👋 This message is from Rust and WebAssembly."

> fibonacci(8)
21

> sum_array([1, 2, 3, 4, 5])
15

> add(10, 20)
30
```