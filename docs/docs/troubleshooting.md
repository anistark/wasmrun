# Troubleshooting

Common issues and solutions when using Wasmrun.

## Plugin Issues

### Plugin Not Available

**Problem**: Error message "Plugin not available" when running a project.

**Solution**:

For built-in language support:
```bash
# C/C++ is built-in, no installation needed
wasmrun --language c ./my-project
```

For external plugins, install them first:
```bash
# Install the appropriate plugin
wasmrun plugin install wasmrust   # Rust plugin
wasmrun plugin install wasmgo     # Go plugin
wasmrun plugin install waspy      # Python plugin
wasmrun plugin install wasmasc    # AssemblyScript plugin

# Verify installation
wasmrun plugin list
```

### Plugin Dependencies Missing

**Problem**: Error about missing tools or dependencies.

**Solution**:

Check which dependencies are needed:
```bash
wasmrun plugin info <plugin-name>
```

Install missing tools:

**For Rust (wasmrust plugin)**:
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack  # Optional, for web apps
```

**For Go (wasmgo plugin)**:
```bash
# Install TinyGo from: https://tinygo.org/
# macOS:
brew install tinygo
# Linux: See https://tinygo.org/getting-started/install/
```

**For Python (waspy plugin)**:
```bash
# No dependencies! waspy is a pure Rust compiler
```

**For AssemblyScript (wasmasc plugin)**:
```bash
npm install -g assemblyscript
# or
yarn global add assemblyscript
# or
pnpm add -g assemblyscript
```

**For C/C++ (built-in)**:
```bash
# Install Emscripten from: https://emscripten.org/
```

### Wrong Plugin Selected

**Problem**: Wasmrun is using the wrong language plugin for your project.

**Solution**:

Force a specific plugin:
```bash
wasmrun --language rust ./project
wasmrun --language go ./project
wasmrun --language python ./project
wasmrun --language c ./project
wasmrun --language asc ./project
```

Or ensure your project has proper configuration files:
- **Rust**: `Cargo.toml`
- **Go**: `go.mod`
- **Python**: `.py` files
- **C/C++**: `Makefile` or `.c`/`.cpp` files
- **AssemblyScript**: `package.json` with `assemblyscript` dependency

### Plugin Installation Fails

**Problem**: `wasmrun plugin install` command fails.

**Solution**:

1. Check internet connection
2. Verify cargo is installed and updated:
```bash
rustup update
cargo --version
```

3. Check crates.io is accessible:
```bash
cargo search wasmrust
```

4. Try with verbose output:
```bash
wasmrun --debug plugin install wasmrust
```

5. Manually install via cargo:
```bash
cargo install wasmrust
# Then check if wasmrun detects it
wasmrun plugin list
```

## Installation Issues

### Cargo Installation Fails

**Problem**: `cargo install wasmrun` fails.

**Solution**:

1. Update Rust toolchain:
```bash
rustup update stable
```

2. Clear cargo cache and retry:
```bash
rm -rf ~/.cargo/registry/cache
cargo install wasmrun
```

3. Install from source:
```bash
git clone https://github.com/anistark/wasmrun.git
cd wasmrun
cargo install --path .
```

### DEB/RPM Package Installation Issues

**Problem**: Package installation fails on Linux.

**Solution**:

**For DEB packages**:
```bash
# Fix dependency issues
sudo apt install -f

# Install manually
sudo dpkg -i wasmrun_*.deb
sudo apt-get install -f
```

**For RPM packages**:
```bash
# Use dnf (Fedora/RHEL 8+)
sudo dnf install wasmrun-*.rpm

# Or rpm directly
sudo rpm -i wasmrun-*.rpm

# Fix dependencies
sudo dnf install -f
```

### Binary Not Found After Installation

**Problem**: `wasmrun: command not found` after installation.

**Solution**:

Add cargo bin directory to PATH:
```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell config
source ~/.bashrc  # or ~/.zshrc
```

Verify installation:
```bash
which wasmrun
wasmrun --version
```

## Server Issues

### Port Already in Use

**Problem**: Error "Address already in use" or port binding fails.

**Solution**:

Option 1: Stop existing wasmrun server
```bash
wasmrun stop
```

Option 2: Use a different port
```bash
wasmrun run ./project --port 3001
```

Option 3: Find and kill the process using the port
```bash
# macOS/Linux
lsof -ti:8420 | xargs kill -9

