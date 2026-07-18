---
sidebar_position: 2
title: C/C++
---

# C/C++

Built-in C/C++ WebAssembly plugin, shipped with wasmrun.

## About

The `c` plugin compiles C and C++ projects to WebAssembly using [Emscripten](https://emscripten.org/). It is **built-in**: it ships inside the wasmrun binary, needs no separate installation, and its version tracks wasmrun itself.

- **Source**: [`src/plugin/languages/c_plugin.rs`](https://github.com/anistark/wasmrun/blob/main/src/plugin/languages/c_plugin.rs) in the wasmrun repository

## Install

Nothing to install for the plugin itself; it is always available.

**Requirements:**

- [Emscripten SDK](https://emscripten.org/docs/getting_started/downloads.html), with `emcc` on your `PATH`

```sh
# Verify Emscripten is available
emcc --version
```

## Usage

wasmrun auto-detects C/C++ projects from entry files (`main.c`, `src/main.c`, `app.c`, `index.c`) or a build file (`Makefile`, `CMakeLists.txt`):

```sh
# Compile and serve with the dev server
wasmrun ./my-c-project --watch

# Compile only
wasmrun compile ./my-c-project

# Plugin info
wasmrun plugin info c
```

If the project has a `Makefile` (also `makefile` or `GNUmakefile`), the plugin builds through it; otherwise it invokes `emcc` directly on the detected entry file.

## What It Covers

- **C and C++ sources**: `.c`, `.h`, `.cpp`, `.hpp`, `.cc`, `.cxx`
- **Makefile builds**: existing build setups are used as-is when present
- **Direct `emcc` compilation** with `.wasm` + `.js` glue output
- **Web application output**: Emscripten's `web` target alongside plain `wasm`
- **Optimization levels** and **live reload** through wasmrun's dev server

## What It Doesn't Cover

- **Installing Emscripten**: the SDK must be set up separately; wasmrun reports a clear error when `emcc` is missing
- **CMake builds**: `CMakeLists.txt` is used for project detection, but compilation goes through Make or direct `emcc`, not a CMake pipeline
- **Running WASM**: execution is handled by wasmrun ([Server](/docs/server), [Exec](/docs/exec), or [OS](/docs/os) mode)

## See Also

- [C/C++ language guide](../server/languages/c-cpp.md): project setup and workflows
- [Plugin usage](./usage.md): manage plugins
