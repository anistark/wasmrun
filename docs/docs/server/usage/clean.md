---
sidebar_position: 7
title: clean
---

# wasmrun clean

Remove build artifacts and temporary files.

## Synopsis

```sh
wasmrun clean [PROJECT] [OPTIONS]
```

**Aliases:** `clear`, `reset`

## Description

Removes build artifacts, compiled output, and cached data. Does not delete source code, configuration files, or installed plugins.

## Options

### `-p, --path <PATH>`

Path to the project directory to clean.

```sh
wasmrun clean --path ./my-project
wasmrun clean -p ./my-project
wasmrun clean ./my-project     # positional
```

Default: current directory (`.`)

### `-a, --all`

Clean both project artifacts and wasmrun's global temporary directories.

```sh
wasmrun clean --all
```

## What Gets Removed

### Project Artifacts (default)

| Language | Removed |
|---|---|
| Rust | `target/` |
| Go | `*.wasm` output |
| C/C++ | `*.o`, `*.wasm` |
| Python | `__pycache__/`, `*.pyc` |
| AssemblyScript | `build/` |
| All | `dist/`, generated `.wasm` and `.js` in project root |

### With `--all`

Additionally removes:

- `~/.wasmrun/temp/` — temporary compilation files
- `~/.wasmrun/cache/` — build cache
- `.wasmrun-server/` — server state files

:::warning
`--all` does **not** remove installed plugins from `~/.wasmrun/plugins/`. Use `wasmrun plugin uninstall` for that.
:::

## Examples

### Clean Current Project

```sh
wasmrun clean
```

### Clean Everything

```sh
wasmrun clean --all
```

### Clean Before Rebuild

```sh
wasmrun clean
wasmrun compile --optimization release
```

### Clean Multiple Projects

```sh
for project in projects/*; do
    wasmrun clean "$project"
done
```

## What's Preserved

- Source code
- Configuration files (`Cargo.toml`, `go.mod`, `package.json`, etc.)
- Lock files (`Cargo.lock`, `pnpm-lock.yaml`)
- Dependencies (`node_modules/`)
- Installed plugins (`~/.wasmrun/plugins/`)

## See Also

- [compile](./compile.md) — rebuild after cleaning
- [stop](./stop.md) — stop server before cleaning
