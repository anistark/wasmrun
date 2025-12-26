# Creating Plugins

Wasmrun implements a powerful plugin architecture that enables seamless integration of different programming languages and compilation toolchains.

## Plugin Architecture Overview

### How Plugins Work

Every plugin in Wasmrun implements the core `Plugin` trait, providing a consistent interface for language support:

```rust
pub trait Plugin {
    fn info(&self) -> &PluginInfo;                    // Plugin metadata
    fn can_handle_project(&self, path: &str) -> bool; // Project compatibility
    fn get_builder(&self) -> Box<dyn WasmBuilder>;    // Compilation engine
}
```

The `WasmBuilder` trait defines the compilation interface:

```rust
pub trait WasmBuilder {
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult>;
    fn check_dependencies(&self) -> Vec<String>;      // Missing tools
    fn validate_project(&self, path: &str) -> CompilationResult<()>;
    fn clean(&self, path: &str) -> Result<()>;        // Cleanup artifacts
    fn supported_extensions(&self) -> &[&str];        // File extensions
    fn entry_file_candidates(&self) -> &[&str];       // Entry files
}
```

### Plugin Capabilities System

Each plugin declares its capabilities through structured metadata:

```rust
pub struct PluginCapabilities {
    pub compile_wasm: bool,          // Standard .wasm file generation
    pub compile_webapp: bool,        // Web application bundling
    pub live_reload: bool,           // Development server support
    pub optimization: bool,          // Size/speed optimization passes
    pub custom_targets: Vec<String>, // Additional compilation targets
}

pub struct PluginInfo {
    pub name: String,                     // Plugin identifier
    pub version: String,                  // Plugin version
    pub description: String,              // Human-readable description
    pub author: String,                   // Plugin author
    pub extensions: Vec<String>,          // Supported file extensions
    pub entry_files: Vec<String>,         // Project entry points
    pub plugin_type: PluginType,          // Built-in vs External
    pub dependencies: Vec<String>,        // Required system tools
    pub capabilities: PluginCapabilities,
}
```

## Creating a Built-in Plugin

Built-in plugins are compiled directly into Wasmrun. They provide core language support without requiring installation.

### Step 1: Create Plugin Structure

Create a new file in `src/plugin/languages/` directory:

```rust
// src/plugin/languages/my_language_plugin.rs
use crate::plugin::{Plugin, PluginInfo, PluginCapabilities, PluginType};
use crate::compiler::builder::WasmBuilder;

pub struct MyLanguagePlugin {
    info: PluginInfo,
}

impl MyLanguagePlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "mylang".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "My Language WebAssembly compiler".to_string(),
            author: "Your Name".to_string(),
            extensions: vec!["ml".to_string(), "myl".to_string()],
            entry_files: vec!["main.ml".to_string(), "package.myl".to_string()],
            plugin_type: PluginType::Builtin,
            source: None,
            dependencies: vec!["mylang-compiler".to_string()],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: true,
                optimization: true,
                custom_targets: vec!["wasm32-wasi".to_string()],
            },
        };
        Self { info }
    }
}
```

### Step 2: Implement Plugin Trait

```rust
impl Plugin for MyLanguagePlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, path: &str) -> bool {
        // Check for language-specific files or configurations
        std::path::Path::new(path).join("mylang.config").exists() ||
        std::fs::read_dir(path).ok().map_or(false, |mut entries| {
            entries.any(|e| e.ok().map_or(false, |entry| {
                entry.path().extension()
                    .and_then(|ext| ext.to_str())
                    .map_or(false, |ext| ext == "ml" || ext == "myl")
            }))
        })
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(MyLanguageBuilder::new())
    }
}
```

### Step 3: Implement WasmBuilder Trait

