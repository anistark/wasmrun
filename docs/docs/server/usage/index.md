---
sidebar_position: 1
title: Usage Overview
---

# Server Mode Commands

Server mode provides commands for compiling, serving, inspecting, and managing WebAssembly projects.

## Core Workflow

```sh
# Compile + serve in one step
wasmrun run ./my-project

# Or step by step
wasmrun compile ./my-project --output ./dist
wasmrun verify ./dist/output.wasm
wasmrun run ./dist/output.wasm
```

## Command Reference

| Command | Description |
|---|---|
| [`run`](./run.md) | Compile and serve a project or WASM file with a dev server |
| [`compile`](./compile.md) | Compile a project to WebAssembly |
| [`verify`](./verify.md) | Validate a WASM binary's structure and format |
| [`inspect`](./inspect.md) | Analyze a WASM module's exports, imports, memory, and sections |
| [`stop`](./stop.md) | Stop any running wasmrun server |
| [`clean`](./clean.md) | Remove build artifacts and temporary files |
