---
title: Languages
---

# Languages

Exec mode does not compile source code and does not use [compilation plugins](/docs/plugins). What it runs depends on how you invoke it.

## WebAssembly Binaries

`wasmrun exec` runs pre-built `.wasm` files through the built-in interpreter with [WASI support](./wasi.md). Any language that compiles to WASI-targeting WebAssembly works:

| Language | Typical toolchain | Compile with |
|---|---|---|
| Rust | `wasm32-wasip1` target | [`wasmrust` plugin](../server/languages/rust.md) or `cargo build` |
| Go | TinyGo | [`wasmgo` plugin](../server/languages/go.md) or `tinygo build` |
| C/C++ | Emscripten / wasi-sdk | [C/C++ guide](../server/languages/c-cpp.md) or `clang --target=wasm32-wasi` |
| AssemblyScript | `asc` | [`wasmasc` plugin](../server/languages/assemblyscript.md) |

The plugin-based compile step belongs to [Server Mode](/docs/server); use [`wasmrun compile`](../server/usage/compile.md) (or your own toolchain) to produce the `.wasm`, then run it:

```sh
wasmrun compile ./my-rust-project
wasmrun exec ./dist/output.wasm
```

## Source Execution (Agent API)

The [Agent API](./agent.md) extends exec mode with direct source execution. Language runtimes are not compiled locally; they are WASM modules fetched from [wasmhub](https://anistark.github.io/wasmhub/) and executed inside the same sandboxed interpreter:

| Language | Aliases | Runtime |
|---|---|---|
| JavaScript | `js`, `nodejs` | [wasmhub `nodejs` runtime](https://anistark.github.io/wasmhub/runtimes/nodejs/) (CommonJS `require()`, Node built-ins, web globals) |
| TypeScript | `ts`, `tsx` | Transpiled in-sandbox by the [wasmhub `swc` module](https://anistark.github.io/wasmhub/runtimes/swc/), then run on the `nodejs` runtime |

See [JavaScript runtime capabilities](./usage/agent-exec.md#javascript-runtime-capabilities) for the supported built-ins and limits. Unsupported languages (for example `python`) return HTTP 400; more runtimes will arrive as they land on wasmhub.

## What Goes Where

- **Compile a language to WASM**: [Server Mode languages](../server/languages/rust.md), powered by [plugins](/docs/plugins)
- **Run a `.wasm` file natively**: exec mode, this section
- **Run JS/TS source in a sandbox**: the [Agent API](./agent.md)
- **Browser-based multi-language execution**: [OS Mode language selection](../os/usage/language.md), also powered by wasmhub runtimes
