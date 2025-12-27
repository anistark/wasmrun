# Contributing

We welcome contributions to Wasmrun! This guide will help you get started with contributing code, documentation, plugins, and more.

## Development Setup

### Prerequisites

**Required Tools:**

```bash
# Just task runner (recommended)
cargo install just

# WebAssembly target (for testing)
rustup target add wasm32-unknown-unknown
```

**Optional Tools for Plugin Testing:**

```bash
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

1. **Fork and Clone**:

```bash
git clone https://github.com/yourusername/wasmrun.git
cd wasmrun
```

2. **Build**:

```bash
just build  # Or: cargo build --release
```

3. **Run Tests**:

```bash
just test          # Run all tests

# Run specific test modules
cargo test plugin::tests
cargo test server::tests -- --test-threads=1
```

4. **Code Quality**:

```bash
just format        # Format code with rustfmt
just lint          # Run clippy lints
```

## Development Workflow

### Using Just Commands

Wasmrun uses a `justfile` for common development tasks:

```bash
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

# Release commands (for maintainers)
just prepare-publish # Prepare for publishing
just publish         # Publish to crates.io and GitHub
```

### Pre-Commit Checklist

Before pushing your changes, ensure:

```bash
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

### GitHub Workflows

All CI workflows are in `.github/workflows/`:
- `ci.yml` - Format and lint checks
- `test.yml` - Test execution
- `examples.yml` - Example projects compilation

## Code Style Guidelines

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

## UI Development Workflow

The `wasmrun-ui` is built with [Preact](https://preactjs.com/), [TypeScript](https://www.typescriptlang.org/), and [Tailwind CSS](https://tailwindcss.com/):

```bash
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

## Contributing Process

### 1. Create a Branch

```bash
git checkout -b feature/my-new-plugin
git checkout -b feature/enhance-rust-plugin
```

### 2. Development and Testing

```bash
# Make your changes
just format           # Format code
just lint             # Check lints
just test             # Run tests
```

### 3. Documentation Updates

- Update relevant documentation in README.md if needed
- Add tests for new plugin functionality
- Update CONTRIBUTING.md if adding new patterns

### 4. Create Pull Request

- **Clear description**: Explain what your changes do and why
- **Reference issues**: Link to any related GitHub issues
- **Include testing steps**: Show how to test your plugin changes
- **Breaking changes**: Clearly mark any breaking changes
- **Performance impact**: Note any performance considerations
- **Plugin compatibility**: Ensure changes don't break other plugins

## PR Review Checklist

### Code Quality
- [ ] Code follows style guidelines (`just format` && `just lint`)
- [ ] TypeScript code passes type checking (`just type-check`)
- [ ] All tests pass locally (`just test`)
- [ ] No clippy warnings (`cargo clippy --all-targets --all-features -- -D warnings`)
- [ ] Code is formatted (`cargo fmt --all -- --check`)

### Testing
- [ ] New functionality includes tests
- [ ] Tests don't hang (use `cfg!(test)` guards for server tests)
- [ ] Tests are cross-platform compatible
- [ ] Integration tests use appropriate timeouts

### Documentation
- [ ] Documentation is updated if needed
- [ ] Doc comments added for public APIs
- [ ] Examples updated if APIs changed
- [ ] CHANGELOG.md updated for notable changes

### CI/CD
- [ ] All CI workflows pass (format, lint, tests, examples)
- [ ] Examples compile on both Ubuntu and Windows
- [ ] No new dependency conflicts
- [ ] Breaking changes are clearly marked

### Performance & Compatibility
- [ ] Error messages are user-friendly
- [ ] Performance impact is considered
- [ ] Cross-platform compatibility verified
- [ ] UI changes work in all supported modes (console, app)
- [ ] Binary size increase is reasonable

## Bug Reports

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

```bash
# Get plugin information
wasmrun plugin info [plugin-name]

# Test with verbose output
WASMRUN_DEBUG=1 wasmrun run ./project --language [plugin-name] --verbose
```

## Feature Requests

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

## Testing Guidelines

### Running Tests

```bash
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

```
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

## Examples Development Guide

The `examples/` directory contains sample WebAssembly projects for different programming languages. These serve as learning resources and testing grounds for Wasmrun's capabilities.

### Available Examples

| Language | Directory | Capabilities |
| --- | --- | --- |
| Rust | `rust-hello/` | wasm-bindgen, browser APIs, memory management |
| Go | `go-hello/` | syscall/js, concurrency, time operations |
| C | `c-hello/` | Emscripten, math library, manual memory |
| AssemblyScript | `asc-hello/` | TypeScript syntax, performance optimization |

### Testing Examples

Examples work with standard wasmrun commands:

```bash
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

## Advanced Development Topics

### Adding New Commands

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

## License

By contributing to Wasmrun, you agree that your contributions will be licensed under the project's [MIT license](https://github.com/anistark/wasmrun/blob/main/LICENSE).

Thank you for contributing to Wasmrun! Every contribution matters, whether it's code, documentation, bug reports, new plugins, or spreading the word about the project.

## Next Steps

- **[Creating Plugins](creating-plugins.md)**: Learn to create your own plugins
- **[Debugging](debugging.md)**: Debug your contributions effectively
- **[Architecture](architecture.md)**: Understand the codebase structure
