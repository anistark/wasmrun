# C WebAssembly Example

This example demonstrates how to create a WebAssembly module using C with Emscripten.

## Features

- String manipulation and greeting functions
- Mathematical calculations (fibonacci, factorial, prime check)
- Array processing with dynamic memory allocation
- Math library functions (square root)
- Memory management examples

## Build and Run

From the wasmrun project root:

```sh
# Run with wasmrun
wasmrun run examples/c-hello

# Or compile manually
wasmrun compile examples/c-hello
```

## Usage in JavaScript

Once loaded, you can call C functions from JavaScript using Emscripten's runtime:

```javascript
// Call functions using ccall
Module.ccall('greet', null, ['string'], ['World']);

// Call functions using cwrap for repeated use
const fibonacci = Module.cwrap('fibonacci', 'number', ['number']);
console.log(fibonacci(10)); // Returns 55

const factorial = Module.cwrap('factorial', 'number', ['number']);
console.log(factorial(5)); // Returns 120

// Check if number is prime
const isPrime = Module.cwrap('is_prime', 'number', ['number']);
console.log(isPrime(17)); // Returns 1 (true)

// Calculate square root
const sqrt = Module.cwrap('square_root', 'number', ['number']);
console.log(sqrt(25)); // Returns 5.0
```

## Requirements

- Emscripten SDK for compilation
- Wasmrun automatically handles the build process