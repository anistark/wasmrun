# clean

Clean build artifacts and temporary files.

## Synopsis

```bash
wasmrun clean [PROJECT] [OPTIONS]
```

**Aliases:** `clear`, `reset`

## Description

The `clean` command removes build artifacts, temporary files, and cached data created by Wasmrun. This is useful for:

- Freeing up disk space
- Troubleshooting build issues
- Ensuring fresh builds
- Resetting project state

## Options

### `-p, --path <PATH>`

Path to the project directory to clean.

```bash
wasmrun clean --path ./my-project
wasmrun clean -p ./my-project
```

Default: Current directory (`.`)

:::tip
You can also use a positional argument: `wasmrun clean ./my-project`
:::

### `-a, --all`

Clean both project artifacts AND Wasmrun's temporary directories.

```bash
wasmrun clean --all
wasmrun clean -a
```

Without `--all`, only project-specific build artifacts are removed.

## What Gets Cleaned

### Project Artifacts (Default)

When running `wasmrun clean`:

- `target/` - Rust build directory
- `dist/` - Output directory
- `build/` - Build artifacts
- `*.wasm` - Generated WASM files (in project root)
- `*.js` - Generated JS glue code
- `.wasmrun-cache/` - Project-specific cache

### With `--all` Flag

Additionally cleans Wasmrun's global directories:

- `~/.wasmrun/temp/` - Temporary compilation files
- `~/.wasmrun/cache/` - Build cache
- `.wasmrun-server/` - Server state files

:::warning
The `--all` flag does NOT remove installed plugins from `~/.wasmrun/plugins/`. Use `wasmrun plugin uninstall` to remove plugins.
:::

## Examples

### Clean Current Project

Remove build artifacts from current directory:

```bash
wasmrun clean
```

Output:

```
🧹 Cleaning project...
   ✓ Removed target/
   ✓ Removed dist/
   ✓ Removed *.wasm files
✅ Clean complete!
```

### Clean Specific Project

Clean a specific project directory:

```bash
wasmrun clean ./my-rust-project
```

### Clean Everything

Remove all build artifacts and temp files:

```bash
wasmrun clean --all
```

Output:

```
🧹 Cleaning project...
   ✓ Removed target/
   ✓ Removed dist/
   ✓ Removed *.wasm files

🧹 Cleaning Wasmrun temp directories...
   ✓ Removed ~/.wasmrun/temp/
   ✓ Removed ~/.wasmrun/cache/
   ✓ Removed .wasmrun-server/

✅ Clean complete!
   Freed: 245 MB
```

### Clean Multiple Projects

Clean several projects:

```bash
wasmrun clean ./project1
wasmrun clean ./project2
wasmrun clean ./project3
```

Or with a loop:

```bash
for project in projects/*; do
    wasmrun clean "$project"
done
```

### Clean Before Build

Ensure fresh build:

```bash
wasmrun clean && wasmrun compile
```

## Use Cases

### Free Disk Space

Remove accumulated build artifacts:

```bash
# Clean current project
wasmrun clean --all

# Check freed space
du -sh ~/.wasmrun/
```

### Troubleshoot Build Issues

If builds are failing or behaving unexpectedly:

```bash
wasmrun clean --all
wasmrun compile --verbose
```

### CI/CD Fresh Builds

Ensure clean slate in CI:

```bash
# In CI script
wasmrun clean --all
wasmrun compile --optimization release
```

### Switch Optimization Levels

Clean before changing optimization:

```bash
wasmrun clean
wasmrun compile --optimization size
```

### Reset Development Environment

Start fresh:

```bash
wasmrun clean --all
wasmrun plugin install wasmrust
wasmrun run
```

## Language-Specific Cleaning

### Rust Projects

Removes:
- `target/` directory (Cargo build output)
- `Cargo.lock` is preserved
- `pkg/` (wasm-pack output)

### Go Projects

Removes:
- `*.wasm` files
- TinyGo build cache
- Generated artifacts

### Python Projects

Removes:
- `__pycache__/`
- `*.pyc` files
- `.waspy-build/`
- Generated WASM output

### C/C++ Projects

Removes:
- `*.o` object files
- `*.wasm` files
- Build directories

### AssemblyScript Projects

Removes:
- `build/` directory
- `*.wasm` files
- AssemblyScript build cache

## Safe Operations

The clean command is safe and:

- **Never deletes source code** - Only removes build artifacts
- **Preserves configuration** - Keeps `wasmrun.toml`, `Cargo.toml`, etc.
- **Keeps dependencies** - Preserves `node_modules/`, plugin installations
- **Confirms before deleting** - Shows what will be removed

## Performance

Cleaning is fast:

- Small projects: < 1 second
- Large projects: < 5 seconds
- With `--all` flag: < 10 seconds

## Comparison with Language Tools

### vs Cargo Clean

```bash
# Cargo
cargo clean

# Wasmrun
wasmrun clean
```

Wasmrun's clean also removes Wasmrun-specific artifacts beyond Cargo's scope.

### vs npm/pnpm clean

```bash
# npm
npm run clean  # (if configured)

# Wasmrun
wasmrun clean
```

Wasmrun handles WebAssembly build artifacts specifically.

## Troubleshooting

### Permission Denied

If you get permission errors:

```bash
# Check ownership
ls -la target/

# Fix permissions
chmod -R 755 target/
wasmrun clean
```

### Files Still Present

Some files may be locked by running processes:

```bash
# Stop server first
wasmrun stop

# Then clean
wasmrun clean --all
```

### Cannot Remove Directory

Directory in use by another process:

```bash
# Check for running processes
lsof | grep wasmrun

# Kill if necessary
pkill wasmrun

# Clean again
wasmrun clean
```

## Dry Run (Future Feature)

Currently, clean always removes files. A future `--dry-run` option will show what would be removed:

```bash
# Future feature
wasmrun clean --dry-run
```

## Selective Cleaning

To clean specific items only:

```bash
# Clean Rust target only
rm -rf target/

# Clean WASM files only
rm -f *.wasm

# Clean dist only
rm -rf dist/
```

## Integration with Other Commands

### Before Compile

```bash
wasmrun clean
wasmrun compile --optimization release
```

### Before Run

```bash
wasmrun clean --all
wasmrun run --watch
```

### After Development

```bash
# Done for the day
wasmrun stop
wasmrun clean --all
```

## See Also

- [compile](./compile.md) - Build projects
- [stop](./stop.md) - Stop running server
- [plugin](./plugin.md) - Manage plugins (not affected by clean)
