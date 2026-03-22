---
sidebar_position: 1
title: Usage Overview
---

# OS Mode Usage

The `os` command starts a browser-based execution environment with a WASM virtual machine.

## Synopsis

```sh
wasmrun os [PROJECT] [OPTIONS]
```

## Quick Reference

```sh
# Run current directory
wasmrun os

# Run a specific project
wasmrun os ./my-node-app

# With language and options
wasmrun os ./my-app --language python --port 3000 --watch
```

## Sub-Pages

| Topic | Description |
|---|---|
| [Running Projects](./running.md) | Start projects, language detection, basic workflow |
| [Language Selection](./language.md) | Auto-detection, manual override, supported runtimes |
| [Server Options](./server-options.md) | Port, CORS, verbose, watch mode configuration |
