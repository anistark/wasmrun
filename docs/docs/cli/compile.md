# compile

Compile a WebAssembly project with optimization options.

## Synopsis

```bash
wasmrun compile [PROJECT] [OPTIONS]
```

**Aliases:** `build`, `c`

## Description

The `compile` command builds your project to WebAssembly without starting a development server. This is ideal for:

- Production builds
- CI/CD pipelines
- Pre-compiling for distribution
- Generating optimized WASM binaries

The command detects your project language and uses the appropriate compiler toolchain.

## Options

### `-p, --path <PATH>`

Path to the project directory.

```bash
wasmrun compile --path ./my-project
wasmrun compile -p ./my-project
```

Default: Current directory (`.`)

:::tip
You can also use a positional argument: `wasmrun compile ./my-project`
:::

### `-o, --output <DIRECTORY>`

Output directory for compiled files.

```bash
wasmrun compile --output ./dist
wasmrun compile -o ./build
```

Default: Current directory (`.`)

### `--optimization <LEVEL>`

Compilation optimization level.

```bash
wasmrun compile --optimization release
wasmrun compile --optimization size
```

Available levels:

| Level | Description | Use Case |
|-------|-------------|----------|
| `debug` | No optimization, debug symbols | Development and debugging |
| `release` | Optimized for performance | Production (default) |
| `size` | Optimized for binary size | Bandwidth-constrained environments |

Default: `release`

### `-v, --verbose`

Show detailed compilation output.

```bash
wasmrun compile --verbose
wasmrun compile -v
```

## Examples

### Basic Compilation

Compile current project:

```bash
wasmrun compile
```

### Specific Project

Compile a specific project:

```bash
wasmrun compile ./my-rust-project
```

### With Output Directory

Save to specific directory:

```bash
wasmrun compile ./my-project --output ./dist
```

### Size-Optimized Build

Minimize file size:

```bash
wasmrun compile --optimization size
```

### Debug Build

Build with debug symbols:

```bash
wasmrun compile --optimization debug --verbose
```

### Complete Production Build

```bash
wasmrun compile ./my-project \
  --output ./dist \
  --optimization release \
  --verbose
```

## Optimization Levels Explained

### Debug Mode

```bash
wasmrun compile --optimization debug
```

- No optimizations applied
- Includes debug symbols
- Faster compilation
- Larger file size
- Easier debugging

### Release Mode (Default)

```bash
wasmrun compile --optimization release
```

- Full optimizations enabled
- Balanced size/performance
- Recommended for production
- Longer compilation time

### Size Mode

```bash
wasmrun compile --optimization size
```

- Aggressive size reduction
- May sacrifice some performance
- Ideal for web delivery
- Removes unnecessary code
- Applies compression hints

## Output Files

After successful compilation, you'll find:

```
dist/
├── module.wasm          # Compiled WebAssembly binary
├── module.js            # JavaScript glue code (if applicable)
└── module_bg.wasm       # Background WASM (for wasm-bindgen projects)
```

## Language-Specific Behavior

### Rust Projects

Compiles using Cargo with appropriate target:

```bash
# Standard compilation
wasmrun compile

# With wasm-bindgen
wasmrun plugin install wasmrust
wasmrun compile
```

Generated files:
- `*.wasm` - WASM binary
- `*.js` - JS bindings (wasm-bindgen)
- `*.d.ts` - TypeScript definitions (wasm-bindgen)

### Go Projects

Uses TinyGo compiler:

```bash
wasmrun plugin install wasmgo
wasmrun compile
```

### Python Projects

Compiles Python to WASM using waspy:

```bash
wasmrun plugin install waspy
wasmrun compile
```

### C/C++ Projects

Uses Emscripten:

```bash
wasmrun compile
```

### AssemblyScript Projects

Uses AssemblyScript compiler:

```bash
wasmrun plugin install wasmasc
wasmrun compile
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Build WASM

on: [push]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install Wasmrun
        run: cargo install wasmrun

      - name: Compile Project
        run: wasmrun compile --optimization release --output ./dist

      - name: Upload Artifacts
        uses: actions/upload-artifact@v2
        with:
          name: wasm-build
          path: ./dist/
```

### GitLab CI

```yaml
build:
  script:
    - cargo install wasmrun
    - wasmrun compile --optimization release --output ./dist
  artifacts:
    paths:
      - dist/
```

## Compilation Stages

When you run compile, Wasmrun:

1. **Detects Language** - Analyzes project structure
2. **Validates Dependencies** - Checks required tools
3. **Runs Compiler** - Invokes language-specific toolchain
4. **Applies Optimizations** - Based on optimization level
5. **Generates Output** - Places files in output directory

Progress output:

```
🔍 Detecting project language...
   ✓ Detected: Rust

📦 Compiling project...
   → Running: cargo build --target wasm32-unknown-unknown --release

🎯 Optimizing...
   → Optimization level: release

✅ Compilation successful!
   → Output: ./dist/module.wasm (245 KB)
```

## Performance Tips

### Faster Builds

For development iterations:

```bash
wasmrun compile --optimization debug
```

### Smaller Bundles

For web deployment:

```bash
wasmrun compile --optimization size
```

Then compress further:

```bash
gzip -9 dist/module.wasm
brotli -9 dist/module.wasm
```

### Parallel Builds

Leverage cargo's parallelism (Rust):

```bash
CARGO_BUILD_JOBS=8 wasmrun compile
```

## Troubleshooting

### Compilation Fails

Enable verbose mode to see detailed errors:

```bash
wasmrun compile --verbose
```

### Plugin Not Found

Install the required language plugin:

```bash
wasmrun plugin install <plugin-name>
```

### Missing Dependencies

Ensure language toolchain is installed:

```bash
# Rust
rustup target add wasm32-unknown-unknown

# Go
brew install tinygo

# C/C++
brew install emscripten

# AssemblyScript
npm install -g assemblyscript
```

### Output Permission Denied

Check output directory permissions:

```bash
chmod 755 ./dist
wasmrun compile --output ./dist
```

### Large File Size

Try size optimization:

```bash
wasmrun compile --optimization size
```

For Rust, also add to `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

## Verifying Output

After compilation, verify the WASM file:

```bash
wasmrun verify ./dist/module.wasm --detailed
```

Inspect the compiled module:

```bash
wasmrun inspect ./dist/module.wasm
```

## See Also

- [run](./run.md) - Development server with auto-compilation
- [verify](./verify.md) - Verify WASM files
- [inspect](./inspect.md) - Inspect WASM structure
- [exec](./exec.md) - Execute compiled WASM
- [plugin](./plugin.md) - Manage language plugins
