# Chakra

[![Crates.io Version](https://img.shields.io/crates/v/chakra)
](https://crates.io/crates/chakra) [![Crates.io Downloads](https://img.shields.io/crates/d/chakra)](https://crates.io/crates/chakra) [![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/chakra)](https://crates.io/crates/chakra) [![Open Source](https://img.shields.io/badge/open-source-brightgreen)](https://github.com/anistark/chakra) [![Contributors](https://img.shields.io/github/contributors/anistark/chakra)](https://github.com/anistark/chakra/graphs/contributors) ![maintenance-status](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg)

![Chakra Logo](./assets/banner.png)

> Chakra is a WebAssembly (WASM) runtime CLI tool.

## Features

- 🚀 Start Chakra Server by pointing to your .wasm file/project and get a ready playground on your browser.
- 🌐 Instantly opens your default browser with your wasm project and other necessary dependencies pre-loaded.
- 💻 View execution results and logs on an interactive console.
- 🔍 Identifies common entry points and runs your wasm project.
- 📦 Compile your project to wasm using CLI tool directly. (Needs external dependencies.) [WIP]
- 🧩 Verify and inspect your wasm file weather generated via chakra or any other tool. [WIP]

> 👋 It's highly experimental, but fast iterating. Welcoming contributors and support to help bring out this project even better!

## Installation

### From Cargo (Recommended)

```sh
cargo install chakra
```

### From Source

```sh
git clone https://github.com/anistark/chakra.git
cd chakra
cargo install --path .
```

## Usage

Chakra supports both flag-based arguments using `--path` and direct positional arguments for a more intuitive command line experience.

### Basic Usage

Run on current directory:

```sh
chakra
```

Run a WebAssembly file directly:

```sh
chakra myfile.wasm

# Using --path flag
chakra --path ./path/to/your/file.wasm

# With --wasm flag explicitly
chakra --wasm myfile.wasm
```

Run a project directory:

```sh
chakra ./my-wasm-project

# Using --path flag
chakra --path ./my-wasm-project
```

### Custom Port

Specify a custom port (default is `8420`):

```sh
chakra --path ./path/to/your/file.wasm --port 3000
# Or
chakra ./path/to/your/file.wasm --port 3000
```

### Watch Mode

Enable watch mode for live reloading:

```sh
chakra --path ./path/to/your/project --watch
# Or
chakra ./my-wasm-project --watch
```

### Verify WASM File

Verify if a WebAssembly file is in the correct format:

```sh
chakra verify ./path/to/your/file.wasm

# Using --path flag
chakra verify --path ./path/to/your/file.wasm
```

For detailed output:

```sh
chakra verify --path ./path/to/your/file.wasm --detailed
# Or
chakra verify ./path/to/your/file.wasm --detailed
```

### Inspect WASM File

Get detailed information about a WASM file:

```sh
chakra inspect ./path/to/your/file.wasm
# Or
chakra inspect --path ./path/to/your/file.wasm
```

### Compile a Project to WASM

Compile a project directory to WebAssembly:

```sh
chakra compile ./my-project
# Or
chakra compile --path ./my-project
```

With a custom output directory:

```sh
chakra compile ./my-project --output ./build
```

### Run with Specific Language

Specify a language for compilation:

```sh
chakra run ./my-project --language rust
```

### Stop Server

Stop any running Chakra server:

```sh
chakra stop
```

## How It Works

When you run Chakra with a WASM file:

1. It starts a lightweight HTTP server
2. Opens your default browser
3. Serves the WASM file along with a nice UI
4. Attempts to instantiate and run the WebAssembly module
5. Shows execution results and console logs

## Supported WASM Types

Chakra works best with:

- Simple C/C++ compiled WASM files
- Rust WASM files compiled without wasm-bindgen
- Any WASM that doesn't require extensive JavaScript bindings

For complex WASM modules (like those compiled with wasm-bindgen), Chakra will detect this and provide helpful information, but may not be able to execute them fully.

## Examples

### Running a simple C-compiled WASM file:

```sh
# Compile C to WASM (requires emscripten)
emcc -O2 hello.c -o hello.wasm

# Run with Chakra
chakra hello.wasm
```

### Running a simple Rust WASM file:

```sh
# Build a WASM file from Rust
cargo build --target wasm32-unknown-unknown --release

# Run with Chakra
chakra ./target/wasm32-unknown-unknown/release/yourapp.wasm
```

## Troubleshooting

- **"Port is already in use"**: Try specifying a different port with `--port`
- **"No WASM entry point found"**: Your WASM file might not have standard entry points like `main()` or `_start()`
- **"This appears to be a wasm-bindgen module"**: Try using the original JavaScript loader that came with your WASM file

## License

[MIT](./LICENSE)

## Credits

Chakra is built with:
- [tiny_http](https://github.com/tiny-http/tiny-http) - A lightweight HTTP server
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- ❤️ and WebAssembly enthusiasm

![Chakra Logo](./assets/loader.svg)
