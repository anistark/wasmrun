# Contributing to Wasmrun

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) 

Thank you for considering contributing to Wasmrun! This guide will help you understand the project structure, development workflow, and how to make meaningful contributions.

## 🏗️ Project Architecture

Wasmrun is designed with a modular architecture that separates concerns clearly:

```sh
src/
├── cli.rs                    # Command line interface and argument parsing
├── main.rs                   # Application entry point and command routing
├── error.rs                  # Centralized error handling with user-friendly messages
├── ui.rs                     # User interface utilities and styled output
├── debug.rs                  # Debug utilities and logging
├── watcher.rs                # File system watching for live reload functionality
├── commands/                 # Command implementations
│   ├── mod.rs                # Command module exports
│   ├── verify.rs             # WASM verification and inspection
│   ├── compile.rs            # Project compilation with optimization options
│   ├── run.rs                # Development server and project execution
│   ├── clean.rs              # Build artifact cleanup
│   ├── init.rs               # Project initialization
│   ├── stop.rs               # Server management
│   └── plugin.rs             # Plugin management commands
├── compiler/                 # Legacy compilation system (being phased out)
│   ├── mod.rs                # Compiler module exports
│   ├── builder.rs            # Build configuration and result types
│   └── detect.rs             # Project type detection utilities
├── plugin/                   # 🔌 Plugin system (Core Architecture)
│   ├── mod.rs                # Plugin manager and core traits
│   ├── bridge.rs             # Plugin bridge functionality
│   ├── builtin.rs            # Built-in plugin registry
│   ├── config.rs             # Plugin configuration management
│   ├── external.rs           # External plugin loading and management
│   ├── installer.rs          # Plugin installation system
│   ├── manager.rs            # Plugin lifecycle management
│   ├── registry.rs           # Plugin registry and discovery
│   └── languages/            # Built-in language plugins
│       ├── mod.rs            # Language plugin exports
│       ├── asc_plugin.rs     # AssemblyScript plugin
│       ├── c_plugin.rs       # C/C++ plugin with Emscripten
│       └── python_plugin.rs  # Python plugin with py2wasm
├── server/                   # HTTP server and web interface
│   ├── mod.rs                # Server module exports
│   ├── config.rs             # Server configuration and setup
│   ├── handler.rs            # HTTP request handling
│   ├── wasm.rs               # WASM file serving
│   └── utils.rs              # Server utilities
├── template/                 # HTML, CSS, and JavaScript templates
│   ├── mod.rs                # Template module exports
│   └── server/               # WASM runner interface templates
│       ├── mod.rs            # Server template exports
│       ├── index.html        # Main HTML template
│       ├── scripts.js        # JavaScript utilities
│       ├── style.css         # CSS styles
│       └── wasmrun_wasi_impl.js  # WASI implementation
└── utils/                    # Shared utilities and helpers
    ├── mod.rs                # Utility module exports
    ├── command.rs            # Shared command execution utilities
    ├── path.rs               # Path resolution and validation
    ├── plugin_utils.rs       # Plugin-specific utilities
    ├── system.rs             # System information and detection
    └── wasm_analysis.rs      # WebAssembly file analysis
```

## 🔌 Plugin Architecture Deep Dive

Wasmrun implements a plugin architecture with built-in and external plugins.

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

### External Plugin System

External plugins are **dynamically loaded libraries** that integrate with Wasmrun via FFI (Foreign Function Interface). This approach provides:

- 🔗 **Direct Integration**: Plugins use the same traits as built-in plugins
- 🚀 **Performance**: No subprocess overhead, direct function calls
- 📦 **Distribution**: Available on crates.io, installed like `cargo install`
- 🏠 **Isolation**: Installed to `~/.wasmrun/` directory
- ⚙️ **Dynamic Loading**: Loaded at runtime via shared libraries (`.dylib`, `.so`)

#### Plugin Installation Architecture

