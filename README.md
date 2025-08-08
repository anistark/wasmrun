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
- 📦 **Multi-Language Support** - Built-in plugins for C/C++, AssemblyScript, and Python, plus external plugins for Rust and Go
- 🔧 **Built-in Compilation** - Integrated build system with plugin-based compilation
- 🔍 **WASM Inspection** - Verify and analyze WASM files with detailed module information and binary analysis
- 👀 **Live Reload** - Watch mode for automatic recompilation and browser refresh during development
- 🌟 **Full WASI Support** - Complete WebAssembly System Interface implementation with virtual filesystem
- 🌐 **Web Application Support** - Support for Rust web frameworks (Yew, Leptos, Dioxus, etc.) via external plugins
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

Wasmrun's modular plugin architecture that enables seamless integration of different programming languages and compilation toolchains into a unified development experience.

### Architecture Overview

TBD

### Plugin Types

#### 1. **Built-in Plugins** 🔧
Built-in plugins are compiled directly into Wasmrun and provide core language support:

| Plugin | Language | Compiler | Status | Capabilities |
|--------|----------|----------|---------|--------------|
| **C/C++** | C, C++ | Emscripten | ✅ Stable | Full WASM + Web Apps + Makefiles |
| **AssemblyScript** | TypeScript-like | `asc` | ✅ Stable | WASM + Optimization + npm/yarn |
| **Python** | Python | `py2wasm` | 🚧 Beta | Runtime Integration + Bundle creation |

#### 2. **External Plugins** 📦
External plugins extend Wasmrun's capabilities and are installed as **library extensions**:

| Plugin | Language | Installation | Status | Capabilities |
|--------|----------|-------------|---------|--------------|
| **wasmrust** | Rust | `wasmrun plugin install wasmrust` | ✅ Stable | WASM + Web Apps + wasm-bindgen + Frameworks |
| **wasmgo** | Go | `wasmrun plugin install wasmgo` | ✅ Stable | WASM via TinyGo + Optimization |

### How Plugins Work

#### Plugin Interface Architecture
Every plugin implements the core `Plugin` trait providing a consistent interface:

```rust
pub trait Plugin {
    fn info(&self) -> &PluginInfo;                    // Plugin metadata
    fn can_handle_project(&self, path: &str) -> bool; // Project compatibility
    fn get_builder(&self) -> Box<dyn WasmBuilder>;    // Compilation engine
}

pub trait WasmBuilder {
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;
    fn check_dependencies(&self) -> Vec<String>;      // Missing tools
    fn validate_project(&self, path: &str) -> CompilationResult<()>;
    fn clean(&self, path: &str) -> Result<()>;        // Cleanup artifacts
    fn supported_extensions(&self) -> &[&str];        // File extensions
    fn entry_file_candidates(&self) -> &[&str];       // Entry files
}
```

#### Plugin Capabilities System
Each plugin declares its capabilities through structured metadata:

```rust
pub struct PluginCapabilities {
    pub compile_wasm: bool,          // Standard .wasm file generation
    pub compile_webapp: bool,        // Web application bundling
    pub live_reload: bool,           // Development server support
    pub optimization: bool,          // Size/speed optimization passes
    pub custom_targets: Vec<String>, // Additional compilation targets
}

pub struct PluginInfo {
    pub name: String,                     // Plugin identifier
    pub version: String,                  // Plugin version
    pub description: String,              // Human-readable description
    pub author: String,                   // Plugin author
    pub extensions: Vec<String>,          // Supported file extensions
    pub entry_files: Vec<String>,         // Project entry points
    pub plugin_type: PluginType,          // Built-in vs External
    pub dependencies: Vec<String>,        // Required system tools
    pub capabilities: PluginCapabilities,
}
```

### External Plugin Installation

External plugins are **library extensions** that integrate directly with Wasmrun, not standalone binaries. This design enables deep integration while maintaining modularity.

_Maybe, in future we can also support standalone binaries._

#### Plugin Directory Structure (supposed to be)

```sh
~/.wasmrun/
├── config.toml               # Global Wasmrun configuration
├── plugins/                  # Plugin installation directory
│   ├── wasmrust/             # Rust plugin (external)
│   │   ├── Cargo.toml        # Plugin build configuration
│   │   ├── src/
│   │   │   └── lib.rs        # Rust plugin implementation
│   │   ├── wasmrun.toml      # Plugin manifest
│   │   └── .wasmrun_metadata # Installation metadata
│   └── wasmgo/               # Go plugin (external)
│       ├── Cargo.toml        # Plugin uses Rust for integration
│       ├── src/lib.rs        # Go compilation bridge
│       └── wasmrun.toml      # Plugin configuration
├── cache/                    # Build artifact cache
└── logs/                     # Plugin operation logs
```

### Plugin Development Guide

#### Creating a Built-in Plugin

