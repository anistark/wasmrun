# Python WebAssembly Example

This example demonstrates how to create a WebAssembly module using Python with Pyodide.

## Features

- String manipulation and greeting functions
- Mathematical calculations (fibonacci, pi approximation)
- Array processing
- JSON parsing and manipulation
- Advanced math operations using Python's math library

## Build and Run

From the wasmrun project root:

```sh
# Run with wasmrun
wasmrun run examples/python-hello

# Or compile manually
wasmrun compile examples/python-hello
```

## Usage in JavaScript

Once loaded in the browser, you can call Python functions from JavaScript:

```javascript
// Access Python functions through pyodide
await pyodide.loadPackage("numpy"); // if needed

// Call Python functions
pyodide.runPython(`
    greet("World")
    print(fibonacci(10))
    print(sum_array([1, 2, 3, 4, 5]))
    calculate_pi(1000)
    math_operations(10, 3)
`);
```

## Note

Python WebAssembly requires Pyodide runtime. Wasmrun automatically handles the setup for you.