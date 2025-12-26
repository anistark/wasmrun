# Debugging

Wasmrun includes a comprehensive debug mode that provides detailed logging to help with development and troubleshooting.

## Enabling Debug Mode

Enable debug logging with the `--debug` flag:

```bash
# Enable debug mode for any command
wasmrun --debug run ./project
wasmrun --debug plugin install wasmrust
wasmrun --debug exec file.wasm

# Or set environment variable
WASMRUN_DEBUG=1 wasmrun run ./project
```

## Debug Output Types

Debug mode provides several types of logging information:

| Type | Icon | Purpose | Example |
|------|------|---------|---------|
| **DEBUG** | 🔍 | General debug information | File operations, path resolution |
| **TRACE** | 🔬 | Very detailed tracing | Function internals, state changes |
| **ENTER** | 🚪 | Function entry points | Function called with parameters |
| **EXIT** | 🚶 | Function exit points | Function returned with values |
| **TIME** | ⏱️ | Performance timing | Operation duration |

### Example Debug Output

```
🚪 ENTER [main.rs:35] main - args = Args { command: Some(Run { ... }), debug: true }
🔍 DEBUG [main.rs:97] Processing run command: port=8420, language=None, watch=false
🚪 ENTER [server/mod.rs:77] run_project - path=./project, port=8420, language_override=None, watch=false
🔍 DEBUG [server/mod.rs:86] Checking path type: "./project"
🔍 DEBUG [detect.rs:28] detect_project_language - project_path=./project
🔍 DEBUG [detect.rs:39] Checking for language-specific configuration files
🔍 DEBUG [detect.rs:41] Found Cargo.toml - detected Rust project
🚶 EXIT  [detect.rs:43] detect_project_language -> Rust
⏱️ TIME  [server/mod.rs:134] Project compilation took 2.34s
🚶 EXIT  [main.rs:164] main - exit code: 0
```

## Debug Categories

Debug logs are organized by system component:

### CLI & Arguments
Logs command parsing and validation:

```bash
wasmrun --debug run ./project
```

Output:
```
🚪 ENTER [cli.rs:45] parse_args
🔍 DEBUG [cli.rs:67] Parsing command line arguments
🔍 DEBUG [cli.rs:89] Command: Run { path: "./project", port: None, watch: false }
🚶 EXIT  [cli.rs:112] parse_args
```

### Plugin System
Logs plugin loading, detection, and execution:

```bash
wasmrun --debug plugin list
```

Output:
```
🚪 ENTER [plugin/manager.rs:34] get_available_plugins
🔍 DEBUG [plugin/builtin.rs:12] Loading built-in plugins
🔍 DEBUG [plugin/builtin.rs:18] Registered C/C++ plugin
🔍 DEBUG [plugin/external.rs:45] Scanning ~/.wasmrun/plugins/
🔍 DEBUG [plugin/external.rs:78] Found external plugin: wasmrust
🔬 TRACE [plugin/external.rs:89] Loading libwasmrust.dylib
🚶 EXIT  [plugin/manager.rs:56] get_available_plugins -> 2 plugins
```

### Server Operations
Logs HTTP server startup and request handling:

```bash
wasmrun --debug run ./project --port 3000
```

Output:
```
🚪 ENTER [server/mod.rs:77] run_project
🔍 DEBUG [server/mod.rs:112] Starting server on port 3000
🔍 DEBUG [server/handler.rs:34] Handling request: GET /
🔍 DEBUG [server/wasm.rs:67] Serving WASM file: output.wasm
⏱️ TIME  [server/handler.rs:89] Request handled in 12ms
```

### Compilation
Logs build processes, tool detection, and file operations:

```bash
wasmrun --debug compile ./project
```

Output:
```
🚪 ENTER [commands/compile.rs:23] compile_project
🔍 DEBUG [compiler/detect.rs:41] Detected Rust project
🔍 DEBUG [plugin/manager.rs:123] Using wasmrust plugin
🔍 DEBUG [utils/command.rs:45] Executing: cargo build --target wasm32-unknown-unknown
🔬 TRACE [utils/command.rs:67] Command output: Compiling project v0.1.0
⏱️ TIME  [commands/compile.rs:89] Compilation took 3.42s
🚶 EXIT  [commands/compile.rs:95] compile_project -> Success
```

### File Operations
Logs path resolution and file validation:

```bash
wasmrun --debug verify ./file.wasm
```

Output:
```
🚪 ENTER [utils/path.rs:23] resolve_path
🔍 DEBUG [utils/path.rs:34] Resolving path: ./file.wasm
🔍 DEBUG [utils/path.rs:56] Absolute path: /Users/user/project/file.wasm
🚶 EXIT  [utils/path.rs:67] resolve_path
🔍 DEBUG [utils/wasm_analysis.rs:12] Reading WASM file
🔬 TRACE [utils/wasm_analysis.rs:34] File size: 1234 bytes
```

## Debug Macros

Wasmrun provides several macros for debug logging defined in `src/debug.rs`:

### debug_println!

General debug information:

```rust
use crate::debug_println;

debug_println!("Processing file: {}", file_path);
debug_println!("Found {} plugins", plugin_count);
```

Output:
```
🔍 DEBUG [my_file.rs:45] Processing file: ./project/main.rs
🔍 DEBUG [my_file.rs:67] Found 3 plugins
```

### trace_println!

Very detailed tracing for complex operations:

```rust
use crate::trace_println;

trace_println!("Internal state: {:?}", state);
trace_println!("Stack frame: {:?}", frame);
```

