# Chakra

[![Crates.io Version](https://img.shields.io/crates/v/chakra)](https://crates.io/crates/chakra) [![Crates.io Downloads](https://img.shields.io/crates/d/chakra)](https://crates.io/crates/chakra) [![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/chakra)](https://crates.io/crates/chakra) [![Open Source](https://img.shields.io/badge/open-source-brightgreen)](https://github.com/anistark/chakra) [![Contributors](https://img.shields.io/github/contributors/anistark/chakra)](https://github.com/anistark/chakra/graphs/contributors) ![maintenance-status](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg)

![Chakra Logo](./assets/banner.png)

> Chakra is a powerful WebAssembly (WASM) runtime CLI tool with full WASI support.

## ✨ Features

- 🚀 **Instant Development Server** - Point Chakra to your .wasm file or project and get a ready playground in your browser
- 🌐 **Browser Integration** - Automatically opens your default browser with interactive console and debugging tools
- 💻 **Interactive Console** - View execution results and logs in a beautiful web interface
- 🔍 **Smart Detection** - Automatically identifies entry points and module types (standard WASM vs wasm-bindgen)
- 📦 **Multi-Language Support** - Compile Rust, Go, C/C++, AssemblyScript, and Python* projects to WASM
- 🔧 **Built-in Compilation** - Integrated build system with optimization options
- 🔍 **WASM Inspection** - Verify and analyze WASM files with detailed module information
- 👀 **Live Reload** - Watch mode for automatic recompilation and browser refresh
- 🌟 **Full WASI Support** - Complete WebAssembly System Interface implementation
- 🌐 **Web Application Support** - First-class support for Rust web frameworks (Yew, Leptos, Dioxus, etc.)
- ⚡ **Zero Configuration** - Works out of the box with sensible defaults

*Python support coming soon

## 🚀 Installation

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

## 📖 Usage

Chakra supports both flag-based arguments using `--path` and direct positional arguments for an intuitive command line experience.

### Quick Start

Run on current directory:

```sh
chakra
```

Run a WebAssembly file directly:

```sh
chakra myfile.wasm
# Or using --path flag
chakra --path ./path/to/your/file.wasm
```

Run a project directory:

```sh
chakra ./my-wasm-project
# Or using --path flag
chakra --path ./my-wasm-project
```

### 🔧 Commands

#### Development Server

Start the development server with live reload:

```sh
chakra run ./my-project --watch
chakra run ./my-project --port 3000 --language rust
```

#### Compilation

Compile a project to WebAssembly:

```sh
chakra compile ./my-project
chakra compile ./my-project --output ./build --optimization release
```

#### Verification & Inspection

Verify a WASM file format:

```sh
chakra verify ./file.wasm
chakra verify ./file.wasm --detailed
```

Get detailed information about a WASM file:

```sh
chakra inspect ./file.wasm
```

#### Project Management

Initialize a new project:

```sh
chakra init my-app --template rust
chakra init my-app --template go --directory ./projects/
```

Clean build artifacts:

```sh
chakra clean ./my-project
```

#### Server Control

Stop any running Chakra server:

```sh
chakra stop
```

## 🛠️ Supported Languages & Frameworks

### Programming Languages

| Language | Status | Compiler | Notes |
|----------|--------|----------|-------|
| ![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) | ✅ Full Support | `cargo` + `rustc` | Standard WASM, wasm-bindgen, and web apps |
| ![Go](https://img.shields.io/badge/go-%2300ADD8.svg?style=for-the-badge&logo=go&logoColor=white) | ✅ Full Support | `tinygo` | Lightweight Go runtime |
| ![C](https://img.shields.io/badge/c-%2300599C.svg?style=for-the-badge&logo=c&logoColor=white) ![C++](https://img.shields.io/badge/c++-%2300599C.svg?style=for-the-badge&logo=c%2B%2B&logoColor=white) | ✅ Full Support | `emscripten` | Complete toolchain support |
| ![AssemblyScript](https://img.shields.io/badge/assembly%20script-%23000000.svg?style=for-the-badge&logo=assemblyscript&logoColor=white) | ✅ Full Support | `asc` | TypeScript-like syntax |
| ![Python](https://img.shields.io/badge/python-3670A0?style=for-the-badge&logo=python&logoColor=ffdd54) | 🚧 Coming Soon | `py2wasm` / `waspy` | In development |

### Web Frameworks (Rust)

Chakra automatically detects and supports Rust web frameworks. Highly experimental!
_Currently in active development. Feel free to contribute in if you're interested in working on it._

## 🌟 How It Works

### For WASM Files

1. Chakra starts a lightweight HTTP server
2. Opens your default browser with an interactive interface
3. Serves the WASM file with comprehensive WASI support
4. Provides real-time console output and debugging tools

### For Projects

1. **Language Detection** - Automatically identifies project type
2. **Dependency Checking** - Verifies required tools are installed
3. **Compilation** - Builds optimized WASM with proper flags
4. **Serving** - Runs development server with live reload
5. **Web App Mode** - Special handling for web applications

## 🔍 WASI Support

Chakra includes a complete WebAssembly System Interface (WASI) implementation:

### Supported Features ✅

- **Virtual Filesystem** - Complete file system with directories and files
- **Standard I/O** - stdout, stderr with console integration
- **Environment Variables** - Full environment variable support
- **Command Arguments** - Access to command-line arguments
- **File Operations** - Read, write, seek, and file management
- **Random Number Generation** - Secure random numbers via Web Crypto API
- **Time Functions** - System time and high-precision timers
- **Pre-opened Directories** - Filesystem sandboxing

### Coming Soon 🚧

- **Network Sockets** - TCP/UDP socket support
- **Threading** - Multi-threading and shared memory

## 🎯 Use Cases

### Development & Testing

```sh
# Quick WASM testing
chakra test.wasm

# Project development with live reload
chakra run ./my-rust-project --watch

# Build and optimize for production
chakra compile ./my-project --optimization size
```

### Learning & Education

```sh
# Inspect WASM structure
chakra inspect ./complex-module.wasm

# Verify WASM compliance
chakra verify ./student-submission.wasm --detailed
```

### Web Application Development

```sh
# Rust web app with hot reload
chakra run ./my-yew-app --watch

# Multi-framework support
chakra run ./leptos-project
chakra run ./dioxus-app
```

## 🔧 Configuration

### Environment Variables

- `CHAKRA_PORT` - Default server port (default: 8420)
- `CHAKRA_WATCH` - Enable watch mode by default
- `CHAKRA_OUTPUT` - Default output directory for builds

### Project Detection

Chakra automatically detects project types:

- **Rust**: `Cargo.toml` present
- **Go**: `go.mod` or `.go` files
- **C/C++**: `.c` or `.cpp` files
- **AssemblyScript**: `package.json` with AssemblyScript dependency
- **Python**: 🚧 Coming Soon

## 🚀 Examples

### Rust Examples

```sh
# Standard Rust WASM
cargo new --bin my-wasm-app
cd my-wasm-app
# Add your Rust code
chakra run .

# Rust web application
cargo new --bin my-web-app
cd my-web-app
# Add Yew/Leptos dependencies
chakra run . --watch
```

### C Examples

```sh
# Simple C program
echo 'int main() { return 42; }' > hello.c
emcc hello.c -o hello.wasm
chakra hello.wasm
```

### Go Examples

```sh
# TinyGo project
echo 'package main; func main() { println("Hello") }' > main.go
chakra run .
```

## 🔍 Troubleshooting

### Common Issues

**"Port is already in use"**
```sh
chakra stop  # Stop existing server
chakra --port 3001  # Use different port
```

**"No entry point found"**
- Ensure your WASM has `main()`, `_start()`, or exported functions
- Use `chakra inspect` to see available exports

**"Missing compilation tools"**
- Install required compilers (rustc, emcc, tinygo)
- Run `chakra compile --help` for tool requirements

**"wasm-bindgen module detected"**
- Use the `.js` file instead of the `.wasm` file directly
- Run `chakra project-dir` instead of individual files

## 🤝 Contributing

We welcome contributions! See [CONTRIBUTING.md](./CONTRIBUTING.md) for detailed guidelines.

## 📄 License

[MIT License](./LICENSE)

## 🙏 Credits

Chakra is built with love using:

- [tiny_http](https://github.com/tiny-http/tiny-http) - Lightweight HTTP server
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [notify](https://github.com/notify-rs/notify) - File system watching
- And the amazing Rust and WebAssembly communities ❤️

---

![Chakra Logo](./assets/loader.svg)

**Made with ❤️ for the WebAssembly community**