# Or find the PID manually
lsof -i:8420
kill -9 <PID>
```

### Server Won't Start

**Problem**: Development server fails to start.

**Solution**:

1. Check port availability:
```bash
lsof -i:8420  # Check default port
```

2. Try with debug mode:
```bash
wasmrun --debug run ./project
```

3. Check file permissions:
```bash
ls -la ./project
# Ensure you have read permissions
```

4. Verify project structure:
```bash
# Ensure entry files exist
ls -la ./project/src/
```

## Compilation Issues

### No Entry Point Found

**Problem**: Error "No entry point found" when running WASM file.

**Solution**:

1. Ensure your WASM has an entry function:
   - `main()` function
   - `_start()` function
   - Or use `-c/--call` to specify a function:
   ```bash
   wasmrun exec file.wasm -c my_function
   ```

2. Inspect your WASM file:
```bash
wasmrun inspect file.wasm
# Check for exported functions
```

3. Verify compilation output:
```bash
wasmrun compile ./project --verbose
```

### Compilation Fails

**Problem**: Project won't compile.

**Solution**:

1. Check dependencies:
```bash
wasmrun plugin info <language>
# Install any missing dependencies
```

2. Verify project structure:
```bash
# For Rust
cat Cargo.toml
# Check wasm32 target

# For Go
cat go.mod
# Verify TinyGo compatibility

