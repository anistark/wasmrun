---
sidebar_position: 4
---

# C/C++

Build WebAssembly applications with C/C++ using Emscripten.

## Overview

C/C++ has excellent WebAssembly support through Emscripten. Wasmrun includes built-in support for C/C++ projects without requiring a separate plugin.

## Prerequisites

- **Emscripten SDK** ([install from emscripten.org](https://emscripten.org/docs/getting_started/downloads.html))
- **wasmrun** installed (see [Installation](../../installation.md))

## Quick Start

```bash
# Create project
mkdir my-c-wasm && cd my-c-wasm

# Create hello.c
cat > hello.c << 'EOF'
#include <stdio.h>
#include <emscripten.h>

EMSCRIPTEN_KEEPALIVE
int add(int a, int b) {
    return a + b;
}

EMSCRIPTEN_KEEPALIVE
void greet(const char* name) {
    printf("Hello, %s!\n", name);
}

int main() {
    printf("C WebAssembly module loaded!\n");
    return 0;
}
EOF

# Create Makefile
cat > Makefile << 'EOF'
CC = emcc
CFLAGS = -O2

all:
	$(CC) $(CFLAGS) hello.c -o hello.html

clean:
	rm -f hello.wasm hello.js hello.html
EOF

# Run with Wasmrun
wasmrun run .
```

## Compiling

### Basic Compilation

```bash
emcc hello.c -o hello.html
```

### Optimization

```bash
# Optimize for size
emcc -Os hello.c -o hello.wasm

# Optimize for speed
emcc -O3 hello.c -o hello.wasm
```

### WASI Target

```bash
emcc --target=wasm32-wasi hello.c -o hello.wasm
```

## Example Project

See [`examples/c-hello`](https://github.com/anistark/wasmrun/tree/main/examples/c-hello) for a complete example.

## Best Practices

1. Use `EMSCRIPTEN_KEEPALIVE` to export functions
2. Enable optimizations for production
3. Minimize memory usage
4. Use `printf` for console output

## Additional Resources

- [Emscripten Documentation](https://emscripten.org/docs/)
- [WebAssembly C/C++ Guide](https://webassembly.org/getting-started/developers-guide/)
- [Wasmrun Examples](https://github.com/anistark/wasmrun/tree/main/examples)
