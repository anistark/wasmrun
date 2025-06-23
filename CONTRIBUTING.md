# Contributing to Chakra

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) 

Thank you for considering contributing to Chakra! This guide will help you understand the project structure, development workflow, and how to make meaningful contributions.

## 🏗️ Project Architecture

Chakra is designed with a modular architecture that separates concerns clearly:

```sh
src/
├── cli.rs              # Command line interface and argument parsing
├── main.rs             # Application entry point and command routing
├── error.rs            # Centralized error handling with user-friendly messages
├── ui.rs               # User interface utilities and styled output
├── watcher.rs          # File system watching for live reload functionality
├── commands/           # Command implementations
│   ├── verify.rs       # WASM verification and inspection
│   ├── compile.rs      # Project compilation with optimization options
│   ├── run.rs          # Development server and project execution
│   ├── clean.rs        # Build artifact cleanup
│   ├── init.rs         # Project initialization (TODO)
│   └── stop.rs         # Server management
├── compiler/           # Legacy compilation system (being phased out)
│   ├── builder.rs      # Build configuration and result types
│   └── detect.rs       # Project type detection utilities
├── plugin/             # 🔌 Plugin system
│   ├── mod.rs          # Plugin manager and core traits
│   ├── registry.rs     # Plugin registry and discovery
│   ├── external.rs     # External plugin loading (TODO)
│   └── languages/      # Built-in language plugins
│       ├── rust_plugin.rs      # Rust plugin
│       ├── go_plugin.rs        # Go plugin with TinyGo support
│       ├── c_plugin.rs         # C/C++ plugin with Emscripten
│       ├── asc_plugin.rs       # AssemblyScript plugin
│       └── python_plugin.rs    # Python plugin with Pyodide
├── server/             # HTTP server and web interface
│   ├── config.rs       # Server configuration and setup
│   ├── handler.rs      # HTTP request handling
│   ├── wasm.rs         # WASM file serving
│   ├── webapp.rs       # Web application support
│   └── utils.rs        # Server utilities
├── template/           # HTML, CSS, and JavaScript templates
│   ├── server/         # WASM runner interface templates
│   └── webapp/         # Web application templates
└── utils/              # Shared utilities and helpers
    ├── path.rs         # Path resolution and validation
    └── command.rs      # Shared command execution utilities
```

## 🔌 Plugin Architecture Overview

Chakra's new plugin system provides:

- **Unified Interface** - All plugins implement the same `Plugin` and `WasmBuilder` traits
- **Self-Contained** - Each plugin handles both metadata and compilation logic
- **Shared Utilities** - Common functionality through `CommandExecutor`
- **Extensible** - Easy to add new language support

### Plugin Structure

Each plugin is a single struct that implements both traits:

```rust
pub struct RustPlugin {
    info: PluginInfo,
}

impl Plugin for RustPlugin {
    // Plugin metadata and project detection
}

impl WasmBuilder for RustPlugin {
    // Compilation and build logic
}
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
git clone https://github.com/anistark/chakra.git
cd chakra
just build  # Or: cargo build --release
```

2. **Run tests**:
```sh
just test          # Run all tests

# Run specific test modules
cargo test plugin::tests
cargo test server::tests -- --test-threads=1
```

3. **Test plugins**:
```sh
# Test plugin detection and dependencies
chakra plugin list

# Test specific plugins
just example-wasm-rust    # Test Rust plugin
just run ./examples/rust_example.wasm

just example-wasm-emcc   # Test C plugin (if emcc available)
just run ./examples/simple.wasm
```

4. **Code quality**:
```sh
just format        # Format code with rustfmt
just lint          # Run clippy lints
```

## 📝 Development Workflow

### Using Just Commands

Chakra uses a `justfile` for common development tasks:

```sh
# Development commands
just build           # Build in release mode
just test            # Run all tests
just format          # Format code with rustfmt
just lint            # Run clippy lints
just clean           # Clean build artifacts

# Plugin testing commands
just run WASM_FILE   # Test with a WASM file
just example-wasm    # Generate test WASM files
just stop            # Stop running servers

# Release commands [For Maintainers only]
just prepare-publish # Prepare for publishing
just publish         # Publish to crates.io and GitHub
```

### Code Style Guidelines

1. **Formatting**: Use `rustfmt` with default settings (`just format`)
2. **Linting**: All clippy warnings must be addressed (`just lint`)
3. **Error Handling**: Use the centralized `ChakraError` types in `src/error.rs`
4. **Documentation**: Add doc comments for public APIs and complex logic
5. **Testing**: Add tests for new functionality, ensure they don't hang
6. **User Experience**: Focus on helpful error messages and clear output
7. **Plugin Consistency**: Use `CommandExecutor` for shared operations

## 🧪 Adding New Features

### Adding a New Command

1. **Create command file** in `src/commands/`
2. **Add to CLI** in `src/cli.rs`
3. **Add to main router** in `src/main.rs`
4. **Export from commands module** in `src/commands/mod.rs`

### 🔌 Adding a New Plugin

TBD

### Enhancing Existing Plugins

To enhance an existing plugin:

