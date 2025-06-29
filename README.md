# Wasmrun

[![Crates.io Version](https://img.shields.io/crates/v/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads](https://img.shields.io/crates/d/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/wasmrun)](https://crates.io/crates/wasmrun) [![Open Source](https://img.shields.io/badge/open-source-brightgreen)](https://github.com/anistark/wasmrun) [![Contributors](https://img.shields.io/github/contributors/anistark/wasmrun)](https://github.com/anistark/wasmrun/graphs/contributors) ![maintenance-status](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg)

![Wasmrun Logo](./assets/banner.png)

> Wasmrun is a powerful WebAssembly (WASM) runtime CLI tool with full WASI support and modular plugin architecture.

## ✨ Features

- 🚀 **Instant Development Server** - Point Wasmrun to your .wasm file or project and get a ready playground in your browser
- 🌐 **Browser Integration** - Automatically opens your default browser with interactive console and debugging tools
- 💻 **Interactive Console** - View execution results and logs in a beautiful web interface
- 🔍 **Smart Detection** - Automatically identifies entry points and module types (standard WASM vs wasm-bindgen)
- 🔌 **Plugin Architecture** - Modular language support through a flexible plugin system
- 📦 **Multi-Language Support** - Built-in plugins for Rust, Go, C/C++, AssemblyScript, and Python
- 🔧 **Built-in Compilation** - Integrated build system with plugin-based compilation
- 🔍 **WASM Inspection** - Verify and analyze WASM files with detailed module information and binary analysis
- 👀 **Live Reload** - Watch mode for automatic recompilation and browser refresh during development
- 🌟 **Full WASI Support** - Complete WebAssembly System Interface implementation with virtual filesystem
- 🌐 **Web Application Support** - Experimental support for Rust web frameworks (Yew, Leptos, Dioxus, etc.)
- ⚡ **Zero Configuration** - Works out of the box with sensible defaults and automatic project detection

## 🚀 Installation

### From Cargo (Recommended)

```sh
cargo install wasmrun
```

### From Source

```sh
git clone https://github.com/anistark/wasmrun.git
cd wasmrun
cargo install --path .
```

## 📖 Usage

Wasmrun supports both flag-based arguments using `--path` and direct positional arguments for an intuitive command line experience.

### Quick Start

```sh
# Run on current directory
wasmrun

# Run a WebAssembly file directly  
wasmrun myfile.wasm

# Run a project directory
wasmrun ./my-wasm-project

# With flags
wasmrun --path ./path/to/your/file.wasm
wasmrun --path ./my-wasm-project
```

### 🔧 Commands

#### Development Server

Start the development server with live reload:

```sh
wasmrun run ./my-project --watch
wasmrun run ./my-project --port 3000 --language rust
```

#### Compilation

Compile a project to WebAssembly using the appropriate plugin:

```sh
wasmrun compile ./my-project
wasmrun compile ./my-project --output ./build --optimization release
wasmrun compile ./my-project --optimization size --verbose
```

#### Plugin Management

List available plugins and check dependencies:

```sh
# List all available plugins
wasmrun plugin list

# Get detailed plugin information
wasmrun plugin info rust
wasmrun plugin info go
```

#### Verification & Inspection

Verify a WASM file format and analyze structure:

```sh
wasmrun verify ./file.wasm
wasmrun verify ./file.wasm --detailed

wasmrun inspect ./file.wasm
```

#### Project Management

Initialize a new project:

```sh
wasmrun init my-app --template rust
wasmrun init my-app --template go --directory ./projects/
```

Clean build artifacts:

```sh
wasmrun clean ./my-project
```

#### Server Control

Stop any running Wasmrun server:

```sh
wasmrun stop
```

## 🔌 Plugin Architecture

Wasmrun uses a modular plugin system where each programming language is supported through dedicated plugins. This architecture provides:

- **Extensibility** - Easy to add new language support
- **Maintainability** - Each plugin is self-contained
- **Consistency** - Unified interface across all languages
- **Flexibility** - Plugin-specific optimizations and features

### Built-in Plugins

| Plugin | Status | Compiler/Runtime | Capabilities |
|--------|--------|------------------|--------------|
| ![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) | ✅ Full Support | `cargo` + `rustc` | Standard WASM, wasm-bindgen, web apps, optimization |
| ![C](https://img.shields.io/badge/c-%2300599C.svg?style=for-the-badge&logo=c&logoColor=white) ![C++](https://img.shields.io/badge/c++-%2300599C.svg?style=for-the-badge&logo=c%2B%2B&logoColor=white) | ✅ Full Support | `emscripten` | Complete toolchain, Makefile support |
| ![AssemblyScript](https://img.shields.io/badge/assembly%20script-%23000000.svg?style=for-the-badge&logo=assemblyscript&logoColor=white) | ✅ Full Support | `asc` + npm/yarn | TypeScript-like syntax, optimization |
| ![Python](https://img.shields.io/badge/python-3670A0?style=for-the-badge&logo=python&logoColor=ffdd54) | ✅ Beta Support | `py2wasm` | Runtime integration, bundle creation |

### External Plugins

| Plugin | Status | Source | Capabilities |
|--------|--------|--------|--------------|
| ![Go](https://img.shields.io/badge/go-%2300ADD8.svg?style=for-the-badge&logo=go&logoColor=white) | ✅ Full Support | [wasmgo](https://crates.io/crates/wasmgo) | `tinygo` compiler, optimization |

### Plugin Capabilities

Each plugin provides specific capabilities:

| Feature | Rust | Go | C/C++ | AssemblyScript | Python |
|---------|------|----|----|---------------|--------|
| **Compile to WASM** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Web Applications** | ✅ | ❌ | ✅ | ❌ | ✅ |
| **Live Reload** | ✅ | ✅ | ✅ | ✅ | ❌ |
| **Optimization** | ✅ | ✅ | ✅ | ✅ | ❌ |
| **Custom Targets** | Multiple | wasm | web | wasm | TBD |


### Web Frameworks (Rust Plugin)

The Rust plugin automatically detects and supports web frameworks with specialized web application mode:

- **Yew** - Modern Rust / Wasm framework
- **Leptos** - Full-stack, compile-time optimal Rust framework  
- **Dioxus** - Cross-platform GUI library
- **Sycamore** - Reactive library
- **Trunk** - Build tool for Rust-generated WebAssembly

*Web framework support is highly experimental and actively being improved. Looking for contributors. 👋*

## 🌟 How It Works

### Plugin-Based Compilation

1. **Project Detection** - Wasmrun analyzes the project and selects the appropriate plugin
2. **Dependency Verification** - The plugin checks for required tools and dependencies
3. **Compilation** - Plugin-specific build process with optimizations
4. **Output Generation** - WASM file creation with plugin-specific features

### For WASM Files

1. Wasmrun server with WASI support starts running
2. Opens your default browser with an interactive interface
3. Serves the WASM file with comprehensive WASI support including virtual filesystem
4. Provides real-time console output, debugging tools, and file system interaction

### For Projects

1. **Plugin Selection** - Automatically identifies and loads the appropriate language plugin
2. **Dependency Checking** - Plugin verifies required tools are installed
3. **Compilation** - Plugin builds optimized WASM with proper flags and optimizations
4. **Serving** - Runs development server with live reload
5. **Framework Detection** - Special handling for web applications (Rust plugin)

## 🔍 WASI Support

Wasmrun intends to provide support for complete WebAssembly System Interface (WASI) implementation in the browser. It's a work in progress. Some features might work, but it's highly experimental.

## 🎯 Use Cases

### Development & Testing

```sh
# Quick WASM testing with instant feedback
wasmrun test.wasm

# Project development with live reload (plugin auto-detected)
wasmrun run ./my-rust-project --watch

# Build and optimize for production (plugin-specific optimizations)
wasmrun compile ./my-project --optimization size
```

### Plugin Management

```sh
# List available plugins and their capabilities
wasmrun plugin list

# Get detailed information about a specific plugin
wasmrun plugin info rust
```

### Learning & Education

```sh
# Inspect WASM structure and understand internals
wasmrun inspect ./complex-module.wasm

# Verify WASM compliance and format
wasmrun verify ./student-submission.wasm --detailed

# See which plugin would handle a project
wasmrun run ./unknown-project --dry-run
```

### Web Application Development

```sh
# Rust web app with hot reload (Rust plugin auto-detects frameworks)
wasmrun run ./my-yew-app --watch

# Multi-framework support
wasmrun run ./leptos-project
wasmrun run ./dioxus-app

# Python web app with Pyodide
wasmrun run ./my-python-web-app
```

### Performance Analysis

```sh
# Size-optimized builds with plugin-specific optimizations
wasmrun compile ./my-project --optimization size

# Debug builds with full symbols
wasmrun compile ./my-project --optimization debug --verbose

# Compare different plugin optimizations
wasmrun compile ./rust-project --optimization size
wasmrun compile ./go-project --optimization size
```

## 🔧 Configuration

### Environment Variables

- `WASMRUN_PORT` - Default server port (default: 8420)
- `WASMRUN_WATCH` - Enable watch mode by default
- `WASMRUN_OUTPUT` - Default output directory for builds
- `WASMRUN_DEBUG` - Enable debug output
- `RUST_BACKTRACE` - Show stack traces for errors

### Plugin Detection

Wasmrun automatically selects plugins based on project structure:

- **Rust Plugin**: `Cargo.toml` present
- **Go Plugin**: `go.mod` or `.go` files present
- **C/C++ Plugin**: `.c`, `.cpp`, `.h` files, or `Makefile` present
- **AssemblyScript Plugin**: `package.json` with AssemblyScript dependency or `assembly/` directory
- **Python Plugin**: `.py` files or `requirements.txt` present

### Optimization Levels

Plugin-specific optimization levels:

- **`debug`** - Fast compilation, full symbols, no optimization
- **`release`** - Optimized for performance (default)
- **`size`** - Optimized for minimal file size (plugin-dependent implementation)

## 🔍 Troubleshooting

### Plugin-Related Issues

**"No plugin found for project"**
```sh
# Check what files are in your project
ls -la
# Ensure proper entry files exist (Cargo.toml, go.mod, etc.)
# Use wasmrun plugin list to see available plugins
```
🚨 Open an [issue](https://github.com/anistark/wasmrun/issues) and let us know about it.

**"Plugin dependencies missing"**
```sh
# Install missing tools for specific plugins:
rustup target add wasm32-unknown-unknown  # Rust plugin
# Install emcc for C/C++ plugin
# Install tinygo for Go plugin  
# Install asc for AssemblyScript plugin
```

**"Wrong plugin selected"**
```sh
# Force a specific plugin
wasmrun --language rust
wasmrun --language go
```
### Configuring py2wasm

- Make sure that you have python3.11.0 is installed and configured. We recommend
  using [mise](https://mise.jdx.dev/getting-started.html).
  ```sh
  mise use python@3.11.0
  ```
- Now install py2wasm, you can use a virtual environment or not.
```sh
pip install py2wasm
```
- Make sure that you have named the entry file as main.py or app.py.

### Common Issues

**"Port is already in use"**
```sh
wasmrun stop  # Stop existing server
wasmrun --port 3001  # Use different port
```

**"No entry point found"**
- Ensure your WASM has `main()`, `_start()`, or exported functions
- Use `wasmrun inspect` to see available exports
- Check plugin-specific entry file requirements

**"wasm-bindgen module detected"**
- Use the `.js` file instead of the `.wasm` file directly (Rust plugin)
- Run `wasmrun project-dir` instead of individual files

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for detailed guidelines, including how to add new plugins and extend existing ones.

### Adding New Plugins

The modular architecture makes it easy to add support for new languages. See the [plugin development guide](./CONTRIBUTING.md#adding-new-plugins) for details.

## 📄 License

[MIT License](./LICENSE)

## 🙏 Credits

Wasmrun is built with love using:

- [tiny_http](https://github.com/tiny-http/tiny-http) - Lightweight HTTP server
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [notify](https://github.com/notify-rs/notify) - File system watching for live reload
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) - Web integration
- And the amazing Rust and WebAssembly communities ❤️

**Made with ❤️ for the WebAssembly community**

*⭐ If you find Wasmrun useful, please consider starring the repository!*
