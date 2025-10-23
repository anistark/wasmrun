# Wasmrun

![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=for-the-badge&logo=WebAssembly&logoColor=white) 

[![Crates.io Version](https://img.shields.io/crates/v/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads](https://img.shields.io/crates/d/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/wasmrun)](https://crates.io/crates/wasmrun) [![Open Source](https://img.shields.io/badge/open-source-brightgreen)](https://github.com/anistark/wasmrun) [![Contributors](https://img.shields.io/github/contributors/anistark/wasmrun)](https://github.com/anistark/wasmrun/graphs/contributors) ![maintenance-status](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) 

**Wasmrun** is a powerful WebAssembly runtime that simplifies development, compilation, and deployment of WebAssembly applications.

![Banner](./assets/banner.png)

## ✨ Features

- 🚀 **Multi-Language Support** - Build WebAssembly from Rust, Go, Python, C/C++, and AssemblyScript
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

### From Prebuilt Packages

<details>
<summary>DEB Package (Debian/Ubuntu/Pop! OS)</summary>

![Ubuntu](https://img.shields.io/badge/Ubuntu-E95420?style=for-the-badge&logo=ubuntu&logoColor=white) ![Debian](https://img.shields.io/badge/Debian-D70A53?style=for-the-badge&logo=debian&logoColor=white) ![Pop!\_OS](https://img.shields.io/badge/Pop!_OS-48B9C7?style=for-the-badge&logo=Pop!_OS&logoColor=white) ![Linux Mint](https://img.shields.io/badge/Linux%20Mint-87CF3E?style=for-the-badge&logo=Linux%20Mint&logoColor=white)

Wasmrun is available as a DEB package for Debian-based systems.

1. Download the latest `.deb` file from [GitHub Releases](https://github.com/anistark/wasmrun/releases)
2. Install the package:

```bash
# Install the downloaded DEB package
sudo apt install wasmrun_*.deb

# If there are dependency issues, fix them
sudo apt install -f
```
</details>

<details>
<summary>RPM Package (Fedora/RHEL/CentOS)</summary>

![Fedora](https://img.shields.io/badge/Fedora-294172?style=for-the-badge&logo=fedora&logoColor=white) ![Red Hat](https://img.shields.io/badge/Red%20Hat-EE0000?style=for-the-badge&logo=redhat&logoColor=white) ![CentOS](https://img.shields.io/badge/CentOS-262577?style=for-the-badge&logo=centos&logoColor=white)

Wasmrun is available as an RPM package for Red Hat-based systems.

1. Download the latest `.rpm` file from [GitHub Releases](https://github.com/anistark/wasmrun/releases)
2. Install the package:

```bash
# Install the downloaded RPM package
sudo rpm -i wasmrun-*.rpm

# Or using dnf (Fedora/RHEL 8+)
sudo dnf install wasmrun-*.rpm
```
</details>

Track releases on [github releases](https://github.com/anistark/wasmrun/releases) or [via release feed](https://github.com/anistark/wasmrun/releases.atom).

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

Wasmrun's modular plugin architecture enables seamless integration of different programming languages and compilation toolchains into a unified development experience. Here's a detailed guide on [wasmrun plugin architecture](https://blog.anirudha.dev/wasmrun-plugin-architecture).

### Plugin Types

#### 1. **Built-in Plugins** 🔧
Built-in plugins are compiled directly into Wasmrun and provide core language support:

| Plugin | Language | Compiler | Status | Capabilities |
|--------|----------|----------|---------|--------------|
| **C/C++** | C, C++ | Emscripten | ✅ Stable | Full WASM + Web Apps + Makefiles |
| **AssemblyScript** | TypeScript-like | `asc` | ✅ Stable | WASM + Optimization + npm/yarn |

#### 2. **External Plugins** 📦
External plugins are distributed via crates.io and installed dynamically to `~/.wasmrun/`:

| Plugin | Language | Compiler | Installation | Capabilities |
|--------|----------|----------|-------------|--------------|
| **wasmrust** | Rust | `rustc` + `wasm-pack` | `wasmrun plugin install wasmrust` | Full WASM + Web Apps + Optimization |
| **wasmgo** | Go | TinyGo | `wasmrun plugin install wasmgo` | WASM + Optimization + Package Support |
| **waspy** | Python | waspy | `wasmrun plugin install waspy` | WASM + Python-to-WASM Compilation |

**How External Plugins Work:**
- 📦 **Cargo-like Installation**: Similar to `cargo install`, plugins are downloaded and compiled to `~/.wasmrun/`
- 🔗 **Dynamic Loading**: Plugins are loaded as shared libraries (FFI) at runtime
- 🎯 **Same Interface**: External plugins use identical traits as built-in plugins
- 🔧 **Auto-detection**: Once installed, plugins automatically handle their supported project types

### Plugin Management

```sh
# Install external plugins (similar to cargo install)
wasmrun plugin install wasmrust  # Installs to ~/.wasmrun/
wasmrun plugin install wasmgo
wasmrun plugin install waspy

# View all installed plugins
wasmrun plugin list

# Get detailed plugin information
wasmrun plugin info wasmrust
wasmrun plugin info waspy

# Search for available plugins
wasmrun plugin search rust

# Uninstall plugins
wasmrun plugin uninstall wasmgo
```

**Plugin Installation Process:**
1. 🔍 **Discovery**: Searches crates.io for the plugin
2. 📦 **Download**: Uses `cargo install` to build the plugin
3. 🏠 **Storage**: Installs to `~/.wasmrun/plugins/{plugin_name}/`
4. 📋 **Registration**: Updates wasmrun config with plugin capabilities
5. ⚡ **Ready**: Plugin automatically handles supported projects

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

### Python (via External Plugin)

```sh
# Install the Python plugin
wasmrun plugin install waspy

# Run Python projects
wasmrun ./my-python-wasm-project
```

**Requirements:**
- None! waspy is a pure Rust compiler that compiles Python to WebAssembly

**Features:**
- ✅ Python to WebAssembly compilation
- ✅ Support for functions, classes, and basic Python syntax
- ✅ Type annotations support
- ✅ No Python runtime required

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

## 🔍 Project Detection

Wasmrun automatically detects your project type based on:

- **File extensions** (`.rs`, `.go`, `.py`, `.c`, `.cpp`, `.ts`)
- **Configuration files** (`Cargo.toml`, `go.mod`, `Makefile`, `package.json`)
- **Entry point files** (`main.rs`, `main.go`, `main.py`, `main.c`, etc.)

You can override detection with the `--language` flag:

```sh
wasmrun --language rust ./my-project
wasmrun --language go ./my-project
wasmrun --language python ./my-project
```

## 🚨 Troubleshooting

### Plugin Issues

**"Plugin not available"**
```sh
# For built-in language support:
wasmrun --language c        # C/C++ (built-in)
wasmrun --language asc      # AssemblyScript (built-in)

# For Rust projects, install the external plugin:
wasmrun plugin install wasmrust
# Use wasmrun plugin list to see available plugins
```

🚨 Open an [issue](https://github.com/anistark/wasmrun/issues) and let us know about it.

**"Plugin dependencies missing"**
```sh
# Install missing tools for external plugins:
rustup target add wasm32-unknown-unknown  # For wasmrust plugin
go install tinygo.org/x/tinygo@latest     # For wasmgo plugin

# Check plugin dependencies:
wasmrun plugin info wasmrust  # Shows required dependencies
wasmrun plugin info waspy     # Should show no dependencies
```

**"Wrong plugin selected"**
```sh
# Force a specific plugin
wasmrun --language rust
wasmrun --language go
wasmrun --language python
```

### External Plugin Installation

**"Plugin not found during installation"**

```sh
# Make sure you have the correct plugin name
wasmrun plugin install wasmrust   # For Rust support
wasmrun plugin install wasmgo     # For Go support
wasmrun plugin install waspy      # For Python support

# Check available external plugins
wasmrun plugin list --external
```


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
