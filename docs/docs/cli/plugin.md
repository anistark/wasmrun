# plugin

Manage Wasmrun language plugins.

## Synopsis

```bash
wasmrun plugin <SUBCOMMAND> [OPTIONS]
```

## Description

The `plugin` command provides a complete plugin management system for Wasmrun. Plugins extend Wasmrun's capabilities to support additional programming languages and features.

## Subcommands

### `list`

List all available plugins.

```bash
wasmrun plugin list
```

Show detailed information:

```bash
wasmrun plugin list --all
wasmrun plugin list -a
```

Example output:

```
📦 Installed Plugins:
  ✓ wasmrust (v0.5.0) - Rust WebAssembly plugin
  ✓ wasmgo (v0.3.2) - Go/TinyGo WebAssembly plugin

📋 Available Plugins:
  • waspy - Python to WebAssembly compiler
  • wasmasc - AssemblyScript compiler plugin
```

### `install`

Install a plugin from crates.io, URL, or local path.

```bash
wasmrun plugin install <PLUGIN>
```

Options:
- `-v, --version <VERSION>` - Specific version to install

Examples:

```bash
# From crates.io (recommended)
wasmrun plugin install wasmrust

# Specific version
wasmrun plugin install wasmrust --version 0.5.0

# From URL
wasmrun plugin install https://github.com/user/plugin

# From local path
wasmrun plugin install ./path/to/plugin
```

### `uninstall`

Remove an installed plugin.

```bash
wasmrun plugin uninstall <PLUGIN>
```

Example:

```bash
wasmrun plugin uninstall wasmrust
```

### `update`

Update a plugin to the latest version.

```bash
wasmrun plugin update <PLUGIN>
```

Update all plugins:

```bash
wasmrun plugin update all
```

Examples:

```bash
# Update specific plugin
wasmrun plugin update wasmrust

# Update all plugins
wasmrun plugin update all
```

### `enable`

Enable or disable a plugin.

```bash
wasmrun plugin enable <PLUGIN>
wasmrun plugin enable <PLUGIN> --disable
```

Examples:

```bash
# Enable plugin
wasmrun plugin enable wasmrust

# Disable plugin
wasmrun plugin enable wasmrust --disable
```

### `info`

Show detailed information about a plugin.

```bash
wasmrun plugin info <PLUGIN>
```

Example:

```bash
wasmrun plugin info wasmrust
```

Output:

```
Plugin: wasmrust
Version: 0.5.0
Type: External
Language: Rust
Status: Enabled

Description:
  Rust WebAssembly plugin with wasm-bindgen support

Capabilities:
  ✓ Compilation
  ✓ Live reload
  ✓ WASM bindgen
  ✓ TypeScript definitions

Installation: ~/.wasmrun/plugins/wasmrust
```

## Built-in vs External Plugins

### Built-in Plugins

Built-in plugins come with Wasmrun:

- **C/C++** - Emscripten support (built-in)

### External Plugins

External plugins must be installed separately:

| Plugin | Language | Description |
|--------|----------|-------------|
| `wasmrust` | Rust | wasm-bindgen and wasm-pack support |
| `wasmgo` | Go | TinyGo compiler integration |
| `waspy` | Python | Python to WebAssembly compiler |
| `wasmasc` | AssemblyScript | TypeScript-like syntax for WASM |

## Plugin Installation Location

Plugins are installed in:

```
~/.wasmrun/
├── plugins/
│   ├── wasmrust/
│   ├── wasmgo/
│   └── waspy/
└── bin/
    └── plugin binaries
```

## Common Plugins

### wasmrust - Rust Plugin

Provides Rust WebAssembly compilation with wasm-bindgen.

```bash
# Install
wasmrun plugin install wasmrust

# Use
wasmrun run ./my-rust-project
```

Features:
- wasm-bindgen support
- wasm-pack integration
- TypeScript definitions
- Web API bindings

### wasmgo - Go Plugin

TinyGo compiler for Go WebAssembly projects.

```bash
# Install
wasmrun plugin install wasmgo

# Use
wasmrun run ./my-go-project
```

Features:
- TinyGo compilation
- Small binary sizes
- WASI support

### waspy - Python Plugin

Compile Python code to WebAssembly.

```bash
# Install
wasmrun plugin install waspy

# Use
wasmrun run ./my-python-project
```

Features:
- Python to WASM compilation
- Type annotations support
- Subset of Python standard library

### wasmasc - AssemblyScript Plugin

AssemblyScript (TypeScript-like) compiler.

```bash
# Install
wasmrun plugin install wasmasc

# Use
wasmrun run ./my-asc-project
```

Features:
- TypeScript-like syntax
- npm/yarn/pnpm/bun support
- Small output size

## Plugin Workflow

### Initial Setup

```bash
# List available plugins
wasmrun plugin list

# Install needed plugins
wasmrun plugin install wasmrust
wasmrun plugin install wasmgo

# Verify installation
wasmrun plugin info wasmrust
```

### Updating Plugins

```bash
# Check for updates
wasmrun plugin list --all

# Update specific plugin
wasmrun plugin update wasmrust

# Update all plugins
wasmrun plugin update all
```

### Managing Plugins

```bash
# Temporarily disable a plugin
wasmrun plugin enable wasmrust --disable

# Re-enable plugin
wasmrun plugin enable wasmrust

# Remove plugin
wasmrun plugin uninstall wasmgo
```

## Plugin Requirements

### wasmrust

Requires:
- Rust toolchain
- `wasm32-unknown-unknown` target
- wasm-bindgen-cli (optional)

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

### wasmgo

Requires:
- TinyGo

```bash
# macOS
brew install tinygo

# Linux
wget https://github.com/tinygo-org/tinygo/releases/download/v0.30.0/tinygo_0.30.0_amd64.deb
sudo dpkg -i tinygo_0.30.0_amd64.deb
```

### waspy

Requires:
- Python 3.8+

```bash
python3 --version
```

### wasmasc

Requires:
- Node.js
- npm/yarn/pnpm/bun

```bash
node --version
npm --version
```

## Troubleshooting

### Plugin Not Found After Install

Check installation:

```bash
wasmrun plugin list --all
ls ~/.wasmrun/plugins/
```

### Plugin Installation Fails

Enable verbose cargo output:

```bash
CARGO_LOG=trace wasmrun plugin install wasmrust
```

### Plugin Dependencies Missing

Each plugin may have dependencies. Check with:

```bash
wasmrun plugin info <plugin-name>
```

Then install required tools.

### Plugin Won't Enable

Check plugin status:

```bash
wasmrun plugin info <plugin-name>
```

Try reinstalling:

```bash
wasmrun plugin uninstall <plugin-name>
wasmrun plugin install <plugin-name>
```

### Cannot Uninstall Plugin

Ensure no projects are using it:

```bash
wasmrun stop
wasmrun plugin uninstall <plugin-name>
```

## Creating Custom Plugins

See the [Creating Plugins Guide](/docs/development/creating-plugins) for detailed instructions on building your own plugins.

## See Also

- [Plugin System Feature](/docs/features/plugin-system) - Plugin architecture
- [Creating Plugins](/docs/development/creating-plugins) - Build custom plugins
- [Rust Guide](/docs/guides/rust) - Using wasmrust
- [Go Guide](/docs/guides/go) - Using wasmgo
- [Python Guide](/docs/guides/python) - Using waspy
- [AssemblyScript Guide](/docs/guides/assemblyscript) - Using wasmasc
