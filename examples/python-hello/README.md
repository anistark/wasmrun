# Python Hello Example

A simple Python WebAssembly example demonstrating basic functions.

## Features

This example includes:
- String manipulation (`greet`)
- Arithmetic operations (`add`)
- Recursive functions (`fibonacci`, `factorial`)
- Type annotations for all functions

## Functions

- `greet(name: str) -> str` - Returns a greeting message
- `add(a: int, b: int) -> int` - Adds two numbers
- `fibonacci(n: int) -> int` - Calculates the nth Fibonacci number
- `factorial(n: int) -> int` - Calculates the factorial of n

## Running

```bash
wasmrun run examples/python-hello
```

## Requirements

This requires the `waspy` external plugin:

```bash
wasmrun plugin install waspy
```
