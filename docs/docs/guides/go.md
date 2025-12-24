---
sidebar_position: 2
---

# Go

Build WebAssembly applications with Go using TinyGo and Wasmrun.

## Overview

Go supports WebAssembly through TinyGo, a Go compiler designed for small places. Wasmrun's `wasmgo` plugin makes it easy to compile and run Go projects as WebAssembly.

## Prerequisites

- **TinyGo** 0.28 or higher ([install from tinygo.org](https://tinygo.org/getting-started/install/))
- **Go** 1.19 or higher (TinyGo dependency)
- **wasmrun** installed (see [Installation](../installation.md))

## Plugin Installation

```bash
wasmrun plugin install wasmgo
```

## Quick Start

```bash
# Create project
mkdir my-go-wasm && cd my-go-wasm
go mod init my-go-wasm

# Create main.go
cat > main.go << 'EOF'
package main

import "fmt"

func main() {
    fmt.Println("Hello from Go WebAssembly!")
}
EOF

# Run with Wasmrun
wasmrun run . --watch
```

## Native Execution

```bash
# Build for WASI
tinygo build -target=wasi -o app.wasm .

# Execute
wasmrun exec app.wasm
```

Example: [`examples/go-hello`](https://github.com/anistark/wasmrun/tree/main/examples/go-hello)

## Additional Resources

- [TinyGo Documentation](https://tinygo.org/docs/)
- [Wasmrun Examples](https://github.com/anistark/wasmrun/tree/main/examples)