```sh
~/.wasmrun/
├── config.toml               # Global configuration & plugin registry
├── bin/                      # Optional binaries (if plugins provide CLI tools)
│   ├── wasmrust              # Rust plugin binary (optional)
│   └── wasmgo                # Go plugin binary (optional)
├── plugins/                  # Plugin source & metadata
│   ├── wasmrust/             # Rust plugin installation
│   │   ├── Cargo.toml        # Plugin build configuration
│   │   ├── src/lib.rs        # Plugin implementation
│   │   ├── target/release/   # Compiled artifacts
│   │   │   └── libwasmrust.dylib # Shared library for FFI
│   │   └── .wasmrun_metadata # Plugin capabilities & dependencies
│   └── wasmgo/               # Go plugin installation
│       ├── Cargo.toml        # Rust wrapper for Go compiler
│       ├── src/lib.rs        # FFI bridge to Go toolchain
│       ├── target/release/
│       │   └── libwasmgo.dylib   # Shared library
│       └── .wasmrun_metadata
├── cache/                    # Build artifact cache
└── logs/                     # Plugin operation logs
```

#### Plugin Loading Process

1. **Installation**: `wasmrun plugin install wasmrust`
   - Downloads from crates.io using `cargo install`
   - Compiles plugin to `~/.wasmrun/plugins/wasmrust/target/release/`
   - Extracts capabilities from plugin's `Cargo.toml` metadata
   - Updates wasmrun config with plugin information

2. **Runtime Loading**: When processing a Rust project
   - Detects project type (`.rs` files, `Cargo.toml`)
   - Loads `libwasmrust.dylib` dynamically via `libloading`
   - Calls plugin functions directly via FFI interface
   - No subprocess overhead, direct integration

## 🔧 Plugin Development Guide

