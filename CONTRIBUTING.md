# Contributing to Chakra

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) 

Thank you for considering contributing to Chakra! This guide will help you understand the project structure and development workflow.

## Project Structure

```sh
src
├── cli.rs              # Command line argument handling
├── main.rs             # Application entry point
├── server.rs           # HTTP server implementation
├── template            # HTML, CSS, JS templates
│   ├── mod.rs          # Template module exports
│   └── server          # Web server templates
│       ├── index.html  # Main HTML template
│       ├── mod.rs      # Server template module
│       ├── scripts.js  # Browser JavaScript
│       └── style.css   # CSS styles
├── utils.rs            # Utility functions
└── compiler            # WebAssembly compilation module
    ├── detect.rs       # Language detection functionality
    ├── language        # Language-specific implementations
    │   ├── asc.rs      # AssemblyScript compiler
    │   ├── c.rs        # C compiler (via Emscripten)
    │   ├── go.rs       # Go compiler (via TinyGo)
    │   ├── mod.rs      # Language module exports
    │   ├── python.rs   # Python compiler
    │   └── rust.rs     # Rust compiler (wasm32 target)
    └── mod.rs          # Main WebAssembly builder interface
```

## Development Setup

1. Clone the repository:

```sh
git clone https://github.com/anistark/chakra.git
cd chakra
```

2. Install just (task runner):

```sh
cargo install just
```

3. Build the project:

```sh
just build
```

4. Run in development mode:

```sh
just run /path/to/your/test.wasm
```

## Using Just

Chakra uses a `justfile` for common development tasks:

```sh
# List all available commands
just

# Build the project
just build

# Run with a test WASM file
just run ./path/to/file.wasm

# Stop any running server
just stop

# Format code
just format

# Lint code
just lint

# Run tests
just test
```

## Template System

Chakra uses a simple template system that embeds HTML, CSS, and JavaScript files at compile time:

- The `template/server/mod.rs` file provides the `generate_html()` function
- Templates are embedded using `include_str!()` macros
- For development, there's also a `generate_html_dev()` function that loads templates at runtime

To modify templates:
1. Edit files in `src/template/server/`
2. Rebuild the project with `cargo build`

## Adding New Features

### Adding CLI Options

CLI options are defined in `cli.rs` using clap's derive features:

```rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    // Add new options here
}
```

### Enhancing the Web UI

To modify the web interface:
1. Edit `template/server/index.html`, `style.css`, or `scripts.js`
2. Rebuild the project

### WebAssembly Support

The WebAssembly loading and execution is handled in `scripts.js`. When enhancing WebAssembly support:

1. Test with various WASM modules
2. Consider edge cases (missing exports, different compilation targets)
3. Provide helpful error messages

## Testing

Test Chakra with different types of WebAssembly files:

1. Simple C/C++ compiled files: `just example-wasm-emcc`
2. Rust WASM files (with and without wasm-bindgen): `just example-wasm-rust`
3. Files with different entry points

## Building for Release

```sh
cargo build --release
```

## Code Style

- Follow Rust's standard naming conventions
- Use comments for complex logic
- Prefer descriptive error messages

## Release Process

To release a new version of Chakra:

1. Update the version in `Cargo.toml`
2. Use the just commands to handle the release:

```sh
# Ensure everything builds
just prepare-publish

# Publish to crates.io and create a GitHub release
just publish
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Run quality checks with just:
   ```sh
   just format      # Format code
   just lint        # Run clippy lints
   just test        # Run tests
   ```
6. Submit a pull request

## Development Notes

### Server Implementation

Chakra uses `tiny_http` for a minimal HTTP server that:
- Serves the HTML page with embedded CSS and JS
- Serves the WASM file with the correct MIME type
- Handles basic error responses

### Process Management

- The server's PID is stored in `/tmp/chakra_server.pid`
- The `stop` command uses this PID to terminate any running server

### Debugging Tips

When working with WASM loading:
- Use browser developer tools to check network requests
- Look for JavaScript errors in the browser console
- Test with simple WASM files first

## License

By contributing to Chakra, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).
