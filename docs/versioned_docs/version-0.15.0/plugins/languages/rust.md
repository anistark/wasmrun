---
sidebar_position: 1
---

# Rust

Learn how to build WebAssembly applications with Rust and Wasmrun.

## Overview

Rust provides excellent WebAssembly support with mature tooling and great performance. Wasmrun makes it easy to develop and run Rust-based WASM applications through the `wasmrust` plugin.

## Prerequisites

Before you start, ensure you have:

- **Rust** 1.70 or higher ([install from rustup.rs](https://rustup.rs/))
- **wasmrun** installed (see [Installation](../../installation.md))
- **wasm32 target** installed:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

## Plugin Installation

Install the Rust plugin for Wasmrun:

```bash
wasmrun plugin install wasmrust
```

Verify installation:

```bash
wasmrun plugin info wasmrust
```

## Project Setup

### Basic Project Structure

Create a new Rust library project:

```bash
cargo new --lib my-wasm-project
cd my-wasm-project
```

### Cargo.toml Configuration

Configure your `Cargo.toml` for WebAssembly:

```toml
[package]
name = "my-wasm-project"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
```

**Key configuration points:**

- `crate-type = ["cdylib"]` - Creates a dynamic library suitable for WASM
- `wasm-bindgen` - Enables JavaScript interop

### For Web Applications

If you're building a web application that needs DOM access:

```toml
[dependencies]
wasm-bindgen = "0.2"

[dependencies.web-sys]
version = "0.3"
features = [
  "console",
  "Document",
  "Element",
  "Window",
]
```

## Writing Rust for WebAssembly

### Basic Example

Here's a simple example (`src/lib.rs`):

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to WebAssembly.", name)
}

#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[wasm_bindgen]
pub fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}
```

### Console Logging

Add console.log support for debugging:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn process_data(value: i32) -> i32 {
    console_log!("Processing value: {}", value);
    let result = value * 2;
    console_log!("Result: {}", result);
    result
}
```

### Working with Arrays

Process JavaScript arrays in Rust:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn sum_array(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}

#[wasm_bindgen]
pub fn sort_array(mut numbers: Vec<i32>) -> Vec<i32> {
    numbers.sort();
    numbers
}
```

## Development Workflow

### Run Development Server

Start the development server with live reload:

```bash
wasmrun run . --watch
```

This will:
1. Detect your Rust project
2. Compile to WebAssembly
3. Start server at `http://localhost:8420`
4. Auto-reload on file changes

### Specify Port

```bash
wasmrun run . --port 3000 --watch
```

### Compilation Only

To just compile without starting a server:

```bash
wasmrun compile .
```

With optimization:

```bash
wasmrun compile . --optimization release
```

## wasm-bindgen vs Pure WASM

### wasm-bindgen (Browser-Based)

**Use when:**
- Building web applications
- Need DOM access
- Want JavaScript interop
- Deploying to browsers

**Example:**
```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn update_dom() {
    // Interact with browser
}
```

**Run with:** `wasmrun run .`

### Pure WASM (Native Execution)

**Use when:**
- Building CLI tools
- Server-side processing
- No browser required
- Maximum portability

**Configuration for pure WASM:**
```toml
[package]
name = "my-cli-tool"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "my-cli"
path = "src/main.rs"

[dependencies]
# No wasm-bindgen needed
```

**Run with:** `wasmrun exec ./target/wasm32-wasi/release/my-cli.wasm`

## Complete Examples

### Example 1: Simple Functions

See [`examples/rust-hello`](https://github.com/anistark/wasmrun/tree/main/examples/rust-hello) for a complete working example with multiple exported functions.

```bash
# Clone the repository
git clone https://github.com/anistark/wasmrun.git
cd wasmrun/examples/rust-hello

# Run the example
wasmrun run .
```

### Example 2: Web Application

See [`examples/web-leptos`](https://github.com/anistark/wasmrun/tree/main/examples/web-leptos) for a full web application using the Leptos framework.

```bash
cd wasmrun/examples/web-leptos
wasmrun run . --watch
```

## Native Execution

For native execution without a browser:

### Build for WASI

```toml
# Cargo.toml
[package]
name = "native-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "app"
path = "src/main.rs"
```

```rust
// src/main.rs
fn main() {
    println!("Hello from native WASM!");
}
```

### Compile and Execute

```bash
# Compile
cargo build --target wasm32-wasi --release

# Execute natively
wasmrun exec ./target/wasm32-wasi/release/app.wasm
```

### Call Specific Functions

```bash
# Call a specific exported function
wasmrun exec ./mylib.wasm --call add 5 3
```

See [`examples/native-rust`](https://github.com/anistark/wasmrun/tree/main/examples/native-rust) for more native execution examples.

## Best Practices

### 1. Error Handling

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn safe_divide(a: f64, b: f64) -> Result<f64, JsValue> {
    if b == 0.0 {
        Err(JsValue::from_str("Division by zero"))
    } else {
        Ok(a / b)
    }
}
```

### 2. Memory Management

Keep allocations minimal and use references when possible:

```rust
#[wasm_bindgen]
pub fn process_string(input: &str) -> String {
    // Efficient - borrows input
    input.to_uppercase()
}
```

### 3. Optimization

For production builds:

```bash
# Optimize for size
cargo build --target wasm32-unknown-unknown --release

# Further optimization with wasm-opt (if installed)
wasm-opt -Oz -o optimized.wasm target/wasm32-unknown-unknown/release/my_app.wasm
```

### 4. Type Safety

Leverage Rust's type system:

```rust
#[wasm_bindgen]
pub struct User {
    name: String,
    age: u32,
}

#[wasm_bindgen]
impl User {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, age: u32) -> User {
        User { name, age }
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }
}
```

## Troubleshooting

### wasm-bindgen Module Detected

If you see this warning with `wasmrun exec`:

```
Warning: wasm-bindgen module detected
```

**Solution:** wasm-bindgen modules need a browser environment. Use the dev server instead:
```bash
wasmrun run .
```

### Build Failures

**Missing target:**
```bash
rustup target add wasm32-unknown-unknown
```

**Plugin not found:**
```bash
wasmrun plugin install wasmrust
```

### Performance Issues

1. Use `--release` builds for production
2. Profile with `cargo flamegraph`
3. Minimize allocations
4. Use `&str` instead of `String` when possible

## Additional Resources

- [Official wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
- [Wasmrun Examples](https://github.com/anistark/wasmrun/tree/main/examples)
- [Rust API Docs](https://docs.rs)

## Next Steps

- Explore [WASI Support](../../integrations/wasi.md) for system interface capabilities
- Learn about [Native Execution](../../cli/exec.md) in depth
- Check out [Live Reload](../../server/live-reload.md) for development workflow
- Review the [CLI Reference](../../cli/) for all available commands
