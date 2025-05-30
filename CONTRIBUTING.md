# Contributing to Chakra

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) 

Thank you for considering contributing to Chakra! This guide will help you understand the project structure and development workflow.

## 🏗️ Project Architecture

Chakra is designed with a modular architecture that separates concerns clearly:

```sh
src/
├── cli.rs              # Command line interface and argument parsing
├── main.rs             # Application entry point and command routing
├── ui.rs               # User interface utilities and styled output
├── verify.rs           # WASM file verification and binary analysis
├── watcher.rs          # File system watching for live reload
├── commands/           # Command implementations (one per file)
├── compiler/           # Multi-language compilation system
├── server/             # HTTP server and web interface
├── template/           # HTML, CSS, and JavaScript templates
└── utils/              # Shared utilities and helpers
```

## 🛠️ Development Setup

### Prerequisites

```sh
# Just task runner
cargo install just

# Optional: WebAssembly tools for testing
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Getting Started

1. **Clone and build**:
```sh
git clone https://github.com/anistark/chakra.git
cd chakra
just build
```

2. **Run tests**:
```sh
just test
```

3. **Code formatting**:
```sh
just format
just lint
```

4. **Test with examples**:
```sh
just example-wasm-rust
just run ./examples/rust_example.wasm
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

1. **Formatting**: Use `rustfmt` with default settings
2. **Linting**: All clippy warnings must be addressed
3. **Error Handling**: Use `Result<(), String>` for command functions
4. **Documentation**: Add doc comments for public APIs
5. **Testing**: Add tests for new functionality

## 🧪 Adding New Features

### Adding a New Command

1. **Create command file** in `src/commands/`.
2. **Add to CLI** in `src/cli.rs`.
3. **Add to main router** in `src/main.rs`.
4. **Export from commands module** in `src/commands/mod.rs`.

### Adding a New Language

1. **Create language file** in `src/compiler/language/`.
2. **Add to language detection** in `src/compiler/detect.rs`.
3. **Add to builder factory** in `src/compiler/builder.rs`.

### Enhancing the Web Interface

#### Server Templates

To modify the WASM runner interface:

1. **HTML**: Edit `src/template/server/index.html`
2. **CSS**: Edit `src/template/server/style.css` 
3. **JavaScript**: Edit `src/template/server/scripts.js`
4. **WASI**: Edit `src/template/server/chakra_wasi_impl.js`

#### Web App Templates

To modify the web application interface:

1. **HTML**: Edit `src/template/webapp/index.html`
2. **CSS**: Edit `src/template/webapp/style.css`
3. **JavaScript**: Edit `src/template/webapp/scripts.js`

### Testing Your Changes

1. **Unit tests**:
```sh
just test
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
# Add some code
chakra run . --watch

# Test different commands
chakra verify ./examples/rust_example.wasm --detailed
chakra inspect ./examples/rust_example.wasm
```

## 🤝 Pull Request Process

1. **Fork and branch**:
```sh
git checkout -b feature/my-new-feature
```

2. **Develop and test**:
```sh
# Make your changes
just format
just lint
just test
```

3. **Document changes**:
   - Update relevant documentation
   - Add tests for new functionality
   - Update README if needed

4. **Submit PR**:
   - Clear description of changes
   - Reference any related issues
   - Include testing steps

### PR Review Checklist

- [ ] Code follows style guidelines
- [ ] All tests pass
- [ ] Documentation is updated
- [ ] Breaking changes are clearly marked
- [ ] Performance impact is considered

Checkout the [open issues](https://github.com/anistark/chakra/issues). If you've identified an issue not listed here, feel free to open a new issue for it. 🙌

## 📚 Resources

### Learning WebAssembly

- [WebAssembly Official Site](https://webassembly.org/)
- [WASI Specification](https://github.com/WebAssembly/WASI)
- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/)

### Rust Resources

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Clap Documentation](https://docs.rs/clap/latest/clap/)

## 🆘 Getting Help

- **GitHub Issues**: Report bugs and request features
- **GitHub Discussions**: Ask questions and share ideas

## 📄 License

By contributing to Chakra, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).

---

**Thank you for contributing to Chakra! You're awesome! 😁**
