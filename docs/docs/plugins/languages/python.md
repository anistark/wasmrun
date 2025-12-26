---
sidebar_position: 3
---

# Python

Compile Python to WebAssembly using the waspy plugin.

## Overview

The `waspy` plugin enables Python to WebAssembly compilation, allowing you to run Python code in WebAssembly environments.

## Prerequisites

- **Python** 3.8 or higher
- **wasmrun** installed (see [Installation](../installation.md))

## Plugin Installation

```bash
wasmrun plugin install waspy
```

## Quick Start

```bash
# Create project
mkdir my-python-wasm && cd my-python-wasm

# Create main.py
cat > main.py << 'EOF'
def greet(name):
    return f"Hello, {name}!"

def add(a, b):
    return a + b

if __name__ == "__main__":
    print(greet("World"))
    print(add(5, 3))
EOF

# Run with Wasmrun
wasmrun run .
```

## Supported Features

The waspy plugin supports core Python features:

- Basic data types (int, float, str, list, dict)
- Functions and classes
- Control flow (if, for, while)
- Built-in functions
- Type annotations

## Limitations

- Limited standard library support
- No C extensions
- No dynamic imports
- Subset of Python features

## Example Project

See [`examples/python-hello`](https://github.com/anistark/wasmrun/tree/main/examples/python-hello) for a complete example.

## Additional Resources

- [Waspy Documentation](https://github.com/anistark/waspy)
- [Wasmrun Examples](https://github.com/anistark/wasmrun/tree/main/examples)