### Creating a Built-in Plugin

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

       fn can_handle_project(&self, path: &str) -> bool {
           // Check for language-specific files or configurations
           std::path::Path::new(path).join("mylang.config").exists() ||
           std::fs::read_dir(path).ok().map_or(false, |mut entries| {
               entries.any(|e| e.ok().map_or(false, |entry| {
                   entry.path().extension()
                       .and_then(|ext| ext.to_str())
                       .map_or(false, |ext| ext == "ml" || ext == "myl")
               }))
           })
       }

       fn get_builder(&self) -> Box<dyn WasmBuilder> {
           Box::new(MyLanguageBuilder::new())
       }
   }
   ```

3. **Implement WasmBuilder Trait**
   ```rust
   pub struct MyLanguageBuilder {
       language_name: String,
   }

   impl MyLanguageBuilder {
       pub fn new() -> Self {
           Self {
               language_name: "mylang".to_string(),
           }
       }

       fn language_name(&self) -> &str {
           &self.language_name
       }
   }

   impl WasmBuilder for MyLanguageBuilder {
       fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
           // Implementation details for building with your language
           let executor = CommandExecutor::new(&config.project_path);
           
           // Check dependencies
           let missing_deps = self.check_dependencies();
           if !missing_deps.is_empty() {
               return Err(CompilationError::DependencyError {
                   tool: missing_deps.join(", "),
                   language: self.language_name().to_string(),
               });
           }

           // Build command
           let build_result = executor.execute_command(
               "mylang-compiler",
               &["build", "--target", "wasm32", &config.project_path],
               "Failed to compile with MyLang compiler",
           )?;

           Ok(BuildResult {
               wasm_file: build_result.output_file,
               js_file: None,
               additional_files: vec![],
               has_bindgen: false,
           })
       }

       fn check_dependencies(&self) -> Vec<String> {
           let mut missing = Vec::new();
           
           if !CommandExecutor::is_tool_installed("mylang-compiler") {
               missing.push("mylang-compiler".to_string());
           }
           
           missing
       }

       fn validate_project(&self, path: &str) -> CompilationResult<()> {
           // Validate project structure and required files
           let project_path = std::path::Path::new(path);
           
           if !project_path.join("main.ml").exists() && !project_path.join("package.myl").exists() {
               return Err(CompilationError::ProjectValidationFailed {
                   language: self.language_name().to_string(),
                   reason: "No entry file found (main.ml or package.myl)".to_string(),
               });
           }
           
           Ok(())
       }

       fn clean(&self, path: &str) -> Result<()> {
           // Clean build artifacts
           let build_dir = std::path::Path::new(path).join("build");
           if build_dir.exists() {
               std::fs::remove_dir_all(build_dir)?;
           }
           Ok(())
       }

       fn supported_extensions(&self) -> &[&str] {
           &["ml", "myl"]
       }

       fn entry_file_candidates(&self) -> &[&str] {
           &["main.ml", "package.myl", "app.ml"]
       }
   }
   ```

4. **Register the Plugin**
   ```rust
   // In src/plugin/languages/mod.rs
   pub mod my_language_plugin;
   pub use my_language_plugin::MyLanguagePlugin;

   // In src/plugin/builtin.rs or manager.rs
   pub fn get_builtin_plugins() -> Vec<Box<dyn Plugin>> {
       vec![
           Box::new(CPlugin::new()),
           Box::new(AscPlugin::new()),
           Box::new(PythonPlugin::new()),
           Box::new(MyLanguagePlugin::new()), // Add your plugin here
       ]
   }
   ```

### Creating an External Plugin

External plugins are distributed as separate crates that integrate with Wasmrun:

1. **Create a New Crate**
   ```toml
   # Cargo.toml
   [package]
   name = "wasmrun-mylang"
   version = "0.1.0"
   edition = "2021"

   [lib]
   crate-type = ["cdylib"]

   [dependencies]
   wasmrun = { version = "0.10", features = ["plugin-api"] }
   ```

2. **Implement Plugin in lib.rs**
   ```rust
   // src/lib.rs
   use wasmrun::plugin::{Plugin, PluginInfo, WasmBuilder, PluginCapabilities, PluginType};

   pub struct MyExternalPlugin {
       info: PluginInfo,
   }

   impl MyExternalPlugin {
       pub fn new() -> Self {
           // Similar implementation to built-in plugin
       }
   }

   // Implement Plugin and WasmBuilder traits...

   // Export plugin creation function
   #[no_mangle]
   pub extern "C" fn create_plugin() -> *mut dyn Plugin {
       Box::into_raw(Box::new(MyExternalPlugin::new()))
   }
   ```

3. **Create Plugin Manifest**
   ```toml
   # wasmrun.toml
   [plugin]
   name = "mylang"
   version = "0.1.0"
   description = "MyLang WebAssembly compiler plugin"
   author = "Your Name"
   
   [dependencies]
   system = ["mylang-compiler >= 1.0.0"]
   
   [capabilities]
   compile_wasm = true
   compile_webapp = false
   live_reload = true
   optimization = true
   ```

### Plugin Development Best Practices

#### Code Organization

Use the shared utilities and follow established patterns:

```rust
// ✅ Good - use shared CommandExecutor
use crate::utils::command::CommandExecutor;

let executor = CommandExecutor::new(&config.project_path);
let copied = CommandExecutor::copy_to_output(&source, &output_dir, "Language")?;
if CommandExecutor::is_tool_installed("tool") { /* ... */ }

// ❌ Bad - duplicate implementation
fn my_execute_command() { /* ... */ }
```

#### Error Handling

Use consistent error types:

```rust
// ✅ Good - use CompilationError types
return Err(CompilationError::BuildToolNotFound {
    tool: "compiler".to_string(),
    language: self.language_name().to_string(),
});

return Err(CompilationError::BuildFailed {
    language: self.language_name().to_string(),
    reason: "Specific reason".to_string(),
});
```

#### Plugin Info Structure

Provide comprehensive plugin information:

```rust
let info = PluginInfo {
    name: "language".to_string(),
    version: env!("CARGO_PKG_VERSION").to_string(),
    description: "Clear description of what this plugin does".to_string(),
    author: "Wasmrun Team".to_string(),
    extensions: vec!["ext1".to_string(), "ext2".to_string()],
    entry_files: vec!["main.ext".to_string(), "build.config".to_string()],
    plugin_type: PluginType::Builtin,
    source: None,
    dependencies: vec![], // External tool dependencies
    capabilities: PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false, // Set to true if supports web apps
        live_reload: true,
        optimization: true, // Set to false if not supported
        custom_targets: vec!["target1".to_string()],
    },
};
```

## 🛠️ Development Setup

### Prerequisites

**Required Tools:**
```sh
# Just task runner (recommended)
cargo install just