Output:
```
🔬 TRACE [executor.rs:123] Internal state: { pc: 0, stack: [1, 2, 3] }
🔬 TRACE [executor.rs:145] Stack frame: StackFrame { locals: [...] }
```

### debug_enter! and debug_exit!

Log function entry and exit points:

```rust
use crate::{debug_enter, debug_exit};

pub fn compile_project(path: &str) -> Result<()> {
    debug_enter!("compile_project", "path={}", path);

    // Function implementation

    debug_exit!("compile_project");
    Ok(())
}
```

Output:
```
🚪 ENTER [compile.rs:23] compile_project - path=./project
🚶 EXIT  [compile.rs:78] compile_project
```

With return values:

```rust
pub fn detect_language(path: &str) -> Result<String> {
    debug_enter!("detect_language", "path={}", path);

    let language = "rust".to_string();

    debug_exit!("detect_language", &language);
    Ok(language)
}
```

Output:
```
🚪 ENTER [detect.rs:34] detect_language - path=./project
🚶 EXIT  [detect.rs:56] detect_language -> "rust"
```

### debug_time!

Measure execution time of operations:

```rust
use crate::debug_time;

let result = debug_time!("project compilation", {
    compiler.build(&config)?
});

let plugins = debug_time!("plugin loading", {
    load_all_plugins()
});
```

Output:
```
⏱️ TIME  [compile.rs:45] project compilation took 2.34s
⏱️ TIME  [plugin.rs:67] plugin loading took 125ms
```

## Using Debug Logs for Development

### Debugging a Specific Operation

Redirect debug output to a file:

```bash
wasmrun --debug compile ./my-project 2> debug.log
```

### Debugging Plugin Detection

```bash
wasmrun --debug plugin list --all 2> plugin_debug.log
```

### Debugging Server Startup Issues

```bash
wasmrun --debug run ./project --port 3000 2> server_debug.log
```

## Adding Debug Logging to Your Code

When developing plugins or adding features:

```rust
use crate::{debug_enter, debug_exit, debug_println, debug_time, trace_println};

pub fn my_function(param: &str) -> Result<String> {
    debug_enter!("my_function", "param={}", param);

    // Add detailed debug info for complex operations
    debug_println!("Processing parameter: {}", param);

    // Use trace for very detailed logs
    trace_println!("Internal state: {:?}", internal_state);

    // Time expensive operations
    let result = debug_time!("expensive_operation", {
        expensive_computation(param)
    });

    debug_exit!("my_function", &result);
    Ok(result)
}
```

## Debug Best Practices

### 1. Function Boundaries

Use `debug_enter!` and `debug_exit!` for important functions:

```rust
pub fn important_function(args: &Args) -> Result<Output> {
    debug_enter!("important_function", "args={:?}", args);

    // Implementation

    debug_exit!("important_function");
    Ok(output)
}
```

### 2. Error Contexts

Add debug info before operations that might fail:

```rust
debug_println!("Attempting to load plugin: {}", plugin_name);
let plugin = load_plugin(&plugin_name)?;
debug_println!("Successfully loaded plugin: {}", plugin_name);
```

### 3. Performance Monitoring

Use `debug_time!` for potentially slow operations:

```rust
let compiled = debug_time!("WASM compilation", {
    builder.build(&config)?
});
```

### 4. State Changes

Log important state transitions:

```rust
debug_println!("Server state: Starting");
server.start()?;
debug_println!("Server state: Running on port {}", port);
```

### 5. File Operations

Debug file paths and validation results:

```rust
debug_println!("Resolving path: {}", input_path);
let absolute_path = resolve_path(input_path)?;
debug_println!("Absolute path: {}", absolute_path.display());
```

## Common Debugging Scenarios

### Plugin Not Loading

```bash
wasmrun --debug run ./project --language rust 2>&1 | grep -i plugin
```

Look for:
- Plugin detection logs
- Plugin loading errors
- FFI loading issues

### Compilation Failures

```bash
wasmrun --debug compile ./project 2>&1 | grep -i -A 5 "error\|fail"
```

Look for:
- Missing dependencies
- Invalid paths
- Tool execution failures

### Server Startup Problems

```bash
wasmrun --debug run ./project 2>&1 | grep -i -A 5 "server\|port"
```

Look for:
- Port binding issues
- Template loading errors
- File serving problems

### Performance Issues

```bash
wasmrun --debug run ./project 2>&1 | grep -i "TIME"
```

Look for:
- Slow compilation steps
- Long-running operations
- Inefficient file operations

## Debug Output Filtering

### Filter by Category

```bash
# Only server-related logs
wasmrun --debug run ./project 2>&1 | grep "server/"

# Only plugin-related logs
wasmrun --debug compile ./project 2>&1 | grep "plugin/"

# Only timing information
wasmrun --debug compile ./project 2>&1 | grep "TIME"
```

### Filter by Log Level

```bash
# Only ENTER/EXIT for call flow
wasmrun --debug run ./project 2>&1 | grep -E "ENTER|EXIT"

# Only DEBUG and ERROR
wasmrun --debug run ./project 2>&1 | grep -E "DEBUG|ERROR"

# Only performance timing
wasmrun --debug compile ./project 2>&1 | grep "TIME"
```

## Disabling Debug Mode

Debug mode is disabled by default. Only use it when troubleshooting.

```bash
# Normal operation (no debug output)
wasmrun run ./project
```

## Next Steps

- **[Architecture](architecture.md)**: Understand the system architecture
- **[Contributing](contributing.md)**: Contribute debug improvements
- **[Troubleshooting](../troubleshooting.md)**: Common issues and solutions
