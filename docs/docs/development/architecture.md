# Architecture

Wasmrun is designed with a modular architecture that separates concerns clearly and enables extensibility through a powerful plugin system.

## High-Level Overview

Wasmrun consists of several core modules that work together:

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI Layer                            │
│                    (cli.rs, main.rs)                        │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                     Command Layer                            │
│        (commands/: run, compile, plugin, exec, etc)         │
└──────────┬─────────────┬────────────────┬───────────────────┘
           │             │                │
           ▼             ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│    Server    │  │   Plugin     │  │   Runtime    │
│   Module     │  │   System     │  │   Module     │
└──────┬───────┘  └───────┬──────┘  └──────┬───────┘
       │                  │                 │
       ▼                  ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Templates  │  │   Compiler   │  │     WASI     │
│   & Assets   │  │   & Builder  │  │   Support    │
└──────────────┘  └──────────────┘  └──────────────┘
```

## Module Breakdown

### CLI Module (src/cli.rs, src/main.rs)

The entry point for the application:

- **cli.rs**: Defines command-line interface using [clap](https://github.com/clap-rs/clap)
- **main.rs**: Application entry point, routes commands to appropriate handlers
- Handles global flags (`--debug`, `--serve`, etc.)
- Provides user-friendly error messages

### Commands Module (src/commands/)

Each command is implemented in its own file:

| File | Command | Purpose |
|------|---------|---------|
| **run.rs** | `wasmrun run` | Development server with live reload |
| **compile.rs** | `wasmrun compile` | Project compilation with optimization |
| **exec.rs** | `wasmrun exec` | Native WASM execution |
| **plugin.rs** | `wasmrun plugin` | Plugin management (install, list, info) |
| **verify.rs** | `wasmrun verify` | WASM verification and validation |
| **clean.rs** | `wasmrun clean` | Build artifact cleanup |
| **stop.rs** | `wasmrun stop` | Server management |
| **os.rs** | `wasmrun os` | OS mode for multi-language execution |
| **init.rs** | `wasmrun init` | Project initialization |

### Plugin System (src/plugin/)

The core of Wasmrun's extensibility:

```
src/plugin/
├── mod.rs              # Plugin manager and core traits
├── builtin.rs          # Built-in plugin registry
├── external.rs         # External plugin loading (FFI)
├── installer.rs        # Plugin installation system
├── manager.rs          # Plugin lifecycle management
├── registry.rs         # Plugin discovery and registration
├── bridge.rs           # Plugin bridge functionality
├── metadata.rs         # Plugin metadata handling
└── languages/          # Built-in language plugins
    ├── mod.rs
    └── c_plugin.rs     # C/C++ plugin with Emscripten
```

#### Plugin Architecture

Every plugin implements two core traits:

```rust
pub trait Plugin {
    fn info(&self) -> &PluginInfo;
    fn can_handle_project(&self, path: &str) -> bool;
    fn get_builder(&self) -> Box<dyn WasmBuilder>;
}

