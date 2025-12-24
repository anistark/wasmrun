---
sidebar_position: 1
---

# Introduction

Welcome to **Wasmrun** - a powerful WebAssembly runtime that simplifies development, compilation, and deployment of WebAssembly applications.

## What is Wasmrun?

Wasmrun is a comprehensive development server and runtime for WebAssembly projects that enables you to build, develop, and deploy WASM applications across multiple programming languages through an extensible plugin system.

Whether you're building web applications, CLI tools, or exploring WebAssembly capabilities, Wasmrun provides the tools you need with zero configuration.

## Key Features

### Multi-Language Support
Build WebAssembly from your favorite language:
- **Rust** - Full support with `wasmrust` plugin
- **Go** - TinyGo integration with `wasmgo` plugin
- **Python** - Compile Python to WASM with `waspy` plugin
- **C/C++** - Built-in support via Emscripten
- **AssemblyScript** - TypeScript-like syntax with `wasmasc` plugin

### Plugin Architecture
Wasmrun uses an extensible plugin system that makes it easy to add support for new languages and build tools. Plugins can be:
- **Built-in** - C/C++ support is included out of the box
- **External** - Install additional language support with `wasmrun plugin install`
- **Custom** - Create your own plugins for specialized workflows

### Development Experience
- **Live Reload** - Instant feedback with automatic recompilation on file changes
- **Zero-Config Web Server** - Built-in HTTP server on port 8420 with WASM and web app hosting
- **Smart Project Detection** - Automatically detects and configures project types
- **Zero Configuration** - Works out of the box with sensible defaults

### Advanced Features

**Native WASM Execution**
Run compiled WASM files directly with the native interpreter:
```bash
wasmrun exec myapp.wasm arg1 arg2
wasmrun exec mylib.wasm --call add 5 3
```

**WASI Support**
Full WebAssembly System Interface compatibility for filesystem access, environment variables, and command-line arguments.

**Network Isolation**
Per-process network namespaces for secure execution of untrusted code.

**Port Forwarding**
Easy port mapping for web applications with automatic configuration.

**OS Mode**
Browser-based multi-language execution environment for Node.js and Python applications.

## Architecture Overview

Wasmrun consists of several core modules:

- **Server** - HTTP server for hosting WASM and web applications
- **Plugin System** - Dynamic loading and management of language plugins
- **Compiler** - Orchestrates compilation through installed plugins
- **Runtime** - Native WASM interpreter with WASI support
- **Configuration** - Smart project detection and settings management

## Why Wasmrun?

**Simple and Intuitive**
- No complex configuration files
- Automatic project detection
- Familiar command-line interface

**Extensible**
- Plugin-based architecture
- Support for multiple languages
- Easy to add custom workflows

**Developer-Friendly**
- Live reload for fast iteration
- Helpful error messages
- Comprehensive CLI tools

**Production-Ready**
- Native execution for performance
- WASI compatibility
- Network isolation for security

## Getting Started

Ready to build with WebAssembly? Check out the [Installation](./installation.md) guide to get started, or jump straight to the [Quick Start](./quick-start.md) tutorial.

## Learn More

- **[Language Guides](./guides/rust.md)** - Learn how to use Wasmrun with your preferred language
- **[Features](./features/plugin-system.md)** - Explore Wasmrun's capabilities in depth
- **[CLI Reference](./cli/overview.md)** - Complete command-line documentation
- **[Development](./development/architecture.md)** - Contribute to Wasmrun or create plugins

## Community

- [GitHub Repository](https://github.com/anistark/wasmrun)
- [Issue Tracker](https://github.com/anistark/wasmrun/issues)
- [Discussions](https://github.com/anistark/wasmrun/discussions)
- [Crates.io](https://crates.io/crates/wasmrun)
