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
│   ├── init.rs         # Project initialization (planned)
│   └── stop.rs         # Server management
├── compiler/           # Multi-language compilation system
│   ├── builder.rs      # Unified build system with trait-based architecture
│   ├── detect.rs       # Language and project type detection
│   └── language/       # Language-specific builders
│       ├── rust.rs     # Rust compilation with wasm-bindgen support
│       ├── go.rs       # TinyGo compilation
│       ├── c.rs        # Emscripten C/C++ compilation
│       ├── asc.rs      # AssemblyScript compilation
│       └── python.rs   # Python compilation (planned)
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
    └── path.rs         # Path resolution and validation
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

**Optional Tools for Testing:**
```sh
# For C/C++ WASM compilation testing
# Install Emscripten from: https://emscripten.org/

# For Go WASM compilation testing  
# Install TinyGo from: https://tinygo.org/

# For web development
cargo install wasm-pack
cargo install trunk
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
cargo test compiler::tests
cargo test server::tests -- --test-threads=1
```

3. **Code quality**:
```sh
just format        # Format code with rustfmt
just lint          # Run clippy lints
just check         # Check compilation without building
```

4. **Test with examples**:
```sh
just example-wasm-rust    # Generate Rust WASM example
just run ./examples/rust_example.wasm

just example-wasm-emcc   # Generate C WASM example (if emcc available)
just run ./examples/simple.wasm
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

# Testing commands
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

## 🧪 Adding New Features

### Adding a New Command

1. **Create command file** in `src/commands/`
2. **Add to CLI** in `src/cli.rs`
3. **Add to main router** in `src/main.rs`
4. **Export from commands module** in `src/commands/mod.rs`

### Adding a New Language

1. **Create language file** in `src/compiler/language/`
2. **Add to language detection** in `src/compiler/detect.rs`
3. **Add to builder factory** in `src/compiler/builder.rs`

### Enhancing the Web Interface

#### Server Templates (WASM Runner)

To modify the WASM runner interface:

1. **HTML**: Edit `src/template/server/index.html`
2. **CSS**: Edit `src/template/server/style.css` 
3. **JavaScript**: Edit `src/template/server/scripts.js`
4. **WASI Implementation**: Edit `src/template/server/chakra_wasi_impl.js`

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
cargo test my_module::tests
```

2. **Integration testing**:
```sh
# Test with different WASM types
just example-wasm-rust
just run ./examples/rust_example.wasm

just example-wasm-emcc  # If emcc is available
just run ./examples/simple.wasm
```

3. **Manual testing**:
```sh
# Test different project types
mkdir test-rust && cd test-rust
cargo init --bin
echo 'fn main() { println!("Hello WASM!"); }' > src/main.rs
chakra run . --watch

# Test different commands
chakra verify ./examples/rust_example.wasm --detailed
chakra inspect ./examples/rust_example.wasm
chakra compile ./test-rust --optimization size
```

## 🤝 Pull Request Process

### Before Submitting

1. **Fork and branch**:
```sh
git checkout -b feature/my-new-feature
```

2. **Develop and test**:
```sh
# Make your changes
just format           # Format code
just lint             # Check lints
just test             # Run tests
just example-wasm     # Test with examples
```

3. **Update documentation**:
   - Update relevant documentation in README.md if needed
   - Add tests for new functionality
   - Update this CONTRIBUTING.md if adding new patterns

### PR Guidelines

- **Clear description**: Explain what your changes do and why
- **Reference issues**: Link to any related GitHub issues
- **Include testing steps**: Show how to test your changes
- **Breaking changes**: Clearly mark any breaking changes
- **Performance impact**: Note any performance considerations

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
3. **Provide reproduction steps**
4. **Include relevant output** with `CHAKRA_DEBUG=1` if possible
5. **Attach example files** if applicable
6. **Attach screenshots** if applicable

## 💡 Feature Requests

When requesting features:

1. **Check existing issues** to avoid duplicates
2. **Explain the use case** and why it's valuable
3. **Provide examples** of how it would work
4. **Consider implementation complexity**
5. **Be open to alternative solutions**

_If you feel unsure about it, feel free to [open a discussion](https://github.com/anistark/chakra/discussions)._

## 📚 Resources

### Learning WebAssembly

- [WebAssembly Official Site](https://webassembly.org/)
- [WASI Specification](https://github.com/WebAssembly/WASI)
- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/)

### Rust Resources

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Clap Documentation](https://docs.rs/clap/latest/clap/)

### Project-Specific

- [Chakra Issues](https://github.com/anistark/chakra/issues)
- [Chakra Discussions](https://github.com/anistark/chakra/discussions)

## 📄 License

By contributing to Chakra, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).

---

**Thank you for contributing to Chakra! You're helping make WebAssembly development more accessible and enjoyable for everyone! 🚀**

*Remember: Every contribution matters, whether it's code, documentation, bug reports, or spreading the word about the project. 🙌*
