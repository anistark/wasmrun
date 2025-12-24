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
- 🏃 **Native WASM Execution** - Run compiled WASM files directly with the native interpreter and pass arguments

## 📚 Documentation

**Official documentation is available at [wasmrun.readthedocs.io](https://wasmrun.readthedocs.io)**

For comprehensive guides, API references, tutorials, and examples, visit our documentation site.

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

# Run a WebAssembly file with dev server (default)
wasmrun myfile.wasm

# Run a project directory
wasmrun ./my-wasm-project

# With flags
wasmrun --path ./path/to/your/file.wasm
wasmrun --path ./my-wasm-project

# Run a WASM file natively with the interpreter
wasmrun exec myfile.wasm

# Run a WASM file with arguments
wasmrun exec myfile.wasm arg1 arg2 arg3
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

#### Native WASM Execution

Run compiled WASM files directly using the native interpreter with full argument passing and function selection support (useful for CLI tools, test binaries, etc):

```sh
# Execute WASM file natively (runs entry point: main, _start, or start)
wasmrun exec myapp.wasm

# Execute with arguments
wasmrun exec myapp.wasm arg1 arg2 arg3

# Call a specific exported function
wasmrun exec mylib.wasm -c add 5 3
wasmrun exec mylib.wasm --call multiply 4 6

# Call a function with arguments
wasmrun exec myapp.wasm -c process --verbose output.txt

# Stdout goes directly to terminal
wasmrun exec cli-tool.wasm --help
```

**Default Behavior:** Running WASM files with `wasmrun file.wasm` (no subcommand) starts the dev server on port 8420. Use the `exec` subcommand to bypass the server and execute the WASM module directly.

**Argument Passing:** The `exec` subcommand fully supports passing arguments to your WASM program. Arguments are captured and made available to the program through WASI syscalls.

**Function Selection:** Use the `-c` or `--call` flag to invoke a specific exported function. If not specified, the runtime automatically detects and calls the entry point function (in order: start section, `main`, or `_start`).

**Compatibility Note:** Native execution currently works best with pure WASM modules (e.g., compiled from Go with TinyGo). Modules compiled with **wasm-bindgen** (JavaScript interop framework used by Rust's `wasm-pack`) are not currently supported in native mode, as they require JavaScript runtime features. For wasm-bindgen projects, use the dev server or run the project directory instead of the individual `.wasm` file.

### Native Execution Capabilities & Limitations

The native WASM executor is a complete interpreter supporting most WASM features:

**✅ Fully Supported:**
- All arithmetic operations (i32/i64/f32/f64)
- Control flow (if/else, loops, blocks)
- Branching (br, br_if)
- Function calls (direct and recursive)
- Memory operations (load, store, grow, size)
- Local and global variables
- All comparison and unary operations

**⚠️ Current Limitations:**

**Language Runtime Requirements:**
- **Rust**: Functions may fail if they require panic hooks, stack unwinding, or memory allocators. Simple pure functions work well.
- **Go/TinyGo**: Some functions require scheduler initialization or asyncify state. Basic arithmetic and pure functions work reliably.

**Recommended Use Cases:**
- ✅ Pure computational functions (math, algorithms)
- ✅ CLI tools compiled with minimal runtime (TinyGo, pure Rust)
- ✅ Hand-written WAT files
- ✅ C/C++ with Emscripten `--no-entry` flag
- ⚠️ Complex Rust/Go applications (use dev server instead)

**Workarounds:**
- For Rust: Compile with `wasm32-wasi` target and avoid wasm-bindgen
- For Go: Use TinyGo with `-target wasi`
- For complex applications: Use `wasmrun` dev server (runs in browser environment)
- Write pure WASM: Use WAT format for maximum compatibility

**Examples:**
```sh
# ✅ Works perfectly - pure WASM
wasmrun exec pure_math.wasm -c fibonacci 10

# ✅ Works well - simple Rust/Go functions
wasmrun exec simple.wasm -c add 5 3

# ⚠️ May fail - complex Rust with panic handling
wasmrun exec complex.wasm -c process_data

# ✅ Alternative - use dev server for complex code
wasmrun ./my-rust-project
```

## 🏗️ Plugin Architecture

Wasmrun's modular plugin architecture enables seamless integration of different programming languages and compilation toolchains into a unified development experience. Here's a detailed guide on [wasmrun plugin architecture](https://blog.anirudha.dev/wasmrun-plugin-architecture).

### Plugin Types

#### 1. **Built-in Plugins** 🔧
Built-in plugins are compiled directly into Wasmrun and provide core language support:

| Plugin | Language | Compiler | Status | Capabilities |
|--------|----------|----------|---------|--------------|
| **C/C++** | C, C++ | Emscripten | ✅ Stable | Full WASM + Web Apps + Makefiles |

#### 2. **External Plugins** 📦
External plugins are distributed via crates.io and installed dynamically to `~/.wasmrun/`:

| Plugin | Language | Compiler | Installation | Capabilities |
|--------|----------|----------|-------------|--------------|
| **wasmasc** | AssemblyScript | `asc` | `wasmrun plugin install wasmasc` | WASM + Optimization + npm/yarn/pnpm/bun |
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
# Install external plugins
wasmrun plugin install wasmrust # Rust plugin
wasmrun plugin install wasmgo   # Go plugin
wasmrun plugin install waspy    # Python plugin
wasmrun plugin install wasmasc  # AssemblyScript plugin

# View all installed plugins
wasmrun plugin list

# Get detailed plugin information
wasmrun plugin info <plugin-name>

# Uninstall plugins
wasmrun plugin uninstall <plugin-name>
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

### AssemblyScript (via External Plugin)

```sh
# Install the AssemblyScript plugin
wasmrun plugin install wasmasc

# Run AssemblyScript projects
wasmrun ./my-assemblyscript-project
```

**Requirements:**
- AssemblyScript compiler: `npm install -g asc`
- Node.js runtime

**Package Manager Support:**
The plugin automatically detects and uses your preferred package manager:
- `npm` (default)
- `yarn` (via `yarn.lock`)
- `pnpm` (via `pnpm-lock.yaml`)
- `bun` (via `bun.lockb`)

### C/C++ (Built-in)

```sh
# Works out of the box - no plugin installation needed
wasmrun ./my-c-project
```

**Requirements:**
- Emscripten SDK: [https://emscripten.org/](https://emscripten.org/)

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
wasmrun --language asc ./my-project
```

## 🚨 Troubleshooting

### Plugin Issues

**"Plugin not available"**
```sh
# For built-in language support:
wasmrun --language c        # C/C++ (built-in)

# For external plugins, install them first:
wasmrun plugin install wasmrust   # Rust plugin
wasmrun plugin install wasmgo     # Go plugin
wasmrun plugin install waspy      # Python plugin
wasmrun plugin install wasmasc    # AssemblyScript plugin

# View all available plugins:
wasmrun plugin list
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

**"wasm-bindgen module detected" / "Native execution fails on Rust WASM files"**
- wasm-bindgen modules require JavaScript runtime features and are not supported in native mode
- **Recommended approach:**
  - Run the entire project directory: `wasmrun ./my-rust-project` (dev server)
  - Use the `.js` file if available (wasmrust plugin output)
  - Avoid using `wasmrun exec` with wasm-bindgen compiled modules
- **Workaround if you need native execution:**
  - Compile with pure WASM (no wasm-bindgen) or use Go/TinyGo for CLI tools
  - Consider refactoring your Rust code to avoid wasm-bindgen dependencies

## 📼 Talks/Demos

A list of wasmrun related talks (latest first): ✨

| When | What | Where | Who |
| --- | --- | --- | --- |
| Oct 18, 2025 | Bringing Python to WebAssembly | PyCon Thailand 2025, Bangkok | [Farhaan](https://github.com/farhaanbukhsh), [Ani](https://github.com/anistark) |
| Sept 20, 2025 | [Your Next Server Might Be a Browser](https://www.youtube.com/watch?v=NXGxSM9Mqes) | IndiaFOSS 2025, Bengaluru | [Ani](https://github.com/anistark) |
| Sept 13, 2025 | Compiling Python to WASM | PyCon India 2025, Bengaluru | [Farhaan](https://github.com/farhaanbukhsh), [Ani](https://github.com/anistark) |
| July 16, 2025 | [WASM and Python: The Future of Serverless Computing](https://www.youtube.com/watch?v=qes-hzyVIGU) | EuroPython 2025, Prague, Czech | [Farhaan](https://github.com/farhaanbukhsh), [Ani](https://github.com/anistark) |
| May 24, 2025 | [WASM and Python](https://x.com/__bangpypers__/status/1926174903264252149) | BangPypers Meetup | [Farhaan](https://github.com/farhaanbukhsh), [Ani](https://github.com/anistark) |

_If you've talked about wasmrun at a conference, podcast, virtual or local meetup, feel free to add to this list_ 🙌 

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
