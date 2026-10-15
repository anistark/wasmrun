---
sidebar_position: 2
title: Architecture
---

# Architecture

Wasmrun is one binary with four modes and a process manager, which share an interpreter, a WASI layer and some utilities but not much else:

| Mode | Command | Runs code with | Code |
|---|---|---|---|
| Server | `wasmrun`, `wasmrun run` | The browser, after compiling with a plugin | `src/server/`, `src/compiler/`, `src/plugin/` |
| Exec | `wasmrun exec` | The interpreter | `src/runtime/core/`, `src/runtime/wasi/` |
| OS | `wasmrun os` | A browser worker, on a wasmhub runtime | `src/runtime/os_server.rs` and the OS kernel, `ui/src/os/` |
| Agent | `wasmrun agent` | The interpreter, per session | `src/agent/` |
| Process manager | `wasmrun ps` | `wasmrun exec` child processes | `src/ps/` |

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
| **run.rs** | `wasmrun`, `wasmrun run` | Development server with live reload |
| **compile.rs** | `wasmrun compile` | Project compilation with optimization |
| **exec.rs** | `wasmrun exec` | Native WASM execution |
| **agent.rs** | `wasmrun agent` | The REST sandbox API for AI agents |
| **os.rs** | `wasmrun os` | OS mode: projects in a browser VM |
| **ps.rs** | `wasmrun ps` | The process manager's client commands |
| **init.rs** | `wasmrun init` | A new project from a template (the repository's examples, embedded) |
| **plugin.rs** | `wasmrun plugin` | Plugin management (install, list, info) |
| **verify.rs** | `wasmrun verify`, `wasmrun inspect` | WASM verification and inspection |
| **clean.rs** | `wasmrun clean` | Build artifact cleanup |
| **stop.rs** | `wasmrun stop` | Stop every running dev session |

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
├── dev.rs          # The dev session: shared state, both ports, the --watch loop
├── app.rs          # App port: a web app's page, files and build output
├── handler.rs      # UI port: module console or control center, JSON APIs
├── api.rs          # Module info, version, embedded assets
├── lifecycle.rs    # Per-session registry that `wasmrun stop` reads
└── utils.rs        # Ports, content types
```

Key features:
- A module goes to the console on the UI port; a web app runs from its own page on the app port, framed by a control center on the UI port
- With `--watch`, a source change rebuilds and the page reloads itself
- Both ports bind `127.0.0.1`, and the app port refuses dotfiles and any path outside the project or build directory
- Each session records itself in the system temp directory, so `wasmrun stop` can stop every running one
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
│   ├── fuzz.rs             # Mutation fuzzing of the parser, executor and memory (tests)
│   └── spec_suite.rs       # The official spec test suite harness (tests)
├── wasi/                   # WASI implementation (exec and agent mode)
│   ├── mod.rs              # WasiEnv: fds, preopens, args, env, sockets
│   ├── network.rs          # Egress policy for outbound sockets
│   └── syscalls.rs         # WASI syscall implementation
├── runtime_cache.rs        # wasmhub runtime downloads (OS and agent mode)
├── network.rs              # OS mode's wasmnet proxy
├── os_server.rs            # OS mode server
├── multilang_kernel.rs     # OS kernel: language detection, staging, process records
├── microkernel.rs          # OS kernel: the process table and the workspace VFS
├── registry.rs             # OS kernel: the languages it can stage
├── languages/              # OS kernel: Node.js detection and staging
├── wasi_fs.rs              # OS kernel: the virtual filesystem behind /api/fs
├── project_files.rs        # The project bundle the browser VM is given
└── tunnel/                 # `wasmrun os --expose`: the bore client
```

The program in OS mode runs in a browser worker, not on the host: the OS kernel records it and stages its files, and nothing in it schedules or executes code.

### Agent Module (src/agent/)

The REST sandbox API: sessions, auth and tenants, limits, metrics, npm vendoring, ES module lowering, the shell emulation and long-lived servers. Every execution runs on exec mode's interpreter and WASI; agent mode owns only what exec mode has no concept of. See [Agent Mode](../agent/index.md).

### Process Manager (src/ps/)

```
src/ps/
├── mod.rs          # run_supervisor: the supervisor process
├── config.rs       # processes.toml, checked up front
├── supervisor.rs   # One monitor thread per program: start, watch, restart with backoff
├── control.rs      # The loopback control API and supervisor.json
└── logs.rs         # Per-program logs, rotated by size
```

Each program runs as its own `wasmrun exec -q` process, so a crash takes down only that program. See [Process Manager](../ps/index.md).

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
├── http.rs             # The outbound HTTP agent (wasmhub, npm)
├── path.rs             # Path resolution and validation
├── plugin_utils.rs     # Plugin-specific utilities
├── process.rs          # Other processes: is it wasmrun, its memory, SIGTERM
└── system.rs           # System information and detection
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
User runs: wasmrun ./project --watch

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
   - wasmrun exits with the program's exit code
```

### Plugin Installation Flow

```
User runs: wasmrun plugin install wasmrust

1. Plugin command starts (commands/plugin.rs)
   ↓
2. Plugin installer invoked (plugin/installer.rs)
   - Refuses a name crates.io does not have
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
5. Register plugin (plugin/manager.rs)
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

- **[Creating Plugins](../plugins/creating-plugins.md)**: Learn to develop your own plugins
- **[Contributing](./how-to-contribute.md)**: Contribute to Wasmrun development
- **[Debugging](debugging.md)**: Debug and troubleshoot Wasmrun
