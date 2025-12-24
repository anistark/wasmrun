# verify

Verify WebAssembly file format and structure.

## Synopsis

```bash
wasmrun verify <WASM_FILE> [OPTIONS]
```

## Description

The `verify` command validates that a WebAssembly file is correctly formatted and structurally valid. It checks:

- Magic number (`\0asm`)
- Version header
- Section structure
- Type signatures
- Function declarations
- Import/export validity
- Memory and table constraints

This is useful for:
- Debugging compilation issues
- Validating third-party WASM files
- CI/CD quality checks
- Pre-deployment verification

## Arguments

### `<WASM_FILE>`

Path to the WebAssembly file to verify (required).

```bash
wasmrun verify ./module.wasm
wasmrun verify /path/to/output.wasm
```

You can also use the `--path` flag:

```bash
wasmrun verify --path ./module.wasm
wasmrun verify -p ./module.wasm
```

## Options

### `-d, --detailed`

Show detailed verification results including section information.

```bash
wasmrun verify ./module.wasm --detailed
wasmrun verify ./module.wasm -d
```

## Examples

### Basic Verification

Verify a WASM file:

```bash
wasmrun verify ./output.wasm
```

Success output:

```
✅ WebAssembly file is valid
   File: ./output.wasm
   Size: 245 KB
   Format: WebAssembly 1.0
```

### Detailed Verification

Get comprehensive information:

```bash
wasmrun verify ./module.wasm --detailed
```

Detailed output:

```
✅ WebAssembly file is valid

File Information:
  Path: ./module.wasm
  Size: 245 KB (251,392 bytes)
  Format: WebAssembly 1.0
  Magic: \0asm
  Version: 1

Sections Found:
  ✓ Type Section (12 types)
  ✓ Import Section (3 imports)
  ✓ Function Section (45 functions)
  ✓ Table Section (1 table)
  ✓ Memory Section (1 memory, min: 16 pages)
  ✓ Global Section (2 globals)
  ✓ Export Section (8 exports)
  ✓ Code Section (45 function bodies)
  ✓ Data Section (3 data segments)
  ✓ Custom Section: "name" (debug names)

Validation:
  ✓ All function signatures valid
  ✓ All imports properly typed
  ✓ All exports reference valid indices
  ✓ Memory constraints satisfied
  ✓ Table constraints satisfied
  ✓ No invalid opcodes
```

### Verify Multiple Files

Check all WASM files in a directory:

```bash
for file in dist/*.wasm; do
    wasmrun verify "$file"
done
```

### CI/CD Integration

GitHub Actions example:

```yaml
- name: Verify WASM Build
  run: wasmrun verify ./dist/module.wasm --detailed
```

## Verification Checks

### File Format

- Checks magic number is `0x00 0x61 0x73 0x6D` (`\0asm`)
- Verifies version is `0x01 0x00 0x00 0x00` (version 1)

### Structure Validation

- Section order is correct
- Section sizes match content
- No duplicate sections (except Custom)
- Section IDs are valid

### Type Checking

- Function signatures are well-formed
- Import signatures match types
- Export references are valid
- Table element types are correct

### Constraints

- Memory limits are within bounds
- Table limits are within bounds
- Global types are valid
- Data segments fit in memory

## Exit Codes

- `0` - File is valid
- `1` - File is invalid or error occurred

## Common Errors

### Invalid Magic Number

```
❌ Invalid WebAssembly file
   Error: Invalid magic number

   Expected: \0asm (0x00 0x61 0x73 0x6D)
   Found: Different bytes
```

Cause: File is not a WebAssembly module.

Solution: Ensure file was compiled correctly.

### Invalid Version

```
❌ Invalid WebAssembly version

   Expected: 1 (0x01 0x00 0x00 0x00)
   Found: Different version
```

Cause: WASM file uses unsupported version.

Solution: Recompile with standard WASM output.

### Malformed Section

```
❌ Validation failed
   Error: Malformed section at offset 0x1234

   Section: Function
   Issue: Size mismatch
```

Cause: Corrupted or incorrectly generated WASM.

Solution: Recompile the module.

### Type Mismatch

```
❌ Validation failed
   Error: Type mismatch in function 5

   Expected: (i32, i32) -> i32
   Found: (i32) -> i32
```

Cause: Function signature doesn't match declaration.

Solution: Fix function signature in source code and recompile.

### Import/Export Error

```
❌ Validation failed
   Error: Export references invalid function index

   Export: "calculate"
   Function index: 100
   Available functions: 45
```

Cause: Export points to non-existent function.

Solution: Recompile to fix indices.

## Use Cases

### Pre-Deployment Check

Verify before deploying to production:

```bash
#!/bin/bash
if wasmrun verify ./dist/app.wasm; then
    echo "Deploying..."
    # deployment commands
else
    echo "Validation failed!"
    exit 1
fi
```

### Build Verification

After compilation:

```bash
wasmrun compile ./my-project
wasmrun verify ./output.wasm --detailed
```

### Automated Testing

In test scripts:

```bash
# test.sh
wasmrun compile --optimization release
wasmrun verify ./dist/*.wasm || exit 1
wasmrun exec ./dist/test.wasm
```

### Debug Compilation Issues

If compilation produces unexpected results:

```bash
wasmrun compile ./project --verbose
wasmrun verify ./output.wasm --detailed
```

## Comparison with Other Tools

### wasm-validate (WABT)

```bash
# WABT tool
wasm-validate module.wasm

# Wasmrun equivalent
wasmrun verify module.wasm
```

Wasmrun's verify provides:
- Colored output
- More detailed error messages
- Integration with Wasmrun workflow

### wasm-objdump

For inspection rather than validation:

```bash
# For detailed inspection
wasmrun inspect module.wasm

# For validation only
wasmrun verify module.wasm
```

## Integration with Other Commands

### After Compilation

```bash
wasmrun compile ./project
wasmrun verify ./output.wasm
```

### Before Execution

```bash
wasmrun verify ./app.wasm && wasmrun exec ./app.wasm
```

### With Inspection

```bash
wasmrun verify ./module.wasm --detailed
wasmrun inspect ./module.wasm
```

## Performance

Verification is fast:

- Small files (< 1 MB): < 100ms
- Medium files (1-10 MB): < 500ms
- Large files (> 10 MB): < 2s

## See Also

- [inspect](./inspect.md) - Detailed WASM inspection
- [compile](./compile.md) - Compile projects to WASM
- [exec](./exec.md) - Execute WASM files
