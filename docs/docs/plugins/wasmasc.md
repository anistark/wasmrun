---
sidebar_position: 5
title: wasmasc
---

# wasmasc

AssemblyScript WebAssembly plugin for wasmrun.

## About

`wasmasc` compiles [AssemblyScript](https://www.assemblyscript.org/) projects (TypeScript-like syntax) to WebAssembly for wasmrun's [Server Mode](/docs/server), working with whichever JavaScript package manager the project already uses.

[![Crates.io Version](https://img.shields.io/crates/v/wasmasc)](https://crates.io/crates/wasmasc)

- **GitHub**: [anistark/wasmasc](https://github.com/anistark/wasmasc)
- **crates.io**: [crates.io/crates/wasmasc](https://crates.io/crates/wasmasc)
- **docs.rs**: [docs.rs/wasmasc](https://docs.rs/wasmasc)
- **lib.rs**: [lib.rs/crates/wasmasc](https://lib.rs/crates/wasmasc)

## Install

```sh
wasmrun plugin install wasmasc
```

**Requirements:**

- Node.js runtime
- `asc` (the AssemblyScript compiler), installed globally
- Optional: npm, yarn, pnpm, or bun (auto-detected; npm is the fallback)

## Usage

Once installed, wasmrun auto-detects AssemblyScript projects from an AssemblyScript dependency in `package.json` or the standard `assembly/` directory of `.ts` sources:

```sh
# Compile and serve with the dev server
wasmrun ./my-asc-project --watch

# Compile only
wasmrun compile ./my-asc-project

# Plugin management
wasmrun plugin info wasmasc
wasmrun plugin update wasmasc
```

## What It Covers

- **Direct `asc` compilation** of AssemblyScript to WASM
- **Package manager detection**: npm, yarn, pnpm, and bun workflows
- **Three optimization levels**: debug, release, and size
- **Live reload**: watch mode recompilation through wasmrun's dev server
- **Project auto-detection** from `package.json` and lockfiles

## What It Doesn't Cover

- **Web application packaging**: outside AssemblyScript's scope; the plugin produces WASM modules only
- **Full TypeScript**: AssemblyScript is a strict subset of TypeScript compiled ahead of time; arbitrary TS code and npm libraries won't compile
- **Running WASM**: execution is handled by wasmrun, not the plugin

## See Also

- [AssemblyScript language guide](../server/languages/assemblyscript.md): project setup and workflows
- [Plugin usage](./usage.md): install, update, and manage plugins