pub trait WasmBuilder {
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;
    fn check_dependencies(&self) -> Vec<String>;
    fn validate_project(&self, path: &str) -> CompilationResult<()>;
    fn clean(&self, path: &str) -> Result<()>;
    fn supported_extensions(&self) -> &[&str];
    fn entry_file_candidates(&self) -> &[&str];
}
```

#### Plugin Types

1. **Built-in Plugins** (compiled into Wasmrun)
   - C/C++ plugin (Emscripten)
   - Loaded at startup
   - No installation required

2. **External Plugins** (dynamically loaded via FFI)
   - Rust plugin (wasmrust)
   - Go plugin (wasmgo)
   - Python plugin (waspy)
   - AssemblyScript plugin (wasmasc)
   - Installed to `~/.wasmrun/plugins/`
   - Loaded as shared libraries (`.dylib`, `.so`, `.dll`)

### Server Module (src/server/)

HTTP server for development and web apps:

```
src/server/
├── mod.rs          # Server initialization and configuration
├── runner.rs       # Server startup and management
├── handler.rs      # HTTP request routing and handling
├── wasm.rs         # WASM file serving
├── api.rs          # API endpoints
├── lifecycle.rs    # Server lifecycle management
└── utils.rs        # Server utilities
```

Key features:
- Serves WASM files and web apps
- Live reload functionality
- WebSocket connections for OS mode
- Static file serving
- Template rendering

### Runtime Module (src/runtime/)

Native WASM execution engine:

```
src/runtime/
├── mod.rs                  # Runtime coordination
├── core/                   # WASM interpreter core
│   ├── executor.rs         # Instruction execution
│   ├── module.rs           # Module loading and parsing
│   ├── memory.rs           # Linear memory management
│   ├── control_flow.rs     # Control flow operations
│   ├── values.rs           # Value stack operations
│   └── linker.rs           # Module linking
├── wasi/                   # WASI implementation
│   ├── mod.rs
│   └── syscalls.rs         # WASI syscall implementation
├── languages/              # OS mode language runtimes
│   ├── nodejs.rs
│   ├── python.rs
│   └── go.rs
├── os_server.rs            # OS mode server
├── microkernel.rs          # Microkernel implementation
└── syscalls.rs             # System call interface
```

### Compiler Module (src/compiler/)

Legacy compilation system (being phased out in favor of plugins):

```
src/compiler/
├── mod.rs          # Compiler module exports
├── builder.rs      # Build configuration and result types
└── detect.rs       # Project type detection utilities
```

The compiler module is gradually being replaced by the plugin system.

### Utilities Module (src/utils/)

Shared utilities used across the codebase:

```
src/utils/
├── mod.rs              # Utility module exports
├── command.rs          # Command execution utilities
├── path.rs             # Path resolution and validation
├── plugin_utils.rs     # Plugin-specific utilities
├── system.rs           # System information and detection
└── wasm_analysis.rs    # WebAssembly file analysis
```

### Template System (src/template.rs)

Manages HTML templates and web UI assets:

- Templates stored in root `templates/` directory
- Embedded into binary during build
- Supports web apps and development server UI
- Includes Preact-based UI components

### Error Handling (src/error.rs)

Centralized error handling with user-friendly messages:

```rust
pub enum WasmrunError {
    CompilationError(CompilationError),
    ServerError(ServerError),
    PluginError(PluginError),
    IoError(std::io::Error),
    // ...
}
```

All errors implement helpful error messages and suggestions for users.

### Debug System (src/debug.rs)

Comprehensive debug logging with macros:

```rust
debug_println!("Debug message");      // 🔍 DEBUG
trace_println!("Trace message");       // 🔬 TRACE
debug_enter!("function_name");         // 🚪 ENTER
debug_exit!("function_name");          // 🚶 EXIT
debug_time!("operation", { ... });     // ⏱️ TIME
```

See [Debugging](debugging.md) for details.

## Request Flow

### Development Server Flow

```
User runs: wasmrun run ./project --watch

1. CLI parses arguments (cli.rs)
   ↓
2. Main routes to run command (main.rs)
   ↓
3. Run command starts (commands/run.rs)
   ↓
4. Plugin manager detects project type (plugin/manager.rs)
   - Checks file extensions
   - Checks configuration files
   - Matches to appropriate plugin
   ↓