```rust
use crate::compiler::builder::{WasmBuilder, BuildConfig, BuildResult, CompilationResult, CompilationError};
use crate::utils::command::CommandExecutor;

pub struct MyLanguageBuilder {
    language_name: String,
}

impl MyLanguageBuilder {
    pub fn new() -> Self {
        Self {
            language_name: "mylang".to_string(),
        }
    }

    fn language_name(&self) -> &str {
        &self.language_name
    }
}

impl WasmBuilder for MyLanguageBuilder {
    fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
        let executor = CommandExecutor::new(&config.project_path);

        // Check dependencies
        let missing_deps = self.check_dependencies();
        if !missing_deps.is_empty() {
            return Err(CompilationError::DependencyError {
                tool: missing_deps.join(", "),
                language: self.language_name().to_string(),
            });
        }

        // Build command
        let build_result = executor.execute_command(
            "mylang-compiler",
            &["build", "--target", "wasm32", &config.project_path],
            "Failed to compile with MyLang compiler",
        )?;

        Ok(BuildResult {
            wasm_file: build_result.output_file,
            js_file: None,
            additional_files: vec![],
            has_bindgen: false,
        })
    }

    fn check_dependencies(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !CommandExecutor::is_tool_installed("mylang-compiler") {
            missing.push("mylang-compiler".to_string());
        }

        missing
    }

    fn validate_project(&self, path: &str) -> CompilationResult<()> {
        let project_path = std::path::Path::new(path);

        if !project_path.join("main.ml").exists() && !project_path.join("package.myl").exists() {
            return Err(CompilationError::ProjectValidationFailed {
                language: self.language_name().to_string(),
                reason: "No entry file found (main.ml or package.myl)".to_string(),
            });
        }

        Ok(())
    }

    fn clean(&self, path: &str) -> Result<()> {
        let build_dir = std::path::Path::new(path).join("build");
        if build_dir.exists() {
            std::fs::remove_dir_all(build_dir)?;
        }
        Ok(())
    }

    fn supported_extensions(&self) -> &[&str] {
        &["ml", "myl"]
    }

    fn entry_file_candidates(&self) -> &[&str] {
        &["main.ml", "package.myl", "app.ml"]
    }
}
```

### Step 4: Register the Plugin

Add your plugin to the built-in plugin registry:

```rust
// In src/plugin/languages/mod.rs
pub mod my_language_plugin;
pub use my_language_plugin::MyLanguagePlugin;

// In src/plugin/builtin.rs or manager.rs
pub fn get_builtin_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(CPlugin::new()),
        Box::new(MyLanguagePlugin::new()), // Add your plugin here
    ]
}
```

## Creating an External Plugin

External plugins are distributed as separate crates that integrate with Wasmrun via FFI (Foreign Function Interface).

### External Plugin Benefits

- **Distribution**: Available on crates.io, installed like `cargo install`
- **Isolation**: Installed to `~/.wasmrun/` directory
- **Dynamic Loading**: Loaded at runtime via shared libraries
- **Same Interface**: Use identical traits as built-in plugins
- **No Wasmrun Recompilation**: Users install plugins independently

### Step 1: Create a New Crate

```toml
# Cargo.toml
[package]
name = "wasmrun-mylang"
version = "0.1.0"
edition = "2021"
description = "MyLang WebAssembly compiler plugin for Wasmrun"
license = "MIT"
repository = "https://github.com/yourusername/wasmrun-mylang"

[lib]
crate-type = ["cdylib"]  # Required for dynamic loading

[dependencies]
wasmrun = { version = "0.15", features = ["plugin-api"] }
```

### Step 2: Implement Plugin in lib.rs

