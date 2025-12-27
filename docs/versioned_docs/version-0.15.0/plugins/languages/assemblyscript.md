---
sidebar_position: 5
---

# AssemblyScript

Build WebAssembly with TypeScript-like syntax using AssemblyScript.

## Overview

AssemblyScript is a TypeScript-like language that compiles to WebAssembly. The `wasmasc` plugin provides seamless integration with Wasmrun.

## Prerequisites

- **Node.js** 16 or higher
- **npm, yarn, pnpm, or bun** (package manager)
- **wasmrun** installed (see [Installation](../../installation.md))

## Plugin Installation

```bash
wasmrun plugin install wasmasc
```

## Quick Start

```bash
# Create project
mkdir my-asc-wasm && cd my-asc-wasm
npm init -y

# Install AssemblyScript
npm install --save-dev assemblyscript

# Initialize AssemblyScript
npx asinit .

# Edit assembly/index.ts
cat > assembly/index.ts << 'EOF'
export function add(a: i32, b: i32): i32 {
  return a + b;
}

export function greet(name: string): string {
  return `Hello, ${name}!`;
}
EOF

# Run with Wasmrun
wasmrun run . --watch
```

## Project Structure

```
my-asc-wasm/
├── assembly/
│   ├── index.ts      # Source code
│   └── tsconfig.json # TypeScript config
├── build/            # Compiled output
├── package.json
└── asconfig.json     # AssemblyScript config
```

## Type System

AssemblyScript uses WebAssembly types:

```typescript
// Integer types
let a: i32 = 42;        // 32-bit integer
let b: i64 = 100n;      // 64-bit integer
let c: u32 = 255;       // unsigned 32-bit
let d: u8 = 8;          // unsigned 8-bit

// Float types
let e: f32 = 3.14;      // 32-bit float
let f: f64 = 2.71828;   // 64-bit float

// Boolean and strings
let g: bool = true;
let h: string = "Hello";
```

## Exported Functions

```typescript
// Export for JavaScript
export function fibonacci(n: i32): i32 {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

export function processArray(arr: Int32Array): i32 {
  let sum: i32 = 0;
  for (let i = 0; i < arr.length; i++) {
    sum += arr[i];
  }
  return sum;
}
```

## Examples

### Simple Functions

See [`examples/asc-hello`](https://github.com/anistark/wasmrun/tree/main/examples/asc-hello)

### Web Application

See [`examples/web-asc`](https://github.com/anistark/wasmrun/tree/main/examples/web-asc)

## Optimization

```json
{
  "options": {
    "optimize": true,
    "optimizeLevel": 3,
    "shrinkLevel": 2,
    "converge": true
  }
}
```

## Best Practices

1. Use specific integer types (i32, i64) instead of number
2. Avoid dynamic arrays when possible
3. Use typed arrays for performance
4. Enable optimizations for production builds

## Additional Resources

- [AssemblyScript Documentation](https://www.assemblyscript.org/)
- [AssemblyScript Book](https://www.assemblyscript.org/introduction.html)
- [Wasmrun Examples](https://github.com/anistark/wasmrun/tree/main/examples)
