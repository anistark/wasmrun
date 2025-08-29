# Go WebAssembly Example

This example demonstrates how to create a WebAssembly module using Go with syscall/js.

## Features

- Console logging from Go to browser console
- Function exports: `greet()`, `fibonacci()`, `sumArray()`, `getCurrentTime()`
- JavaScript interop using syscall/js package

## Build and Run

From the wasmrun project root:

```sh
# Run with wasmrun
wasmrun run examples/go-hello

# Or compile manually
wasmrun compile examples/go-hello
```

## Usage in JavaScript

Once loaded, you can call these functions from the browser console:

```javascript
// Greet function
greet("World");

// Calculate fibonacci number
console.log(fibonacci(10)); // Returns 55

// Sum an array
console.log(sumArray([1, 2, 3, 4, 5])); // Returns 15

// Get current time
console.log(getCurrentTime());
```

## Note

Go WASM modules require the `wasm_exec.js` helper file. Wasmrun automatically handles this for you.