# For C
cat Makefile
# Check Emscripten configuration
```

3. Try with verbose output:
```bash
wasmrun compile ./project --verbose
```

4. Use debug mode:
```bash
wasmrun --debug compile ./project
```

### Build Artifacts Not Found

**Problem**: Can't find compiled WASM files.

**Solution**:

Check standard output locations:

**For Rust**:
```bash
ls ./target/wasm32-unknown-unknown/release/*.wasm
ls ./target/wasm32-unknown-unknown/debug/*.wasm
```

**For Go (TinyGo)**:
```bash
ls ./*.wasm
ls ./build/*.wasm
```

**For C (Emscripten)**:
```bash
ls ./build/*.wasm
ls ./*.wasm
```

Or compile with specified output:
```bash
wasmrun compile ./project --output ./build/
```

## Native Execution Issues

### wasm-bindgen Module Detected

**Problem**: "wasm-bindgen module detected" error when using `wasmrun exec`.

**Explanation**: wasm-bindgen modules require JavaScript runtime features and are not supported in native execution mode.

**Solution**:

Option 1: Run the project directory (uses dev server):
```bash
wasmrun run ./my-rust-project
```

Option 2: Compile without wasm-bindgen:
```rust
// Cargo.toml - remove wasm-bindgen
[dependencies]
# wasm-bindgen = "0.2"  # Remove this

// Use wasm32-wasi target instead
```

```bash
cargo build --target wasm32-wasi
wasmrun exec target/wasm32-wasi/release/my_project.wasm
```

Option 3: Use Go/TinyGo for CLI tools:
```bash
tinygo build -o output.wasm -target wasi main.go
wasmrun exec output.wasm
```

### Native Execution Fails on Rust WASM

**Problem**: Rust WASM files fail with native execution.

**Explanation**: Some Rust functions require panic hooks, stack unwinding, or memory allocators that aren't fully supported in native mode.

**Solution**:

**Recommended approaches**:

1. Run the project directory (dev server):
```bash
wasmrun ./my-rust-project
```

2. Use wasm32-wasi target for CLI tools:
```bash
cargo build --target wasm32-wasi
wasmrun exec target/wasm32-wasi/release/project.wasm
```

3. Write pure functions without panic handling:
```rust
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b  // Simple, no panic possible
}
```

**Workarounds**:
- Avoid functions that can panic
- Use `#[no_mangle]` for exported functions
- Keep functions simple and pure
- Prefer Go/TinyGo for complex CLI tools

### Function Execution Fails

**Problem**: Specific function fails during native execution.

**Solution**:

1. Check function signature:
```bash
wasmrun inspect file.wasm
# Verify function is exported
```

2. Try with explicit function call:
```bash
wasmrun exec file.wasm -c function_name arg1 arg2
```

3. Check for language-specific issues:

**Rust**: May need panic hooks or runtime
```rust
// Use simpler, pure functions
#[no_mangle]
pub extern "C" fn simple_fn(x: i32) -> i32 {
    x * 2
}
```

**Go/TinyGo**: May need scheduler initialization
```bash
# Use -scheduler=none for simple functions
tinygo build -o out.wasm -scheduler=none main.go
```

## File Watching Issues

### Live Reload Not Working

**Problem**: Changes don't trigger recompilation with `--watch`.

**Solution**:

1. Ensure `--watch` flag is used:
```bash
wasmrun run ./project --watch
```

2. Check file permissions:
```bash
ls -la ./project/src/
```

3. Verify supported file types are being changed:
- `.rs` for Rust
- `.go` for Go
- `.py` for Python
- `.c`, `.cpp` for C/C++
- `.ts` for AssemblyScript

4. Try restarting the server:
```bash
wasmrun stop
wasmrun run ./project --watch
```

### Too Many Open Files

**Problem**: "Too many open files" error with file watching.

**Solution**:

Increase file descriptor limit:

**macOS**:
```bash
ulimit -n 4096
```

**Linux**:
```bash
ulimit -n 4096
# Or permanently in /etc/security/limits.conf
```

## Debugging Tips

### Enable Debug Mode

Get detailed information about what's happening:

```bash
# Any command with debug output
wasmrun --debug run ./project
wasmrun --debug compile ./project
wasmrun --debug plugin install wasmrust

# Save debug output to file
wasmrun --debug compile ./project 2> debug.log
```

### Get Plugin Information

```bash
# List all plugins
wasmrun plugin list

# Get detailed plugin info
wasmrun plugin info wasmrust
wasmrun plugin info wasmgo

# Check plugin installation location
ls -la ~/.wasmrun/plugins/
```

### Verify WASM File

```bash
# Basic verification
wasmrun verify file.wasm

# Detailed analysis
wasmrun verify file.wasm --detailed

# Inspect structure
wasmrun inspect file.wasm
```

### Check System Information

```bash
# Wasmrun version
wasmrun --version

# Rust version
rustc --version
cargo --version

# Check PATH
echo $PATH | tr ':' '\n'

# Check cargo bin directory
ls -la ~/.cargo/bin/
```

## Getting Help

If you're still experiencing issues:

1. **Check existing issues**: [GitHub Issues](https://github.com/anistark/wasmrun/issues)
2. **Enable debug mode**: Run with `--debug` flag
3. **Gather information**:
   - OS and version
   - Rust version (`rustc --version`)
   - Wasmrun version (`wasmrun --version`)
   - Plugin list (`wasmrun plugin list`)
   - Error messages (full output)
4. **Create an issue**: [New Issue](https://github.com/anistark/wasmrun/issues/new)

Include in your issue:
- Full error message
- Debug output (if applicable)
- Steps to reproduce
- System information
- Project structure (if relevant)
- Screenshots (if applicable)

## Common Error Messages

### "Plugin not available"
See [Plugin Not Available](#plugin-not-available)

### "Plugin dependencies missing"
See [Plugin Dependencies Missing](#plugin-dependencies-missing)

### "Port already in use"
See [Port Already in Use](#port-already-in-use)

### "No entry point found"
See [No Entry Point Found](#no-entry-point-found)

### "wasm-bindgen module detected"
See [wasm-bindgen Module Detected](#wasm-bindgen-module-detected)

### "Address already in use"
See [Port Already in Use](#port-already-in-use)

### "Too many open files"
See [Too Many Open Files](#too-many-open-files)

## Next Steps

- **[Installation](installation.md)**: Detailed installation instructions
- **[Quick Start](quick-start.md)**: Get started quickly
- **[CLI Reference](cli/)**: Command reference
- **[Debugging](development/debugging.md)**: Advanced debugging techniques
