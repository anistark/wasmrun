---
sidebar_position: 8
title: init
---

# wasmrun init

Create a new project from a template.

## Synopsis

```sh
wasmrun init [NAME] [--template <TEMPLATE>] [--directory <DIR>]
```

**Aliases:** `new`

## Description

Writes a small, working project into a new directory and prints the command that runs it. Each template is one of the repository's [examples](https://github.com/anistark/wasmrun/tree/main/examples), compiled into wasmrun, so a new project is one that CI already builds and runs. The example's name is replaced with yours wherever it appears: the crate or package name, the module path, the README.

Most templates are for server mode, but `nodejs` and `wasi` start projects for the other modes, so `init` is the starting point for all of them.

## Options

| Option | Default | Description |
|---|---|---|
| `NAME` | `my-wasmrun-project` | The project's name, used for the directory and inside the files |
| `-t, --template <TEMPLATE>` | `rust` | Which template to start from (see below) |
| `-d, --directory <DIR>` | `NAME` | Where to create it, when that differs from the name |

`init` refuses a directory that already exists, so it never writes over your files.

## Templates

| Template | Mode | Comes from | Run it with |
|---|---|---|---|
| `rust` | [Server](../index.md) | `examples/rust-hello` | `wasmrun my-app` |
| `go` | Server | `examples/go-hello` | `wasmrun my-app` |
| `c` | Server | `examples/c-hello` | `wasmrun my-app` |
| `asc` | Server | `examples/asc-hello` | `wasmrun my-app` |
| `python` | Server | `examples/python-hello` | `wasmrun my-app` |
| `nodejs` | [OS](../../os/index.md), or an [agent session's server](../../agent/usage/serving.md) | `examples/nodejs-http-api` | `wasmrun os my-app` |
| `wasi` | [Exec](../../exec/index.md) | `examples/native-rust` | `cargo build --release --target wasm32-wasip1`, then `wasmrun exec` the `.wasm` |

A server-mode template still needs its language's toolchain and plugin to build, as any project does. See [Languages](../languages/rust.md) for what each one needs.

## Usage

```sh
wasmrun init hello-wasm
```

```
  + Cargo.toml
  + src/lib.rs
  + README.md
  + .gitignore

✅ Created hello-wasm from the rust template (server mode). Next:
   wasmrun hello-wasm
```

A Node.js HTTP server for OS mode:

```sh
wasmrun init api --template nodejs
wasmrun os api
```

A WASI program for exec mode, in a directory other than its name:

```sh
wasmrun init tool -t wasi -d tools/tool
cd tools/tool && cargo build --release --target wasm32-wasip1
wasmrun exec target/wasm32-wasip1/release/tool.wasm
```

## See Also

- [run](./run.md): serve the project once it exists
- [Quick Start](../../quick-start.md)