1. **Create Plugin Structure**
   ```rust
   // src/plugin/languages/my_language_plugin.rs
   use crate::plugin::{Plugin, PluginInfo, PluginCapabilities, PluginType};
   use crate::compiler::builder::WasmBuilder;
   
   pub struct MyLanguagePlugin {
       info: PluginInfo,
   }
   
   impl MyLanguagePlugin {
       pub fn new() -> Self {
           let info = PluginInfo {
               name: "mylang".to_string(),
               version: env!("CARGO_PKG_VERSION").to_string(),
               description: "My Language WebAssembly compiler".to_string(),
               author: "Your Name".to_string(),
               extensions: vec!["ml".to_string(), "myl".to_string()],
               entry_files: vec!["main.ml".to_string(), "package.myl".to_string()],
               plugin_type: PluginType::Builtin,
               source: None,
               dependencies: vec!["mylang-compiler".to_string()],
               capabilities: PluginCapabilities {
                   compile_wasm: true,
                   compile_webapp: false,
                   live_reload: true,
                   optimization: true,
                   custom_targets: vec!["wasm32-wasi".to_string()],
               },
           };
           Self { info }
       }
   }
   ```

2. **Implement Plugin Trait**
   ```rust
   impl Plugin for MyLanguagePlugin {
       fn info(&self) -> &PluginInfo {
           &self.info
       }
   
       fn can_handle_project(&self, project_path: &str) -> bool {
           let path = std::path::Path::new(project_path);
           
           // Check for entry files
           for entry_file in &self.info.entry_files {
               if path.join(entry_file).exists() {
                   return true;
               }
           }
           
           // Check for source files with supported extensions
           if let Ok(entries) = std::fs::read_dir(path) {
               for entry in entries.flatten() {
                   if let Some(ext) = entry.path().extension() {
                       if let Some(ext_str) = ext.to_str() {
                           if self.info.extensions.contains(&ext_str.to_string()) {
                               return true;
                           }
                       }
                   }
               }
           }
           
           false
       }
   
       fn get_builder(&self) -> Box<dyn WasmBuilder> {
           Box::new(MyLanguageBuilder::new())
       }
   }
   ```

3. **Implement WasmBuilder**
   ```rust
   pub struct MyLanguageBuilder;
   
   impl MyLanguageBuilder {
       pub fn new() -> Self {
           Self
       }
   }
   
   impl WasmBuilder for MyLanguageBuilder {
       fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
           // Implement your language's compilation logic
           let project_path = &config.input;
           let output_dir = &config.output_dir;
           
           // Example: Call your language's WASM compiler
           let output = std::process::Command::new("mylang-compiler")
               .args(&["--target", "wasm32-wasi"])
               .args(&["--output", output_dir])
               .arg(project_path)
               .output()
               .map_err(|e| CompilationError::ToolExecutionFailed {
                   tool: "mylang-compiler".to_string(),
                   reason: e.to_string(),
               })?;
   
           if !output.status.success() {
               return Err(CompilationError::BuildFailed {
                   language: "mylang".to_string(),
                   reason: String::from_utf8_lossy(&output.stderr).to_string(),
               });
           }
   
           Ok(BuildResult {
               output_path: format!("{}/output.wasm", output_dir),
               language: "mylang".to_string(),
               optimization_level: config.optimization.clone(),
               build_time: std::time::Duration::from_millis(100), // Measure actual time
               file_size: std::fs::metadata(format!("{}/output.wasm", output_dir))
                   .map(|m| m.len())
                   .unwrap_or(0),
           })
       }
   
       fn check_dependencies(&self) -> Vec<String> {
           let mut missing = Vec::new();
           
           // Check if compiler is available
           if !self.is_tool_available("mylang-compiler") {
               missing.push("mylang-compiler".to_string());
           }
           
           missing
       }
   
       fn validate_project(&self, project_path: &str) -> CompilationResult<()> {
           let path = std::path::Path::new(project_path);
           
           if !path.exists() {
               return Err(CompilationError::BuildFailed {
                   language: "mylang".to_string(),
                   reason: format!("Project path does not exist: {}", project_path),
               });
           }
   
           // Add your validation logic here
           Ok(())
       }
   
       // ... implement other required methods
   }
   ```

4. **Register Plugin in Manager**
   ```rust
   // In src/plugin/manager.rs
   impl PluginManager {
       fn load_builtin_plugins() -> Vec<BuiltinPlugin> {
           vec![
               // Existing plugins
               BuiltinPlugin::new(Arc::new(CPlugin::new())),
               BuiltinPlugin::new(Arc::new(AscPlugin::new())),
               BuiltinPlugin::new(Arc::new(PythonPlugin::new())),
               
               // Your new plugin
               BuiltinPlugin::new(Arc::new(MyLanguagePlugin::new())),
           ]
       }
   }
   ```