# WebAssembly target (for testing)
rustup target add wasm32-unknown-unknown
```

**Optional Tools for Plugin Testing:**
```sh
# For C/C++ plugin testing
# Install Emscripten from: https://emscripten.org/

# For Go plugin testing
# Install TinyGo from: https://tinygo.org/

# For Rust web development
cargo install wasm-pack
cargo install trunk

# For AssemblyScript plugin testing
npm install -g assemblyscript
```

### Getting Started

1. **Clone and build**:
```sh
git clone https://github.com/anistark/wasmrun.git
cd wasmrun
just build  # Or: cargo build --release
```

2. **Run tests**:
```sh
just test          # Run all tests

# Run specific test modules
cargo test plugin::tests
cargo test server::tests -- --test-threads=1
```

3. **Code quality**:
```sh
just format        # Format code with rustfmt
just lint          # Run clippy lints
```

## 📝 Development Workflow

### Using Just Commands

Wasmrun uses a `justfile` for common development tasks:

```sh
# Development commands
just build           # Build in release mode
just test            # Run all tests
just format          # Format code with rustfmt
just lint            # Run clippy lints
just clean           # Clean build artifacts

# Plugin testing commands
just run WASM_FILE   # Test with a WASM file
just stop            # Stop running servers

# Release commands [For Maintainers only]
just prepare-publish # Prepare for publishing
just publish         # Publish to crates.io and GitHub
```

### Code Style Guidelines

1. **Formatting**: Use `rustfmt` with default settings (`just format`)
2. **Linting**: All clippy warnings must be addressed (`just lint`)
3. **Error Handling**: Use the centralized `WasmrunError` types in `src/error.rs`
4. **Documentation**: Add doc comments for public APIs and complex logic
5. **Testing**: Add tests for new functionality, ensure they don't hang
6. **Plugin Integration**: Use shared utilities from `utils/` modules

### File Organization Guidelines

When adding new functionality:

- **Commands**: Add to `src/commands/` if it's a CLI command
- **Plugin Languages**: Add to `src/plugin/languages/` for built-in plugins
- **Server Features**: Add to `src/server/` for web server functionality
- **Utilities**: Add to `src/utils/` for shared functionality
- **Templates**: Add to `src/template/` for HTML/CSS/JS resources
- **Tests**: Co-locate with the module being tested or in `tests/` for integration tests

## 🚀 Contributing Process

### 1. Getting Started

**Fork and branch**:
```sh
git checkout -b feature/my-new-plugin
git checkout -b feature/enhance-rust-plugin
```

### 2. Development and Testing

**Develop and test**:
```sh
# Make your changes
just format           # Format code
just lint             # Check lints
just test             # Run tests
```

### 3. Documentation Updates

**Update documentation**:
- Update relevant documentation in README.md if needed
- Add tests for new plugin functionality
- Update this CONTRIBUTING.md if adding new patterns

### PR Guidelines

- **Clear description**: Explain what your changes do and why
- **Reference issues**: Link to any related GitHub issues
- **Include testing steps**: Show how to test your plugin changes
- **Breaking changes**: Clearly mark any breaking changes
- **Performance impact**: Note any performance considerations
- **Plugin compatibility**: Ensure changes don't break other plugins

### PR Review Checklist

- [ ] Code follows style guidelines (`just format` && `just lint`)
- [ ] All tests pass (`just test`)
- [ ] New functionality includes tests
- [ ] Documentation is updated if needed
- [ ] No hanging server tests (cfg!(test) guards added)
- [ ] Error messages are user-friendly
- [ ] Performance impact is considered

## 🐛 Bug Reports

When reporting bugs:

1. **Use the issue template** if available
2. **Include system information**:
   - OS and version
   - Rust version (`rustc --version`)
   - Wasmrun version (`wasmrun --version`)
3. **Include plugin information**:
   - Which plugin is affected (`wasmrun plugin list`)
4. **Provide reproduction steps**
5. **Include relevant output** with `WASMRUN_DEBUG=1` if possible
6. **Attach example files** if applicable
7. **Attach screenshots** if applicable

### Plugin-Specific Bug Reports

For plugin-related issues:

```sh
# Get plugin information
wasmrun plugin info [plugin-name]

