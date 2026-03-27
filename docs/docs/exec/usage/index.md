---
sidebar_position: 1
title: Usage Overview
---

# Exec Mode Usage

The `exec` command runs WebAssembly files natively using wasmrun's built-in interpreter.

## Synopsis

```sh
wasmrun exec <WASM_FILE> [OPTIONS] [-- ARGS...]
```

## Quick Reference

```sh
# Run a WASM file
wasmrun exec ./program.wasm

# Pass arguments
wasmrun exec ./program.wasm hello world

# Call a specific function
wasmrun exec ./module.wasm --call add 5 3
```

## Sub-Pages

| Topic | Description |
|---|---|
| [Running WASM Files](./running.md) | Basic execution, entry points, output |
| [Function Calling](./functions.md) | Call specific exported functions with `--call` |
| [Argument Passing](./arguments.md) | Pass arguments to WASM programs |

For HTTP-based access (AI agents, automation), see [Agent API](../agent.md).