#### Creating an External Plugin

External plugins are separate Rust crates that implement the Wasmrun plugin interface:

1. **Create Plugin Crate**

```sh
cargo new --lib wasmxlang
cd wasmxlang
```

2. **Plugin Cargo.toml**
   ```toml
   [package]
   name = "wasmxlang"
   version = "0.1.0"
   edition = "2021"
   description = "XLang to WebAssembly compiler plugin for Wasmrun"
   
   [lib]
   crate-type = ["cdylib", "rlib"]
   
   [dependencies]
   wasmrun-core = { path = "../wasmrun" }  # Or from crates.io when published
   
   [wasm_plugin]
   name = "xlang"
   version = "0.1.0"
   capabilities = ["compile_wasm", "live_reload", "optimization"]
   extensions = ["xl", "xlang"]
   entry_files = ["main.xl", "project.xlang"]
   dependencies = ["xlang-compiler"]
   ```

3. **Implement Plugin**
   ```rust
   // src/lib.rs
   use wasmrun_core::plugin::*;
   
   #[no_mangle]
   pub extern "C" fn wasmrun_plugin_info() -> PluginInfo {
       PluginInfo {
           name: "xlang".to_string(),
           version: "0.1.0".to_string(),
           description: "XLang to WebAssembly compiler".to_string(),
           author: "Your Name".to_string(),
           capabilities: PluginCapabilities {
               compile_wasm: true,
               compile_webapp: false,
               live_reload: true,
               optimization: true,
               custom_targets: vec!["wasm32-unknown-unknown".to_string()],
           },
           extensions: vec!["xl".to_string(), "xlang".to_string()],
           entry_files: vec!["main.xl".to_string(), "project.xlang".to_string()],
           plugin_type: PluginType::External,
           source: Some(PluginSource::CratesIo {
               name: "wasmxlang".to_string(),
               version: "0.1.0".to_string(),
           }),
           dependencies: vec!["xlang-compiler".to_string()],
       }
   }
   ```

### Plugin Configuration

#### Project-Level Configuration
```toml
# wasmrun.toml in your project root
[project]
name = "my-wasm-app"
version = "0.1.0"
language = "rust"  # Force specific plugin

[build]
plugin = "wasmrust"        # Explicit plugin selection
optimization = "release"   # Plugin-specific optimization
output_dir = "./dist"      # Custom output directory

[capabilities]
live_reload = true
web_app = true             # Enable web app features if supported

[dependencies]
# Plugin-specific dependency configuration
```

#### Global Plugin Configuration
```toml
# ~/.wasmrun/config.toml
[settings]
auto_update = false
default_optimization = "size"
verbose = false

[external_plugins.wasmrust]
enabled = true
install_path = "/home/user/.wasmrun/plugins/wasmrust"
auto_update = true

[external_plugins.wasmgo]
enabled = true
install_path = "/home/user/.wasmrun/plugins/wasmgo"
auto_update = false
```

## 🔍 WASI Support

Wasmrun intends to provide support for complete WebAssembly System Interface (WASI) implementation in the browser. It's a work in progress. Some features might work, but it's highly experimental.

## 🎯 Use Cases

### Development & Testing

```sh
# Quick WASM testing with instant feedback
wasmrun test.wasm

# Project development with live reload (plugin auto-detected)
wasmrun ./my-rust-project --watch

# Build and optimize for production (plugin-specific optimizations)
wasmrun compile ./my-project --optimization size
```

### Plugin Management

```sh
# List available plugins and their capabilities
wasmrun plugin list

# Install external plugins
wasmrun plugin install wasmrust

# Get detailed information about a specific plugin
wasmrun plugin info wasmrust
```

### Debugging WASM

```sh
# Inspect WASM structure and understand internals
wasmrun inspect ./complex-module.wasm

# Verify WASM compliance and format
wasmrun verify ./student-submission.wasm --detailed

# See which plugin would handle a project
wasmrun ./unknown-project --dry-run
```

### Web Application Development

```sh
# Rust web app with hot reload (external wasmrust plugin auto-detects frameworks)
wasmrun ./my-yew-app --watch

# Multi-framework support
wasmrun ./leptos-project
wasmrun ./dioxus-app

# Python web app with Pyodide
wasmrun ./my-python-web-app
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

- **Rust Plugin (External)**: `Cargo.toml` present - requires `wasmrun plugin install wasmrust`
- **Go Plugin (External)**: `go.mod` or `.go` files present - requires `wasmrun plugin install wasmgo`
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
- Font used for logo is *Pixeled* by [OmegaPC777](https://www.youtube.com/channel/UCc5ROnYDjc4hynqsLFw4Fzg).
- And the amazing Rust and WebAssembly communities ❤️

**Made with ❤️ for the WebAssembly community**

*⭐ If you find Wasmrun useful, please consider starring the repository!*
