# AssemblyScript WebAssembly Example

This example demonstrates how to create a WebAssembly module using AssemblyScript.

## Features

- String manipulation (greet, reverse string)
- Mathematical calculations (fibonacci, factorial, power, square root)
- Array processing (sum, find max, create array)
- Prime number checking
- Memory management with typed arrays

## Build and Run

From the wasmrun project root:

```sh
# Run with wasmrun
wasmrun run examples/asc-hello

# Or compile manually
wasmrun compile examples/asc-hello
```

## Usage in JavaScript

Once loaded, you can call AssemblyScript functions from JavaScript:

```javascript
// Import the WASM module
const wasmModule = await WebAssembly.instantiateStreaming(fetch('build/release.wasm'));
const { greet, fibonacci, sumArray, isPrime, factorial, reverseString, findMax, power, squareRoot, createArray } = wasmModule.instance.exports;

// Call functions
console.log(greet("World"));
console.log(fibonacci(10)); // Returns 55
console.log(sumArray([1, 2, 3, 4, 5])); // Returns 15
console.log(isPrime(17)); // Returns 1 (true)
console.log(factorial(5)); // Returns 120n (bigint)
console.log(reverseString("Hello")); // Returns "olleH"
console.log(findMax([3, 7, 2, 9, 1])); // Returns 9
console.log(power(2, 8)); // Returns 256
console.log(squareRoot(25)); // Returns 5
```

## Requirements

- Node.js and npm for building
- AssemblyScript compiler (installed via npm)
- Wasmrun automatically handles the build process