```rust
// src/lib.rs
use wasmrun::plugin::{Plugin, PluginInfo, WasmBuilder, PluginCapabilities, PluginType};
use wasmrun::compiler::builder::{BuildConfig, BuildResult, CompilationResult};

pub struct MyExternalPlugin {
    info: PluginInfo,
}

impl MyExternalPlugin {
    pub fn new() -> Self {
        let info = PluginInfo {
            name: "mylang".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "MyLang WebAssembly compiler".to_string(),
            author: "Your Name".to_string(),
            extensions: vec!["ml".to_string()],
            entry_files: vec!["main.ml".to_string()],
            plugin_type: PluginType::External,
            source: Some("https://github.com/yourusername/wasmrun-mylang".to_string()),
            dependencies: vec!["mylang-compiler".to_string()],
            capabilities: PluginCapabilities {
                compile_wasm: true,
                compile_webapp: false,
                live_reload: true,
                optimization: true,
                custom_targets: vec![],
            },
        };
        Self { info }
    }
}

impl Plugin for MyExternalPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn can_handle_project(&self, path: &str) -> bool {
        // Implement project detection logic
        std::path::Path::new(path).join("mylang.config").exists()
    }

    fn get_builder(&self) -> Box<dyn WasmBuilder> {
        Box::new(MyLanguageBuilder::new())
    }
}

// Export plugin creation function for FFI
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    Box::into_raw(Box::new(MyExternalPlugin::new()))
}
```

### Step 3: Create Plugin Manifest

```toml
# wasmrun.toml
[plugin]
name = "mylang"
version = "0.1.0"
description = "MyLang WebAssembly compiler plugin"
author = "Your Name"

[dependencies]
system = ["mylang-compiler >= 1.0.0"]

[capabilities]
compile_wasm = true
compile_webapp = false
live_reload = true
optimization = true
```

### Step 4: Publish to crates.io

```bash
cargo publish
```

### Step 5: Installation by Users

```bash
wasmrun plugin install wasmrun-mylang
```

This will:
1. Download from crates.io using `cargo install`
2. Compile plugin to `~/.wasmrun/plugins/mylang/target/release/`
3. Extract capabilities from plugin's manifest
4. Register plugin with Wasmrun

## Plugin Development Best Practices

### Use Shared Utilities

Use the shared utilities provided by Wasmrun:

```rust
// ✅ Good - use shared CommandExecutor
use crate::utils::command::CommandExecutor;

let executor = CommandExecutor::new(&config.project_path);
let copied = CommandExecutor::copy_to_output(&source, &output_dir, "Language")?;
if CommandExecutor::is_tool_installed("tool") { /* ... */ }

// ❌ Bad - duplicate implementation
fn my_execute_command() { /* ... */ }
```

### Consistent Error Handling

Use CompilationError types:

```rust
// ✅ Good - use CompilationError types
return Err(CompilationError::BuildToolNotFound {
    tool: "compiler".to_string(),
    language: self.language_name().to_string(),
});

return Err(CompilationError::BuildFailed {
    language: self.language_name().to_string(),
    reason: "Specific reason".to_string(),
});
```

### Comprehensive Plugin Info

Provide detailed plugin information:

```rust
let info = PluginInfo {
    name: "language".to_string(),
    version: env!("CARGO_PKG_VERSION").to_string(),
    description: "Clear description of what this plugin does".to_string(),
    author: "Wasmrun Team".to_string(),
    extensions: vec!["ext1".to_string(), "ext2".to_string()],
    entry_files: vec!["main.ext".to_string(), "build.config".to_string()],
    plugin_type: PluginType::Builtin,
    source: None,
    dependencies: vec![],
    capabilities: PluginCapabilities {
        compile_wasm: true,
        compile_webapp: false,
        live_reload: true,
        optimization: true,
        custom_targets: vec!["target1".to_string()],
    },
};
```

### Debug Logging

Add debug logging for troubleshooting:

```rust
use wasmrun::{debug_enter, debug_exit, debug_println};

pub fn build(&self, config: &BuildConfig) -> CompilationResult<BuildResult> {
    debug_enter!("build", "config={:?}", config);

    debug_println!("Checking dependencies for {}", self.language_name());
    let missing_deps = self.check_dependencies();

    if !missing_deps.is_empty() {
        debug_println!("Missing dependencies: {:?}", missing_deps);
        return Err(CompilationError::DependencyError {
            tool: missing_deps.join(", "),
            language: self.language_name().to_string(),
        });
    }

    // Build logic...

    debug_exit!("build");
    Ok(result)
}
```

