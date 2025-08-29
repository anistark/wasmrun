# AssemblyScript Web Application Example

This example demonstrates a complete web application built with AssemblyScript and WebAssembly, featuring interactive UI components and real-time WebAssembly computation.

## Features

- Interactive web interface with multiple demo sections
- Mathematical operations (Fibonacci, prime checking, power calculations)
- String manipulation and validation
- Geometric calculations (area calculator for different shapes)
- Performance benchmarking
- Modern glassmorphism UI design
- Real-time WebAssembly function calls

## Build and Run

From the wasmrun project root:

```sh
# Run with wasmrun
wasmrun run examples/web-asc

# Or compile manually
wasmrun compile examples/web-asc
```

### Manual Setup (Optional)

If you want to build and serve manually:

```sh
cd examples/web-asc
npm install
npm run build
npm run serve
```

## Functions Available

The WebAssembly module exports these functions:

- `greet(name: string)` - Personalized greeting
- `fibonacci(n: i32)` - Calculate nth Fibonacci number
- `isPrime(n: i32)` - Check if number is prime
- `reverseString(input: string)` - Reverse a string
- `power(base: f64, exponent: i32)` - Calculate power
- `validateEmail(email: string)` - Basic email validation
- `calculateArea(shape: string, width: f64, height: f64)` - Calculate geometric areas

## Usage in JavaScript

```javascript
// Functions are automatically available after module loads
wasm.greet("Developer");
wasm.fibonacci(10);
wasm.isPrime(17);
wasm.reverseString("AssemblyScript");
wasm.calculateArea("rectangle", 10, 5);
```

## Requirements

- AssemblyScript compiler
- Modern web browser with WebAssembly support

## Notes

This example showcases AssemblyScript's ability to create interactive web applications with near-native performance. The UI demonstrates real-time WebAssembly function calls with performance monitoring.