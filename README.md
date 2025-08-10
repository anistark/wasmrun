# Wasmrun

![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=for-the-badge&logo=WebAssembly&logoColor=white) 

[![Crates.io Version](https://img.shields.io/crates/v/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads](https://img.shields.io/crates/d/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/wasmrun)](https://crates.io/crates/wasmrun) [![Open Source](https://img.shields.io/badge/open-source-brightgreen)](https://github.com/anistark/wasmrun) [![Contributors](https://img.shields.io/github/contributors/anistark/wasmrun)](https://github.com/anistark/wasmrun/graphs/contributors) ![maintenance-status](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) 

**Wasmrun** is a powerful WebAssembly runtime that simplifies development, compilation, and deployment of WebAssembly applications.

![Banner](./assets/banner.png)

## ✨ Features

- 🚀 **Multi-Language Support** - Build WebAssembly from Rust, Go, C/C++, AssemblyScript, and Python
- 🔌 **Plugin Architecture** - Extensible system with built-in and external plugins
- 🔥 **Live Reload** - Instant development feedback with file watching
- 🌐 **Zero-Config Web Server** - Built-in HTTP server with WASM and web app hosting
- 📦 **Smart Project Detection** - Automatically detects and configures project types
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

List available plugins and manage external plugins:

```sh
# List all available plugins
wasmrun plugin list

# Install external plugins
wasmrun plugin install wasmrust
wasmrun plugin install wasmgo

# Get detailed plugin information
wasmrun plugin info wasmrust
wasmrun plugin info wasmgo
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

## 🏗️ Plugin Architecture

Wasmrun's modular plugin architecture enables seamless integration of different programming languages and compilation toolchains into a unified development experience.

### Plugin Types

#### 1. **Built-in Plugins** 🔧
Built-in plugins are compiled directly into Wasmrun and provide core language support:

| Plugin | Language | Compiler | Status | Capabilities |
|--------|----------|----------|---------|--------------|
| **C/C++** | C, C++ | Emscripten | ✅ Stable | Full WASM + Web Apps + Makefiles |
| **AssemblyScript** | TypeScript-like | `asc` | ✅ Stable | WASM + Optimization + npm/yarn |
| **Python** | Python | `py2wasm` | 🚧 Beta | Runtime Integration + Bundle creation |

#### 2. **External Plugins** 📦
External plugins are distributed via crates.io and can be installed as needed:

| Plugin | Language | Compiler | Installation | Capabilities |
|--------|----------|----------|-------------|--------------|
| **wasmrust** | Rust | `rustc` + `wasm-pack` | `wasmrun plugin install wasmrust` | Full WASM + Web Apps + Optimization |
| **wasmgo** | Go | TinyGo | `wasmrun plugin install wasmgo` | WASM + Optimization + Package Support |

*More external plugins coming soon!*

### Plugin Installation

```sh
# Install external plugins for additional language support
wasmrun plugin install wasmrust
wasmrun plugin install wasmgo

# View all available plugins
wasmrun plugin list

# Get plugin information and status
wasmrun plugin info wasmrust
```

## 🛠️ Language Support

### Rust (via External Plugin)

```sh
# Install the Rust plugin
wasmrun plugin install wasmrust

# Run Rust projects
wasmrun ./my-rust-wasm-project
```

**Requirements:**
- Rust toolchain
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- Optional: `wasm-pack` for web applications

### Go (via External Plugin)

```sh
# Install the Go plugin
wasmrun plugin install wasmgo

# Run Go projects
wasmrun ./my-go-wasm-project
```

**Requirements:**
- TinyGo compiler: [https://tinygo.org/](https://tinygo.org/)

### C/C++ (Built-in)

```sh
# Works out of the box - no plugin installation needed
wasmrun ./my-c-project
```

**Requirements:**
- Emscripten SDK: [https://emscripten.org/](https://emscripten.org/)

### AssemblyScript (Built-in)

```sh
# Works out of the box
wasmrun ./my-assemblyscript-project
```

**Requirements:**
- AssemblyScript compiler: `npm install -g assemblyscript`

### Python (Built-in - Beta)

```sh
# Works out of the box
wasmrun ./my-python-project
```

**Requirements:**
- Python 3.11.0 (recommended to use [mise](https://mise.jdx.dev/))
- py2wasm: `pip install py2wasm`

## 🔍 Project Detection

Wasmrun automatically detects your project type based on:

- **File extensions** (`.rs`, `.go`, `.c`, `.cpp`, `.py`, `.ts`)
- **Configuration files** (`Cargo.toml`, `go.mod`, `Makefile`, `package.json`)
- **Entry point files** (`main.rs`, `main.go`, `main.c`, `main.py`, etc.)

You can override detection with the `--language` flag:

```sh
wasmrun --language rust ./my-project
wasmrun --language go ./my-project
```

## 🚨 Troubleshooting

### Plugin Issues

**"Plugin not available"**
```sh
# For built-in language support:
wasmrun --language c        # C/C++ (built-in)
wasmrun --language asc      # AssemblyScript (built-in)
wasmrun --language python   # Python (built-in)

# For Rust projects, install the external plugin:
wasmrun plugin install wasmrust
# Use wasmrun plugin list to see available plugins
```

🚨 Open an [issue](https://github.com/anistark/wasmrun/issues) and let us know about it.

**"Plugin dependencies missing"**
```sh
# Install missing tools for specific plugins:
rustup target add wasm32-unknown-unknown  # For wasmrust plugin
# Install emcc for C/C++ plugin
# Install tinygo for wasmgo plugin  
# Install asc for AssemblyScript plugin
```

**"Wrong plugin selected"**
```sh
# Force a specific plugin
wasmrun --language rust
wasmrun --language go
```

### External Plugin Installation

**"Plugin not found during installation"**

```sh
# Make sure you have the correct plugin name
wasmrun plugin install wasmrust   # For Rust support
wasmrun plugin install wasmgo     # For Go support

# Check available external plugins
wasmrun plugin list --external
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
wasmrun stop         # Stop existing server
wasmrun --port 3001  # Use different port
```

**"No entry point found"**
- Ensure your WASM has `main()`, `_start()`, or exported functions
- Use `wasmrun inspect` to see available exports
- Check plugin-specific entry file requirements

**"wasm-bindgen module detected"**
- Use the `.js` file instead of the `.wasm` file directly (wasmrust plugin)
- Run `wasmrun project-dir` instead of individual files

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for detailed guidelines, including how to create and maintain plugins.

## 📄 License

[MIT License](./LICENSE)

## 🙏 Credits

Wasmrun is built with love using:

- [tiny_http](https://github.com/tiny-http/tiny-http) - Lightweight HTTP server
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [notify](https://github.com/notify-rs/notify) - File system watching for live reload
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) - Web integration
- Font used for logo is *Pixeled* by [OmegaPC777](https://www.youtube.com/channel/UCc5ROnYDjc4hynqsLFw4Fzg).
- And the amazing Rust and WebAssembly communities ❤️

**Made with ❤️ for the WebAssembly community**

*⭐ If you find Wasmrun useful, please consider starring the repository!*