### Testing Your Plugin

Create test fixtures for your plugin:

```
tests/
└── fixtures/
    └── mylang_project/
        ├── main.ml
        └── mylang.config
```

Write integration tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_can_handle_project() {
        let plugin = MyLanguagePlugin::new();
        assert!(plugin.can_handle_project("tests/fixtures/mylang_project"));
    }

    #[test]
    fn test_check_dependencies() {
        let builder = MyLanguageBuilder::new();
        let missing = builder.check_dependencies();
        // Verify dependency checking logic
    }
}
```

## Plugin Configuration Schema

External plugins use a standardized configuration format:

```toml
# wasmrun.toml
[plugin]
name = "plugin-name"
version = "1.0.0"
description = "Plugin description"
author = "Author Name"

[dependencies]
system = ["tool1 >= 1.0", "tool2"]
wasmrun = "0.15.0"

[capabilities]
compile_wasm = true
compile_webapp = false
live_reload = true
optimization = true
custom_targets = ["wasm32-wasi", "wasm32-unknown-unknown"]

[settings]
default_optimization = "size"
entry_files = ["main.ext", "app.ext"]
file_extensions = ["ext", "extension"]
```

## Plugin Installation Architecture

External plugins are installed to `~/.wasmrun/`:

```
~/.wasmrun/
├── config.toml               # Global configuration & plugin registry
├── bin/                      # Optional binaries (if plugins provide CLI tools)
│   ├── wasmrust              # Rust plugin binary (optional)
│   └── wasmgo                # Go plugin binary (optional)
├── plugins/                  # Plugin source & metadata
│   ├── wasmrust/             # Rust plugin installation
│   │   ├── Cargo.toml        # Plugin build configuration
│   │   ├── src/lib.rs        # Plugin implementation
│   │   ├── target/release/   # Compiled artifacts
│   │   │   └── libwasmrust.dylib # Shared library for FFI
│   │   └── .wasmrun_metadata # Plugin capabilities & dependencies
│   └── wasmgo/               # Go plugin installation
│       ├── Cargo.toml
│       ├── src/lib.rs
│       ├── target/release/
│       │   └── libwasmgo.dylib
│       └── .wasmrun_metadata
├── cache/                    # Build artifact cache
└── logs/                     # Plugin operation logs
```

## Plugin Loading Process

### Installation

```bash
wasmrun plugin install wasmrust
```

1. Downloads from crates.io using `cargo install`
2. Compiles plugin to `~/.wasmrun/plugins/wasmrust/target/release/`
3. Extracts capabilities from plugin's `Cargo.toml` metadata
4. Updates wasmrun config with plugin information

### Runtime Loading

When processing a project:

1. Detects project type (`.rs` files, `Cargo.toml`)
2. Loads `libwasmrust.dylib` dynamically via `libloading`
3. Calls plugin functions directly via FFI interface
4. No subprocess overhead, direct integration

## Examples

### Real-World Plugin Examples

Study existing plugins:

- **C/C++ Plugin**: `src/plugin/languages/c_plugin.rs` (built-in)
- **Rust Plugin**: [wasmrust](https://github.com/anistark/wasmrust) (external)
- **Go Plugin**: [wasmgo](https://github.com/farhaanbukhsh/wasmgo) (external)
- **Python Plugin**: [waspy](https://github.com/wasmfx/waspy) (external)
- **AssemblyScript Plugin**: [wasmasc](https://github.com/anistark/wasmasc) (external)

## Next Steps

- **[Contributing](contributing.md)**: Learn how to contribute your plugin
- **[Debugging](debugging.md)**: Debug your plugin implementation
- **[Architecture](architecture.md)**: Understand Wasmrun's architecture
