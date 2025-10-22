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
│       └── c_plugin.rs       # C/C++ plugin with Emscripten
├── server/                   # HTTP server and web interface
│   ├── mod.rs                # Server module exports
│   ├── config.rs             # Server configuration and setup
│   ├── handler.rs            # HTTP request handling
│   ├── wasm.rs               # WASM file serving
│   └── utils.rs              # Server utilities
├── template.rs               # Template management system for web UI
│   # Templates are stored in root templates/ directory
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
just format          # Format both Rust and TypeScript/JavaScript code
just lint            # Run clippy lints and ESLint
just type-check      # Run TypeScript type checking
just clean           # Clean build artifacts

# Plugin testing commands
just run WASM_FILE   # Test with a WASM file
just stop            # Stop running servers

# Release commands [For Maintainers only]
just prepare-publish # Prepare for publishing
just publish         # Publish to crates.io and GitHub
```

### Pre-Commit Checklist

**Before pushing your changes, ensure:**

```sh
# 1. Format code
just format
# or
cargo fmt --all

# 2. Run lints (must pass with no warnings)
just lint
# or
cargo clippy --all-targets --all-features -- -D warnings

# 3. Run tests locally
just test
# or
cargo test --all-features

# 4. Type check TypeScript (if UI changes)
just type-check
```

### Github Workflows

All CI workflows are in `.github/workflows/`:
- `ci.yml` - Format and lint checks
- `test.yml` - Test execution
- `examples.yml` - Example projects compilation

### UI Development Workflow

The `wasmrun-ui` is built with [Preact](https://preactjs.com/), [TypeScript](https://www.typescriptlang.org/), and [Tailwind CSS](https://tailwindcss.com/):

```sh
# UI development setup (from ui/ directory)
cd ui
pnpm install         # Install UI dependencies
pnpm dev             # Start UI development server

# UI development commands (from project root)
just format          # Formats both Rust and TypeScript code
just lint            # Lints both Rust and TypeScript code  
just type-check      # Runs TypeScript type checking

# UI is automatically built and embedded during main build
just build           # Does all checks and builds with ui
```

### UI Component Development

When working on UI components:

1. **Component Structure**: Follow existing patterns in `ui/src/components/`
2. **Styling**: Use Tailwind CSS classes for consistent design
3. **Type Safety**: Ensure all props and state are properly typed
4. **Testing**: Components are tested through integration tests
5. **Hooks**: Use React hooks following established patterns (`useCallback`, `useEffect`)

The UI is embedded into the Rust binary during build, so changes require a full rebuild to test in wasmrun.

### Code Style Guidelines

1. **Formatting**: Use `rustfmt` and `prettier` with default settings (`just format`)
2. **Linting**: All clippy warnings and ESLint errors must be addressed (`just lint`)
3. **Type Checking**: All TypeScript code must pass type checking (`just type-check`)
4. **Error Handling**: Use the centralized `WasmrunError` types in `src/error.rs`
5. **Documentation**: Add doc comments for public APIs and complex logic
6. **Testing**: Add tests for new functionality, ensure they don't hang
7. **Plugin Integration**: Use shared utilities from `utils/` modules
8. **UI Components**: Follow React hooks best practices and TypeScript typing conventions

### File Organization Guidelines

When adding new functionality:

- **Commands**: Add to `src/commands/` if it's a CLI command
- **Plugin Languages**: Add to `src/plugin/languages/` for built-in plugins
- **Server Features**: Add to `src/server/` for web server functionality
- **Utilities**: Add to `src/utils/` for shared functionality
- **Templates**: Managed via `src/template.rs`, files stored in root `templates/` directory
- **UI Components**: Add to `ui/` directory for React/TypeScript web interface components
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

#### Code Quality
- [ ] Code follows style guidelines (`just format` && `just lint`)
- [ ] TypeScript code passes type checking (`just type-check`)
- [ ] All tests pass locally (`just test`)
- [ ] No clippy warnings (`cargo clippy --all-targets --all-features -- -D warnings`)
- [ ] Code is formatted (`cargo fmt --all -- --check`)

#### Testing
- [ ] New functionality includes tests
- [ ] Tests don't hang (use `cfg!(test)` guards for server tests)
- [ ] Tests are cross-platform compatible
- [ ] Integration tests use appropriate timeouts

#### Documentation
- [ ] Documentation is updated if needed
- [ ] Doc comments added for public APIs
- [ ] Examples updated if APIs changed
- [ ] CHANGELOG.md updated for notable changes

#### CI/CD
- [ ] All CI workflows pass (format, lint, tests, examples)
- [ ] Examples compile on both Ubuntu and Windows
- [ ] No new dependency conflicts
- [ ] Breaking changes are clearly marked

#### Performance & Compatibility
- [ ] Error messages are user-friendly
- [ ] Performance impact is considered
- [ ] Cross-platform compatibility verified
- [ ] UI changes work in all supported modes (console, app)
- [ ] Binary size increase is reasonable

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

# UI-related testing
just type-check
just lint
just format
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

## 🐛 Debug Mode for Development

Wasmrun includes a comprehensive debug mode that provides detailed logging to help with development and troubleshooting.

### Using Debug Mode

Enable debug logging with the `--debug` flag:

```sh
# Enable debug mode for any command
wasmrun --debug
```

### Debug Output Types

Debug mode provides several types of logging information:

1. **🔍 DEBUG** - General debug information with file and line numbers
2. **🔬 TRACE** - Very detailed tracing for complex operations
3. **🚪 ENTER** - Function entry points with parameters
4. **🚶 EXIT** - Function exit points with return values
5. **⏱️ TIME** - Performance timing for slow operations

### Debug Categories

Debug logs are organized by system component:

- **CLI & Arguments**: Command parsing and validation
- **Plugin System**: Plugin loading, detection, and execution
- **Server Operations**: HTTP server startup, request handling
- **Compilation**: Build processes, tool detection, file operations
- **File Operations**: Path resolution, file validation

### Using Debug Logs for Development

When developing plugins or debugging issues:

```sh
# Debug a specific operation
wasmrun --debug compile ./my-project 2> debug.log

