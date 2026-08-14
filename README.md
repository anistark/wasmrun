# Wasmrun

![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?style=for-the-badge&logo=WebAssembly&logoColor=white)

[![Crates.io Version](https://img.shields.io/crates/v/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads](https://img.shields.io/crates/d/wasmrun)](https://crates.io/crates/wasmrun) [![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/wasmrun)](https://crates.io/crates/wasmrun) [![Open Source](https://img.shields.io/badge/open-source-brightgreen)](https://github.com/anistark/wasmrun) [![Contributors](https://img.shields.io/github/contributors/anistark/wasmrun)](https://github.com/anistark/wasmrun/graphs/contributors) ![maintenance-status](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Wasmrun** is a powerful WebAssembly runtime that simplifies development, compilation, and deployment of WebAssembly applications.

![Banner](./assets/banner.png)

## ✨ Features

- 🚀 **Multi-Language Support** - Rust, Go, Python, C/C++, and AssemblyScript
- 🔌 **Plugin Architecture** - Extensible system with built-in and external plugins
- 🔥 **Live Reload** - Instant development feedback with file watching
- 🌐 **Zero-Config Web Server** - Built-in HTTP server for WASM and web apps
- 📦 **Smart Project Detection** - Automatically detects and configures project types
- 🏃 **Native WASM Execution** - Run WASM files directly with argument passing
- 🤖 **Agent Sandboxes** - A REST API that gives AI agents isolated environments to run code in, without Docker

## 📚 Documentation

**📖 [Full Documentation](https://wasmrun.readthedocs.io)**

## 🚀 Quick Start

### Installation

```sh
cargo install wasmrun
```

On Arch Linux, install from the [AUR](https://aur.archlinux.org/packages/wasmrun-bin) (community-maintained):

```sh
yay -S wasmrun-bin
```

For other installation methods (DEB, RPM, from source), see the [Installation Guide](https://wasmrun.readthedocs.io/en/latest/docs/installation).

### Basic Usage

```sh
# Run a WASM file with dev server
wasmrun myfile.wasm

# Run a project directory
wasmrun ./my-wasm-project

# Compile a project
wasmrun compile ./my-project

# Execute WASM natively
wasmrun exec myfile.wasm

# Install language plugins
wasmrun plugin install wasmrust
wasmrun plugin install wasmgo
```

See the [Quick Start Guide](https://wasmrun.readthedocs.io/en/latest/docs/quick-start) for a complete tutorial.

## 🎛️ Four Modes

| Mode | Command | What it does |
|------|---------|--------------|
| **[Server](https://wasmrun.readthedocs.io/en/latest/docs/server)** | `wasmrun ./my-project` | Compile and serve a project with a dev server, live reload, and browser-based module inspection |
| **[Exec](https://wasmrun.readthedocs.io/en/latest/docs/exec)** | `wasmrun exec ./program.wasm` | Run a WASM file natively through the built-in interpreter with WASI. No browser, no server |
| **[Agent](https://wasmrun.readthedocs.io/en/latest/docs/agent)** | `wasmrun agent` | A REST sandbox API for AI agents. No Docker, no daemon |
| **[OS](https://wasmrun.readthedocs.io/en/latest/docs/os)** | `wasmrun os ./my-project` | Browser-based VM with a virtual filesystem and multi-language runtimes |

### 🏃 Exec Mode

Run a `.wasm` file directly. Arguments after the file go to the program, and `--call` picks an exported function instead of the entry point:

```sh
wasmrun exec ./program.wasm arg1 arg2
wasmrun exec ./math.wasm --call add 2 3
```

The interpreter implements WASI Preview 1, so file I/O, environment variables, arguments, clocks, and randomness all work.

### 🤖 Agent Mode

Start a sandbox server for AI agents:

```sh
wasmrun agent --port 8430
```

Create a session, then execute code in it:

```sh
SID=$(curl -sX POST http://localhost:8430/api/v1/sessions | jq -r .session_id)

curl -X POST http://localhost:8430/api/v1/sessions/$SID/exec \
  -H 'Content-Type: application/json' \
  -d '{"source": "const _ = require(\"lodash\"); console.log(_.chunk([1,2,3,4], 2));",
       "dependencies": {"lodash": "^4.17.21"}}'
```

- **JavaScript and TypeScript** run from source, with npm dependencies vendored into the sandbox (it has no network of its own) and lockfiles for reproducible installs
- **Pre-compiled `.wasm`** modules run through the same interpreter as exec mode
- `GET /api/v1/tools` returns **OpenAI/Anthropic-compatible tool schemas**, so an LLM can drive the sandbox through function calling
- Per-session limits on memory, fuel, output, file size, and disk; optional API-key auth with tenant isolation

## 🔌 Plugin System

Wasmrun uses a plugin architecture for language support:

**Built-in:**
- C/C++ (Emscripten)

**External Plugins:**
- Rust: `wasmrun plugin install wasmrust`
- Go: `wasmrun plugin install wasmgo`
- Python: `wasmrun plugin install waspy`
- AssemblyScript: `wasmrun plugin install wasmasc`

Learn more in the [Plugin Documentation](https://wasmrun.readthedocs.io/en/latest/docs/plugins/).

## 🤝 Contributing

We welcome contributions! See our [Contributing Guide](https://wasmrun.readthedocs.io/en/latest/docs/development/contributing).

## 🎤 Community

- [Community Page](https://wasmrun.readthedocs.io/en/latest/community/) - Talks, demos, and contributors
- [GitHub Issues](https://github.com/anistark/wasmrun/issues)
- [GitHub Discussions](https://github.com/anistark/wasmrun/discussions)

## 📄 License

[MIT License](./LICENSE)

## 🙏 Credits

Wasmrun is built with love using:

- [tiny_http](https://github.com/tiny-http/tiny-http) - Lightweight HTTP server
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [notify](https://github.com/notify-rs/notify) - File system watching for live reload
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) - Web integration
- Font used for logo is *Pixeled* by [OmegaPC777](https://www.youtube.com/channel/UCc5ROnYDjc4hynqsLFw4Fzg)
- And the amazing Rust and WebAssembly communities ❤️

**Made with ❤️ for the WebAssembly community**

*⭐ If you find Wasmrun useful, please consider starring the repository!*
