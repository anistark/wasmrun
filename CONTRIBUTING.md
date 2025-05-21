# Contributing to Chakra

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white) 

Thank you for considering contributing to Chakra! This guide will help you understand the project structure and development workflow.

## Project Structure

```sh
src
├── cli.rs              # Command line argument handling
├── main.rs             # Application entry point
├── compiler            # WebAssembly compilation module
│   ├── detect.rs       # Language detection functionality
│   ├── language        # Language-specific implementations
│   │   ├── asc.rs      # AssemblyScript compiler
│   │   ├── c.rs        # C compiler (via Emscripten)
│   │   ├── go.rs       # Go compiler (via TinyGo)
│   │   ├── mod.rs      # Language module exports
│   │   ├── python.rs   # Python compiler
│   │   └── rust.rs     # Rust compiler (wasm32 target)
│   └── mod.rs          # Main WebAssembly builder interface
├── server              # HTTP server implementation (modular)
│   ├── mod.rs          # Server public API and re-exports
│   ├── config.rs       # Server configuration and setup
│   ├── handler.rs      # HTTP request handling
│   ├── utils.rs        # Server utility functions
│   ├── wasm.rs         # WebAssembly file handling
│   └── webapp.rs       # Web application support
├── template            # HTML, CSS, JS templates
│   ├── mod.rs          # Template module exports
│   ├── server          # Web server templates
│   │   ├── chakra_wasi_impl.js  # WASI implementation for browser
│   │   ├── index.html  # Main HTML template
│   │   ├── mod.rs      # Server template module
│   │   ├── scripts.js  # Browser JavaScript
│   │   └── style.css   # CSS styles
│   └── webapp          # Web application templates
│       ├── index.html  # Web app HTML template
│       ├── mod.rs      # Web app template module
│       ├── scripts.js  # Web app JavaScript
│       └── style.css   # Web app CSS styles
├── utils.rs            # Utility functions
├── verify.rs           # WASM file verification
└── watcher.rs          # File watcher for live reload
```

## Module Responsibilities

### Core Modules
- **cli.rs**: Handles command-line argument parsing and application commands
- **main.rs**: The entry point that processes arguments and routes to appropriate functionality
- **utils.rs**: General utility functions used across the application
- **verify.rs**: Functions for verifying and inspecting WebAssembly files
- **watcher.rs**: Implements file watching for live reload functionality

### Server Module (`src/server/`)
The server module has been redesigned with a modular structure for better organization:

- **mod.rs**: Defines the public API for the server functionality, including:
  - `run_wasm_file`: Runs a WebAssembly file directly
  - `run_project`: Compiles and runs a project
  - `is_server_running`: Checks if a server is currently running
  - `stop_existing_server`: Stops an existing server
  - `run_webapp`: Runs a Rust web application

- **config.rs**: Server configuration and setup logic:
  - `ServerConfig`: Structure for server configuration
  - `run_server`: Core server implementation
  - `setup_project_compilation`: Sets up the project compilation environment

- **handler.rs**: HTTP request handling functions:
  - `handle_request`: Processes incoming HTTP requests
  - `handle_webapp_request`: Handles requests for web applications
  - `serve_file`: Serves file content
  - `serve_asset`: Serves static assets

- **wasm.rs**: WebAssembly-specific functionality:
  - `serve_wasm_file`: Serves a WebAssembly file
  - `serve_wasm_bindgen_files`: Serves wasm-bindgen files
  - `handle_wasm_bindgen_files`: Helper for wasm-bindgen projects

- **webapp.rs**: Web application support:
  - `run_webapp`: Runs a Rust web application
  - `run_webapp_server`: Manages the server for web applications
  - `run_webapp_server_with_watch`: Implements watch mode for web apps

- **utils.rs**: Server-specific utility functions:
  - `content_type_header`: Generates content-type headers
  - `print_server_info`: Displays server information
  - `find_wasm_files`: Locates WASM files in directories

### Compiler Module (`src/compiler/`)
- **detect.rs**: Project language detection logic
- **mod.rs**: Main compiler interface
- **language/**: Language-specific implementations
  - Each file implements compilation for a specific language (Rust, Go, C, etc.)

### Template Module (`src/template/`)
- **server/**: Templates for the WebAssembly server
- **webapp/**: Templates for Rust web applications

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

## Command Line Interface

Chakra's CLI is implemented in `cli.rs` using clap's derive features. Each subcommand follows the same pattern, supporting both positional and flag-based paths.

### Adding CLI Options

To add new CLI options:

1. Add the option to the `Args` struct or appropriate subcommand in `cli.rs`
2. Update `main.rs` to handle the new option
3. Ensure both positional and flag-based syntax is supported
4. Update documentation to reflect the changes

Example for handling a path argument in main.rs:

```rust
// Determine the actual path to use
let actual_path = positional_path.clone().unwrap_or_else(|| {
    path.clone().unwrap_or_else(|| String::from("./"))
});
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

### Watch Mode

The `watcher.rs` module provides file-watching functionality for live reloading:
- Monitors project files for changes
- Triggers recompilation when changes are detected
- Sends a reload signal to the browser

### Debugging Tips

When working with WASM loading:
- Use browser developer tools to check network requests
- Look for JavaScript errors in the browser console
- Test with simple WASM files first

## License

By contributing to Chakra, you agree that your contributions will be licensed under the project's [MIT license](./LICENSE).
