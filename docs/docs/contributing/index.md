---
sidebar_position: 1
title: Contributing
---

# Contributing to Wasmrun

Wasmrun is open source and welcomes contributions. This section covers everything you need to get started as a contributor.

## Quick Setup

```sh
# Clone the repo
git clone https://github.com/anistark/wasmrun.git
cd wasmrun

# Build
cargo build

# Run tests
cargo test

# Format and lint
cargo fmt
cargo clippy
```

## What's in This Section

- **[Architecture](./architecture.md)** — how the codebase is structured, module responsibilities, data flow
- **[How to Contribute](./how-to-contribute.md)** — code style, PR process, issue guidelines
- **[Debugging](./debugging.md)** — debug flags, verbose output, common debugging workflows
- **[Troubleshooting](./troubleshooting.md)** — solutions for common build and runtime issues
- **[Changelog](./changelog.mdx)** — release history and what changed in each version

## Repository Structure

```
wasmrun/
├── src/
│   ├── main.rs              # Entry point
│   ├── cli.rs               # CLI argument parsing (clap)
│   ├── commands/             # Subcommand handlers (run, exec, os, compile, etc.)
│   ├── runtime/              # WASM runtime, microkernel, WASI, scheduler
│   ├── server/               # HTTP dev server
│   ├── compiler/             # Language detection, build orchestration
│   ├── plugin/               # Plugin system (manager, registry, installer)
│   ├── config/               # Configuration and constants
│   ├── logging/              # Structured log system
│   └── utils/                # Path resolution, system info, WASM analysis
├── docs/                     # Docusaurus documentation (this site)
├── ui/                       # Preact UI for OS mode
├── templates/                # HTML/JS/CSS templates for server and OS mode
├── examples/                 # Example projects (Rust, Go, C, ASC, Python)
└── tests/                    # Integration tests
```