5. Plugin compiles project (plugin/languages/*.rs)
   - Validates dependencies
   - Executes build command
   - Produces WASM artifacts
   ↓
6. Server starts (server/mod.rs)
   - Loads templates
   - Sets up HTTP routes
   - Configures file watcher (if --watch)
   ↓
7. Browser connects
   - Server serves HTML template
   - Loads WASM file
   - Executes in browser
   ↓
8. File watcher detects changes (watcher.rs)
   - Triggers recompilation
   - Sends reload signal
   - Updates browser
```

### Native Execution Flow

```
User runs: wasmrun exec file.wasm -c function arg1 arg2

1. CLI parses arguments
   ↓
2. Exec command starts (commands/exec.rs)
   ↓
3. WASM module loaded (runtime/core/module.rs)
   - Parses WASM binary
   - Validates structure
   - Extracts exports
   ↓
4. Runtime initializes (runtime/core/executor.rs)
   - Sets up linear memory
   - Initializes value stack
   - Prepares WASI syscalls
   ↓
5. Function executed (runtime/core/executor.rs)
   - Executes instructions
   - Handles WASI syscalls
   - Manages memory operations
   ↓
6. Results returned
   - Output to stdout
   - Return values printed
   - Exit code set
```

### Plugin Installation Flow

```
User runs: wasmrun plugin install wasmrust

1. Plugin command starts (commands/plugin.rs)
   ↓
2. Plugin installer invoked (plugin/installer.rs)
   ↓
3. Download from crates.io
   - Uses cargo install
   - Builds plugin locally
   ↓
4. Install to ~/.wasmrun/
   - Creates plugin directory
   - Copies compiled library
   - Extracts metadata
   ↓
5. Register plugin (plugin/registry.rs)
   - Updates config.toml
   - Registers capabilities
   - Makes plugin available
   ↓
6. Plugin ready for use
   - Auto-detects supported projects
   - Integrates with compiler
```

## Configuration Management

Wasmrun uses a layered configuration approach:

```
src/config/
├── mod.rs          # Configuration coordination
├── constants.rs    # Global constants
├── plugin.rs       # Plugin configuration
└── server.rs       # Server configuration
```

Configuration locations:
- Global: `~/.wasmrun/config.toml`
- Per-plugin: `~/.wasmrun/plugins/{plugin}/wasmrun.toml`
- Project-specific: Detected from project files

## Logging System

```
src/logging/
├── mod.rs          # Logging coordination
├── log_entry.rs    # Log entry types
└── system.rs       # System logging
```

Used primarily for OS mode to provide real-time logs in the browser UI.

## File Watching (src/watcher.rs)

Uses the [notify](https://github.com/notify-rs/notify) crate to watch for file changes and trigger recompilation.

## UI Components

The UI is a separate Preact/TypeScript application:

```
ui/
├── src/
│   ├── components/     # React components
│   ├── hooks/          # Custom hooks
│   └── app.tsx         # Main app
├── package.json
└── tsconfig.json
```

Built with:
- [Preact](https://preactjs.com/)
- [TypeScript](https://www.typescriptlang.org/)
- [Tailwind CSS](https://tailwindcss.com/)

The UI is built and embedded into the Rust binary during compilation.

## Build Process

Wasmrun uses a multi-stage build:

1. **UI Build**: TypeScript/Preact compiled to optimized bundle
2. **Template Embedding**: HTML templates embedded via `include_str!`
3. **Rust Compilation**: Main binary compiled with embedded assets
4. **Plugin Compilation**: External plugins built separately

## Testing Architecture

```
tests/
├── integration/        # Integration tests
├── fixtures/           # Test data and projects
└── ...
```

Testing strategy:
- Unit tests co-located with modules
- Integration tests in `tests/` directory
- Example projects serve as end-to-end tests
- CI runs format, lint, tests, and example builds

## Security Considerations

- **Plugin sandboxing**: External plugins run in isolated processes
- **WASI permissions**: Limited filesystem and network access
- **Network isolation**: Per-process network namespaces (OS mode)
- **Input validation**: All user input validated and sanitized
- **Error messages**: No sensitive information leaked in errors

## Performance Optimizations

- **Lazy plugin loading**: Plugins loaded only when needed
- **Binary embedding**: Templates and UI embedded for fast startup
- **Incremental compilation**: Watch mode only recompiles changed files
- **Caching**: Build artifacts cached in `~/.wasmrun/cache/`

## Cross-Platform Support

Wasmrun supports:
- **macOS** (`.dylib` plugins)
- **Linux** (`.so` plugins)
- **Windows** (`.dll` plugins)

Platform-specific code is isolated in utility modules and uses conditional compilation.

## Next Steps

- **[Creating Plugins](creating-plugins.md)**: Learn to develop your own plugins
- **[Contributing](contributing.md)**: Contribute to Wasmrun development
- **[Debugging](debugging.md)**: Debug and troubleshoot Wasmrun