1. **Identify the plugin file** in `src/plugin/languages/`
2. **Add new functionality** to the implementation
3. **Use shared utilities** from `CommandExecutor` when possible
4. **Update plugin capabilities** in the `PluginInfo`
5. **Add tests** for new functionality

### Enhancing the Web Interface

#### Server Templates (WASM Runner)

To modify the WASM runner interface:

1. **HTML**: Edit `src/template/server/index.html`
2. **CSS**: Edit `src/template/server/style.css` 
3. **JavaScript**: Edit `src/template/server/scripts.js`
4. **Chakra WASI Implementation**: Edit `src/template/server/chakra_wasi_impl.js`

#### Web App Templates (Framework Support)

To modify the web application interface:

1. **HTML**: Edit `src/template/webapp/index.html`
2. **CSS**: Edit `src/template/webapp/style.css`
3. **JavaScript**: Edit `src/template/webapp/scripts.js`

**Note**: Templates are embedded at compile time, so changes require rebuilding.

### Testing Your Changes

1. **Unit tests**:
```sh
just test
cargo test my_plugin::tests
```

2. **Plugin integration testing**:
```sh
# Test plugin detection
chakra plugin list

# Test specific plugin functionality
mkdir test-project && cd test-project
# Create project files for your plugin
chakra run . --language your-plugin
```

3. **Manual testing**:
```sh
# Test different project types
mkdir test-rust && cd test-rust
cargo init --bin
echo 'fn main() { println!("Hello WASM!"); }' > src/main.rs
chakra run . --watch

# Test plugin selection
chakra verify ./examples/rust_example.wasm --detailed
chakra inspect ./examples/rust_example.wasm
chakra compile ./test-rust --optimization size
```

## 🤝 Pull Request Process

### Before Submitting

1. **Fork and branch**:
```sh
git checkout -b feature/my-new-plugin
git checkout -b feature/enhance-rust-plugin
```

2. **Develop and test**:
```sh
# Make your changes
just format           # Format code
just lint             # Check lints
just test             # Run tests
just test-plugins     # Test plugin functionality
just example-wasm     # Test with examples
```

3. **Update documentation**:
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
   - Chakra version (`chakra --version`)
3. **Include plugin information**:
   - Which plugin is affected (`chakra plugin list`)
4. **Provide reproduction steps**
5. **Include relevant output** with `CHAKRA_DEBUG=1` if possible
6. **Attach example files** if applicable
7. **Attach screenshots** if applicable

### Plugin-Specific Bug Reports

For plugin-related issues:

```sh
# Get plugin information
chakra plugin info [plugin-name]

# Test with verbose output
CHAKRA_DEBUG=1 chakra run ./project --language [plugin-name] --verbose
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
- **Performance impact** - especially for compilation speed

_If you feel unsure about it, feel free to [open a discussion](https://github.com/anistark/chakra/discussions)._

## 📚 Resources

### About Chakra

- ✨ [Chakra: A Wasm Runtime](https://blog.anirudha.dev/chakra)

### Learning WebAssembly

- [WebAssembly Official Site](https://webassembly.org/)
- [WASI Specification](https://github.com/WebAssembly/WASI)
- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/)

### Plugin Development

- [Rust Trait Objects](https://doc.rust-lang.org/book/ch17-02-trait-objects.html)
- [Error Handling in Rust](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Testing in Rust](https://doc.rust-lang.org/book/ch11-00-testing.html)

### Language-Specific Resources

- **Rust**: [The Rust Programming Language](https://doc.rust-lang.org/book/)
- **Go**: [TinyGo Documentation](https://tinygo.org/docs/)
- **C/C++**: [Emscripten Documentation](https://emscripten.org/docs/)
- **AssemblyScript**: [AssemblyScript Book](https://www.assemblyscript.org/)
- **Python**: [Waspy](https://github.com/anistark/waspy)

### Rust Resources

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Clap Documentation](https://docs.rs/clap/latest/clap/)

### Project-Specific

- [Chakra Issues](https://github.com/anistark/chakra/issues)
- [Chakra Discussions](https://github.com/anistark/chakra/discussions)

## 🧪 Plugin Development Best Practices

### Using Shared Utilities

Always use `CommandExecutor` for common operations:

```rust
// ✅ Good - use shared utilities
use crate::utils::CommandExecutor;

let output = CommandExecutor::execute_command("tool", &args, &dir, verbose)?;
let copied = CommandExecutor::copy_to_output(&source, &output_dir, "Language")?;
if CommandExecutor::is_tool_installed("tool") { /* ... */ }

// ❌ Bad - duplicate implementation
fn my_execute_command() { /* ... */ }
```

### Error Handling

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

### Plugin Info Structure

Provide comprehensive plugin information:

```rust
let info = PluginInfo {
    name: "language".to_string(),
    version: env!("CARGO_PKG_VERSION").to_string(),
    description: "Clear description of what this plugin does".to_string(),
    author: "Chakra Team".to_string(),
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

## 📄 License

By contributing to Chakra, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).

---

**Thank you for contributing to Chakra! You're helping make WebAssembly development more accessible and enjoyable for everyone! 🚀**

*Remember: Every contribution matters, whether it's code, documentation, bug reports, new plugins, or spreading the word about the project. 🙌*