# Debug plugin detection
wasmrun --debug plugin list --all 2> plugin_debug.log

# Debug server startup issues
wasmrun --debug run ./project --port 3000 2> server_debug.log
```

### Debug Output Example

```
🚪 ENTER [main.rs:35] main - args = Args { command: Some(Run { ... }), debug: true }
🔍 DEBUG [main.rs:97] Processing run command: port=8420, language=None, watch=false
🚪 ENTER [server/mod.rs:77] run_project - path=./project, port=8420, language_override=None, watch=false
🔍 DEBUG [server/mod.rs:86] Checking path type: "./project"
🔍 DEBUG [detect.rs:28] detect_project_language - project_path=./project
🔍 DEBUG [detect.rs:39] Checking for language-specific configuration files
🔍 DEBUG [detect.rs:41] Found Cargo.toml - detected Rust project
🚶 EXIT  [detect.rs:43] detect_project_language -> Rust
⏱️ TIME  [server/mod.rs:134] Project compilation took 2.34s
🚶 EXIT  [main.rs:164] main - exit code: 0
```

### Contributing Debug Improvements

When adding debug logging to your code:

```rust
use crate::{debug_enter, debug_exit, debug_println, debug_time, trace_println};

pub fn my_function(param: &str) -> Result<String> {
    debug_enter!("my_function", "param={}", param);
    
    // Add detailed debug info for complex operations
    debug_println!("Processing parameter: {}", param);
    
    // Use trace for very detailed logs
    trace_println!("Internal state: {:?}", internal_state);
    
    // Time expensive operations
    let result = debug_time!("expensive_operation", {
        expensive_computation(param)
    });
    
    debug_exit!("my_function", &result);
    Ok(result)
}
```

### Debug Best Practices

1. **Function Boundaries**: Use `debug_enter!` and `debug_exit!` for important functions
2. **Error Contexts**: Add debug info before operations that might fail
3. **Performance**: Use `debug_time!` for potentially slow operations
4. **State Changes**: Log important state transitions
5. **File Operations**: Debug file paths and validation results

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

## 📚 Examples Development Guide

The `examples/` directory contains sample WebAssembly projects for different programming languages. These serve as learning resources and testing grounds for Wasmrun's capabilities.

### Available Examples

| Language | Directory | Capabilities |
| --- | --- | --- |
| 🦀 **Rust** | `rust-hello/` | wasm-bindgen, browser APIs, memory management |
| 🐹 **Go** | `go-hello/` | syscall/js, concurrency, time operations |
| 🔧 **C** | `c-hello/` | Emscripten, math library, manual memory |
| 🚀 **AssemblyScript** | `asc-hello/` | TypeScript syntax, performance optimization |

### Testing Examples

Examples work with standard wasmrun commands:

```sh
# Test compilation and running
wasmrun examples/rust-hello
wasmrun compile examples/go-hello

# Verify WASM output
wasmrun verify examples/rust-hello/target/wasm32-unknown-unknown/release/*.wasm
```

### Creating a New Example

When adding a new language example:

1. **Create directory**: `examples/language-hello/`
2. **Add source code** with standard functions: `greet()`, `fibonacci()`, `sum_array()`
3. **Include build config**: Language-specific (Cargo.toml, package.json, etc.)
4. **Write README.md** with usage instructions
5. **Test**: `wasmrun run examples/language-hello`

### Example Requirements

- Works with standard wasmrun commands
- Implements common functions for consistency  
- Includes comprehensive README.md
- Tests successfully in browser console

## 📄 License

By contributing to Wasmrun, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).

---

**Thank you for contributing to Wasmrun! You're helping make WebAssembly development more accessible and enjoyable for everyone! 🚀**

*Remember: Every contribution matters, whether it's code, documentation, bug reports, new plugins, or spreading the word about the project. 🙌*