# Test with verbose output
WASMRUN_DEBUG=1 wasmrun run ./project --language [plugin-name] --verbose
```

## 💡 Feature Requests

When requesting features:

1. **Check existing issues** to avoid duplicates
2. **Explain the use case** and why it's valuable
3. **Provide examples** of how it would work
4. **Consider plugin architecture** - could it be a new plugin?
5. **Consider implementation complexity**
6. **Be open to alternative solutions**

### Plugin Enhancement Requests

For plugin-specific features:

- **Identify target plugin** - which plugin needs enhancement?
- **Plugin compatibility** - how would it affect other plugins?
- **Shared utilities** - could it benefit multiple plugins?

## 🧪 Testing Guidelines

### Running Tests

```sh
# Run all tests
just test

# Run specific test categories
cargo test --lib                         # Unit tests
cargo test --test integration            # Integration tests
cargo test plugin::                      # Plugin tests
cargo test server:: -- --test-threads=1  # Server tests (single-threaded)
```

### Test Data and Examples

Create realistic test scenarios:

```sh
# Test project structures should mirror real-world usage
tests/
├── fixtures/
│   ├── rust_project/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── c_project/
│   │   ├── Makefile
│   │   └── main.c
│   └── mylang_project/
│       ├── main.ml
│       └── mylang.config
└── integration/
    ├── plugin_tests.rs
    └── server_tests.rs
```

## 🔧 Advanced Development Topics

### Plugin System Architecture

The plugin system is designed with these principles:

1. **Trait-based**: All plugins implement the same `Plugin` and `WasmBuilder` traits
2. **Dynamic Loading**: External plugins are loaded at runtime via dynamic libraries
3. **Configuration-driven**: Plugin behavior is controlled through structured configuration
4. **Error Propagation**: Consistent error handling across all plugins

### Adding New Command

To add a new CLI command:

1. **Create command file**:
```rust
// src/commands/my_command.rs
use crate::error::Result;

pub fn run_my_command(arg: &str) -> Result<()> {
    println!("Running my command with: {}", arg);
    Ok(())
}
```

2. **Update command module**:
```rust
// src/commands/mod.rs
pub mod my_command;
pub use my_command::run_my_command;
```

3. **Add to CLI definition**:
```rust
// src/cli.rs
#[derive(Parser)]
pub enum Commands {
    // ... existing commands
    MyCommand {
        arg: String,
    },
}
```

4. **Handle in main**:
```rust
// src/main.rs
Commands::MyCommand { arg } => {
    commands::run_my_command(&arg)?;
}
```

### Extending Server Functionality

To add new server endpoints:

1. **Add handler**:
```rust
// src/server/handler.rs
fn handle_my_endpoint(request: &Request<()>) -> Response<std::io::Cursor<Vec<u8>>> {
    let response_body = "My custom response";
    create_response(200, "text/plain", response_body.as_bytes())
}
```

2. **Register route**:
```rust
// In the main request handler
"/my-endpoint" => handle_my_endpoint(request),
```

### Plugin Configuration Schema

External plugins use a standardized configuration format:

```toml
# wasmrun.toml
[plugin]
name = "plugin-name"
version = "1.0.0"
description = "Plugin description"
author = "Author Name"

[dependencies]
system = ["tool1 >= 1.0", "tool2"]
wasmrun = "0.10.0"

[capabilities]
compile_wasm = true
compile_webapp = false
live_reload = true
optimization = true
custom_targets = ["wasm32-wasi", "wasm32-unknown-unknown"]

[settings]
default_optimization = "size"
entry_files = ["main.ext", "app.ext"]
file_extensions = ["ext", "extension"]
```

## 📄 License

By contributing to Wasmrun, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).

---

**Thank you for contributing to Wasmrun! You're helping make WebAssembly development more accessible and enjoyable for everyone! 🚀**

*Remember: Every contribution matters, whether it's code, documentation, bug reports, new plugins, or spreading the word about the project. 🙌*
