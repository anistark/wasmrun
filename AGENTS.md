# AGENTS.md: AI Coding Agent Instructions for Wasmrun

> Instructions for Claude Code, pi, Cursor, Copilot, and other AI coding agents working on this project.

---

## Project Overview

**Wasmrun** is a WebAssembly runtime. It compiles, runs, inspects, and manages WASM modules with multi-language support (Rust, Go, Python, C/C++, AssemblyScript) via a plugin architecture.

- **Repository:** https://github.com/anistark/wasmrun
- **Crate:** https://crates.io/crates/wasmrun
- **Docs:** https://wasmrun.readthedocs.io
- **License:** MIT
- **Minimum Rust Version:** 1.85
- **Recommended Rust Version:** 1.88

---

## ⚠️ The Four Modes: Read This First

Wasmrun has **four distinct execution modes**. They are separate systems with separate philosophies. **Do not conflate them.** When working on one mode, do not break another mode's functionality.

### 1. Server Mode (`wasmrun` / `wasmrun run`)

**Philosophy:** A development server that compiles source code to WASM and serves it in a browser with a UI.

- **Trigger:** `wasmrun run ./project` or just `wasmrun ./project`
- **What it does:** Detects project language → compiles to WASM via plugins → starts HTTP server → serves browser UI that loads and runs the WASM
- **Key files:**
  - `src/commands/run.rs`: command handler
  - `src/config/server.rs`: server config, `run_server()`
  - `src/server/`: HTTP server infrastructure (handler, API, wasm serving, lifecycle)
  - `src/compiler/`: project compilation
  - `src/plugin/`: plugin system (compile plugins)
  - `src/watcher.rs`: live reload file watching
  - `src/template.rs`: HTML template injection
  - `ui/src/`: Preact UI source (builds into `templates/app/`, `templates/console/` at compile time via `build.rs`)
- **Uses plugins:** Yes. Plugins provide compilation (wasmrust, wasmgo, waspy, wasmasc)
- **Uses browser:** Yes. Serves HTML + JS that loads WASM via `WebAssembly.instantiate()`
- **Docs:** `docs/docs/server/`

### 2. Exec Mode (`wasmrun exec`)

**Philosophy:** A native WASM interpreter. No browser, no server, no compilation. Just parse and execute a `.wasm` binary directly.

- **Trigger:** `wasmrun exec ./file.wasm [args...]`
- **What it does:** Parses WASM binary → initializes memory → links WASI host functions → interprets bytecode → prints output → returns exit code
- **Key files:**
  - `src/commands/exec.rs`: command handler
  - `src/runtime/core/`: **the entire WASM interpreter engine**
    - `module.rs`: binary parser
    - `executor.rs`: instruction executor (~4400 lines, all WASM opcodes)
    - `memory.rs`: linear memory (pages, bounds checking)
    - `values.rs`: value types (i32, i64, f32, f64)
    - `linker.rs`: host function imports/exports linking
    - `native_executor.rs`: high-level API: `execute_wasm_file()`, `execute_wasm_file_with_args()`
    - `control_flow.rs`: control flow analysis
  - `src/runtime/wasi/`: WASI syscall implementations (fd_write, args_get, clock, etc.)
    - `mod.rs`: WasiEnv, create_wasi_linker()
    - `syscalls.rs`: individual syscall host functions
- **Uses plugins:** No
- **Uses browser:** No
- **Docs:** `docs/docs/exec/`

### 3. OS Mode (`wasmrun os`)

**Philosophy:** A browser-based micro-kernel environment. Runs projects (Node.js, Python) inside a WASM VM in the browser with a full development UI (console, filesystem, logs, kernel status).

- **Trigger:** `wasmrun os ./project`
- **What it does:** Detects language → starts HTTP server → serves Preact UI → fetches language runtime WASM from wasmhub → populates virtual FS with project files → boots WASM VM in browser → runs user code sandboxed
- **Key files:**
  - `src/commands/os.rs`: command handler
  - `src/runtime/os_server.rs`: OS mode HTTP server (serves UI, APIs for kernel/fs/logs/tunnel)
  - `src/runtime/multilang_kernel.rs`: multi-language kernel (process management, language detection)
  - `src/runtime/microkernel.rs`: base micro-kernel (process table, WASI, VFS)
  - `src/runtime/dev_server.rs`: per-process dev server (serves WASI filesystem files)
  - `src/runtime/scheduler.rs`: process scheduler
  - `src/runtime/network_namespace.rs`: network isolation, port forwarding
  - `src/runtime/wasi_fs.rs`: virtual filesystem (in-memory, mount points)
  - `src/runtime/project_files.rs`: project file collection for browser transfer
  - `src/runtime/runtime_cache.rs`: language runtime WASM caching (from wasmhub)
  - `src/runtime/tunnel/`: bore tunneling for public access
  - `src/runtime/languages/`: language runtime traits (Node.js, Go, Python)
  - `src/logging/`: structured log trail system
  - `ui/src/`: Preact UI source (components, OS panels, WASI shim; builds into `templates/os/` at compile time via `build.rs`)
- **Uses plugins:** No (uses its own language detection and wasmhub runtimes)
- **Uses browser:** Yes. Full Preact UI with console, filesystem, kernel panels
- **Docs:** `docs/docs/os/`

### 4. Agent Mode (`wasmrun agent`)

**Philosophy:** A REST API that gives AI agents isolated sandboxes to execute code in. Exec mode's interpreter, wrapped in a long-lived HTTP server with sessions, resource limits, and multi-tenancy. No Docker, no daemon, no browser.

- **Trigger:** `wasmrun agent --port 8430`
- **What it does:** Starts an HTTP server → creates per-session sandboxes (isolated work dir + WASI env) → accepts code (shell command, JS/TS source, multi-file project, or `.wasm`) over REST → runs it on the exec interpreter → returns structured JSON (stdout, stderr, exit code, duration)
- **Key files:**
  - `src/commands/agent.rs`: command handler
  - `src/agent/server.rs`: HTTP server, routing, exec orchestration
  - `src/agent/session.rs`: session lifecycle, isolation, expiry
  - `src/agent/executor.rs`: source → runtime execution pipeline (JS, TS via swc)
  - `src/agent/esm.rs`: ESM → CommonJS lowering for vendored packages
  - `src/agent/vendor.rs`: npm dependency resolution and vendoring
  - `src/agent/shell.rs`: shell emulation over the session filesystem
  - `src/agent/auth.rs`, `limits.rs`, `metrics.rs`, `tools.rs`, `api.rs`
- **Executes with:** the exec-mode interpreter (`runtime/core/` + `runtime/wasi/`). A `.wasm` behaves identically under `wasmrun exec` and `POST /exec`
- **Uses plugins:** No
- **Uses browser:** No
- **Docs:** `docs/docs/agent/`

### Mode Boundaries: Critical Rules

1. **Never mix mode-specific logic.** The exec *interpreter core* (`runtime/core/`) must never start an HTTP server or know that one exists; agent mode owns that surface and calls into the interpreter from outside. Server mode must never invoke the bytecode interpreter. OS mode has its own kernel; don't route it through the server mode pipeline.

2. **Agent mode is built on exec mode's engine, not a copy of it.** Execution, WASI syscalls, and interpreter behaviour belong in `runtime/core/` and `runtime/wasi/`, where both modes get them. Agent mode owns only what exec mode has no concept of: sessions, transport, auth, quotas, metrics. Never reimplement interpreter or syscall logic inside `src/agent/`.

3. **Shared code is encouraged, but not at the cost of mode integrity.** If a utility function is useful across modes (e.g., path resolution, error types, WASM binary analysis), keep it in shared modules (`src/utils/`, `src/error.rs`, `src/config/`). But don't bend a mode's design just to share code.

4. **When in doubt, ask the user.** If a change could affect multiple modes and the intent is unclear, stop and ask.

5. **Mode-specific modules should be commented.** If a file/module belongs to a specific mode, include a comment at the top:
   ```rust
   //! OS mode: Multi-language kernel for browser-based WASM execution
   ```

6. **The plugin system belongs to Server Mode.** Plugins provide compilation support (Rust → WASM, Go → WASM, etc.). Exec mode does not compile; it runs pre-built `.wasm` files. OS mode uses wasmhub runtimes, not compilation plugins.

### Shared Components (used by multiple modes)

| Module | Used By | Purpose |
|--------|---------|---------|
| `src/error.rs` | All | Unified error types |
| `src/utils/` | All | Path resolution, WASM analysis, system utils |
| `src/config/constants.rs` | Server, OS | Port defaults, paths |
| `src/runtime/core/` | Exec, Agent | WASM interpreter engine (`module.rs` also used by Verify/Inspect) |
| `src/runtime/wasi/` | Exec, Agent | WASI syscall host functions for interpreter |
| `src/runtime/runtime_cache.rs` | OS, Agent | wasmhub language runtime fetching + caching |
| `src/runtime/wasi_fs.rs` | OS, Dev Server | Virtual in-memory filesystem |
| `src/commands/verify.rs` | Standalone (uses core module parser) | WASM verification |

### Mode Dependency Map

```
Server Mode                     Exec Mode                OS Mode                          Agent Mode
─────────────                   ─────────                ───────                          ──────────
src/commands/run.rs             src/commands/exec.rs     src/commands/os.rs               src/commands/agent.rs
src/config/server.rs            src/runtime/core/*       src/runtime/os_server.rs         src/agent/*
src/server/*                    src/runtime/wasi/*       src/runtime/multilang_kernel.rs
src/compiler/*                                           src/runtime/microkernel.rs       ↓ executes via
src/plugin/*                                             src/runtime/dev_server.rs        src/runtime/core/*
src/watcher.rs                                           src/runtime/scheduler.rs         src/runtime/wasi/*
src/template.rs                                          src/runtime/network_namespace.rs src/runtime/runtime_cache.rs
ui/src/ (→ templates/app/)                               src/runtime/wasi_fs.rs
ui/src/ (→ templates/console/)                           src/runtime/project_files.rs
                                                         src/runtime/runtime_cache.rs
                                                         src/runtime/tunnel/*
                                                         src/runtime/languages/*
                                                         src/logging/*
                                                         ui/src/ (→ templates/os/)
                                                         ui/src/*
```

---

## After Every Set of Changes

After completing any set of changes, **always** run these in order:

1. **`just format`**: Format all Rust and UI code.
2. **`just lint`**: Run clippy (zero warnings enforced) and UI ESLint.
3. **`just build`**: Full build (format → lint → test → release build).

If new functionality was added:

4. **`just test`**: Run the full test suite to ensure nothing is broken.

Do not consider a change complete until all of the above pass cleanly.

### Additional Housekeeping

- **Update `CHANGELOG.md`** as and when needed. Add entries under `[Unreleased]` for any user-facing changes (features, fixes, breaking changes).
- **Update `docs/docs/`** whenever behaviour, CLI usage, or features change. Place docs in the correct mode's section (server/exec/os).
- **Prefer `just` commands** over raw `cargo`/`pnpm` commands. The justfile handles sequencing, version sync, and cross-project builds correctly.
- **Prompt the user if `AGENTS.md` needs updating.** If your changes alter architecture, mode boundaries, CLI commands, key file locations, or behavioural conventions, tell the user: *"This change may require an update to AGENTS.md. Would you like me to update it?"*

### Planning Documents

- **Check `plan/` for active plans** when a related task is mentioned. Files like `plan/ROADMAP.md` and `plan/*_IMPLEMENTATION.md` contain detailed implementation plans, checklists, and phase tracking.
- **`plan/` is for local planning only.** It is gitignored; never commit it. Use it to understand context, track progress, and follow implementation checklists.

### Git Discipline

- **Do not run `git add` or `git commit` unless the user explicitly asks.** Stage and commit only on direct request.
- **When the user asks to commit**, review all staged/unstaged changes and prepare:
  - A **brief title** following conventional commits (`feat:`, `fix:`, `chore:`, etc.)
  - A **detailed description** summarizing what changed and why
  - If new CLI commands or flags were added, include their **usage examples** in the description:
    ```
    New command:
      wasmrun exec --call <function> <file.wasm> [args...]
      Calls a specific exported function from a WASM module.
    ```

---

## Architecture

Wasmrun is a single Rust binary (`wasmrun`) with three companion sub-projects:

| Component | Language | Location | Purpose |
|-----------|----------|----------|---------|
| **Core CLI & Runtime** | Rust | `src/` | CLI, WASM parser, interpreter, WASI, servers |
| **UI** | Preact + TypeScript | `ui/` | Browser-based OS mode interface |
| **Documentation** | Docusaurus + TypeScript | `docs/` | User-facing documentation site |

### Source Layout (`src/`)

```
src/
├── main.rs              # Entry point, command dispatch
├── cli.rs               # CLI argument parsing (clap)
├── error.rs             # Unified error types (WasmrunError)
├── agent/                # [Agent Mode] ★ REST sandbox API for AI agents
│   ├── server.rs        #   HTTP server, routing, exec orchestration
│   ├── session.rs       #   Session lifecycle and isolation
│   ├── executor.rs      #   Source → runtime execution pipeline (JS/TS)
│   ├── esm.rs           #   ESM → CommonJS lowering for vendored packages
│   ├── vendor.rs        #   npm resolution and vendoring
│   ├── shell.rs         #   Shell emulation over the session filesystem
│   ├── auth.rs          #   API keys and tenant isolation
│   ├── limits.rs        #   Resource limits (memory, fuel, output, disk)
│   ├── metrics.rs       #   Metrics endpoint and access log
│   ├── tools.rs         #   LLM tool schemas
│   └── api.rs           #   Request/response types
├── commands/             # Subcommand handlers
│   ├── run.rs           #   [Server Mode] compile + serve
│   ├── exec.rs          #   [Exec Mode] native WASM execution
│   ├── os.rs            #   [OS Mode] browser-based kernel
│   ├── agent.rs         #   [Agent Mode] REST sandbox server
│   ├── compile.rs       #   [Server Mode] compile only
│   ├── verify.rs        #   [Shared] WASM binary verification
│   ├── stop.rs          #   [Server Mode] stop running server
│   ├── clean.rs         #   [Shared] clean build artifacts
│   ├── plugin.rs        #   [Server Mode] plugin management
│   ├── module_display.rs #  [Shared] WASM module display formatting
│   └── issue_detector.rs #  [Shared] WASM module issue detection
├── compiler/             # [Server Mode] Project compilation
├── config/               # Constants, server config, plugin config
├── logging/              # [OS Mode] Structured log trail system
├── plugin/               # [Server Mode] Plugin system
├── runtime/
│   ├── core/             # [Exec, Agent] ★ WASM interpreter engine
│   │   ├── module.rs     #   Binary parser (shared with verify/inspect)
│   │   ├── executor.rs   #   Instruction executor (~4400 lines)
│   │   ├── memory.rs     #   Linear memory
│   │   ├── values.rs     #   Value types
│   │   ├── linker.rs     #   Host function linking
│   │   ├── native_executor.rs  # High-level exec API
│   │   ├── control_flow.rs     # Control flow analysis
│   │   └── tests.rs      #   Unit tests
│   ├── wasi/             # [Exec, Agent] WASI syscall implementations
│   │   ├── mod.rs        #   WasiEnv, linker setup
│   │   └── syscalls.rs   #   fd_write, args_get, clock, etc.
│   ├── os_server.rs      # [OS Mode] HTTP server + API endpoints
│   ├── multilang_kernel.rs # [OS Mode] Multi-language kernel
│   ├── microkernel.rs    # [OS Mode] Base micro-kernel
│   ├── dev_server.rs     # [OS Mode] Per-process dev server
│   ├── scheduler.rs      # [OS Mode] Process scheduler
│   ├── network_namespace.rs # [OS Mode] Network isolation
│   ├── wasi_fs.rs        # [OS Mode] Virtual in-memory filesystem
│   ├── project_files.rs  # [OS Mode] Project file bundling
│   ├── runtime_cache.rs  # [OS, Agent] Wasmhub runtime caching
│   ├── languages/        # [OS Mode] Language runtime traits
│   ├── tunnel/           # [OS Mode] Bore tunneling
│   ├── registry.rs       # [OS Mode] Process/server registry
│   └── syscalls.rs       # [OS Mode] Micro-kernel syscall interface
├── server/               # [Server Mode] HTTP server infrastructure
├── utils/                # [Shared] Path resolution, WASM analysis
├── template.rs           # [Server Mode] HTML template engine
├── ui.rs                 # UI asset embedding
└── watcher.rs            # [Server Mode] File watcher for live reload
```

### Key Architectural Patterns

- **Plugin-based compilation (Server Mode only):** Language support is via external crates.io plugins (`wasmrust`, `wasmgo`, `waspy`, `wasmasc`), except C/C++ (built-in via Emscripten).
- **Self-contained WASM interpreter (Exec Mode):** The `runtime/core/` module is a from-scratch WASM bytecode interpreter, with no dependency on wasmtime/wasmer.
- **WASI syscalls** are implemented as host functions linked via `Linker`, operating on `LinearMemory`. Used by Exec Mode.
- **Virtual filesystem (OS Mode):** `wasi_fs.rs` provides an in-memory filesystem with mount points, used by the OS mode kernel and dev server.
- **UI is embedded:** The `build.rs` script compiles the Preact UI (`ui/`) into `templates/` which get embedded in the binary.
- **Error handling:** Uses `thiserror` + `anyhow`. Custom `WasmrunError` enum in `src/error.rs` with sub-error types.

---

## Documentation Structure

The docs mirror the four-mode architecture:

```
docs/docs/
├── server/               # Server Mode documentation
│   ├── index.md          #   Overview
│   ├── features.md       #   Feature list
│   ├── live-reload.md    #   Live reload explanation
│   └── usage/            #   Commands: run, compile, verify, inspect, stop, clean
├── exec/                 # Exec Mode documentation
│   ├── index.md          #   Overview
│   ├── features.md       #   Feature list
│   ├── wasi.md           #   WASI support details
│   └── usage/            #   Running, arguments, function calls
├── os/                   # OS Mode documentation
│   ├── index.md          #   Overview
│   ├── features.md       #   Feature list
│   ├── network-isolation.md
│   ├── port-forwarding.md
│   ├── public-tunneling.md
│   └── usage/            #   Running, language selection, server options
├── agent/                # Agent Mode documentation
│   ├── index.md          #   Overview, CLI flags, auth, tenancy
│   └── usage/            #   Sessions, execution, files, environment, observability
├── plugins/              # Plugin system (Server Mode)
├── contributing/         # Development guides
├── installation.md
├── intro.md
└── quick-start.md
```

When updating documentation, place content in the correct mode's section. Don't document exec features in the server docs, and vice versa.

---

## Tech Stack & Tooling

| Tool | Purpose | Notes |
|------|---------|-------|
| **Rust** | Core runtime | Edition 2021, MSRV 1.85 |
| **Cargo** | Build system | `cargo build --release` |
| **Just** | Task runner | `justfile`: run `just` for available commands |
| **clap** | CLI parsing | Derive-based, see `src/cli.rs` |
| **Preact** | UI framework | In `ui/`, uses Vite + TypeScript + Tailwind |
| **pnpm** | JS package manager | For both `ui/` and `docs/` |
| **Docusaurus** | Documentation | In `docs/`, deployed to ReadTheDocs |
| **clippy** | Linting | Enforced: `-D warnings` (zero warnings policy) |
| **cargo fmt** | Formatting | Standard rustfmt |

---

## Build & Development

### Quick Commands

```sh
just build          # Format → lint → test → release build (includes UI build)
just test           # Run all Rust tests
just format         # Format Rust + UI code
just lint           # Clippy + UI ESLint
just clean          # Remove build artifacts
just docs-dev       # Start docs dev server
just docs-build     # Build docs for production
```

### Building from Source

```sh
# Full build (compiles UI first via build.rs, then Rust)
cargo build --release

# Skip UI build (faster for runtime-only changes)
SKIP_UI_BUILD=1 cargo build

# Run tests
cargo test

# Run specific test
cargo test test_name
```

### UI Development

```sh
cd ui
pnpm install
pnpm dev            # Vite dev server
pnpm build          # Production build (also triggered by cargo build via build.rs)
pnpm lint
pnpm format
pnpm type-check
```

### Documentation

```sh
cd docs
pnpm install
pnpm start          # Local dev server
pnpm build          # Production build
pnpm typecheck      # TypeScript check
```

---

## Testing

- **Unit tests** live alongside source code (standard Rust `#[cfg(test)]` modules).
- **Integration tests** are in `tests/` (currently `tests/exec_integration_tests.rs`).
- **Test count:** ~750+ tests across unit and integration suites.
- **Network-gated tests** are `#[ignore]`d so the suite stays offline-friendly; run them with `cargo test -- --ignored` (use `--test-threads=1` the first time, while `~/.wasmrun/runtimes` is cold).
- Always run `cargo test` before committing.
- The CI expects zero clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`.

---

## Code Conventions

### Rust

- Follow standard Rust naming: `snake_case` for functions/variables, `PascalCase` for types.
- Use `thiserror` for error types. Add new error variants to `src/error.rs` when needed.
- Keep `#[allow(dead_code)]` annotated with a `// TODO:` comment explaining the plan.
- Prefer `eprintln!` for user-facing error output. Use the `debug_println!` / `debug_enter!` / `debug_exit!` macros for debug-only output.
- The executor (`src/runtime/core/executor.rs`) is large by design; it's a single dispatch loop for all WASM opcodes. Keep instruction implementations in that file.
- Memory operations must always include bounds checking.
- WASI syscalls must return proper errno values (ESUCCESS, EINVAL, EBADF, etc.).
- **Comment mode ownership** on mode-specific files (e.g., `//! [OS Mode] ...` or `//! [Exec Mode] ...`).

### TypeScript (UI & Docs)

- Use Preact (not React) for the UI, importing from `preact` and `preact/hooks`.
- Vite handles three template builds: `app`, `console`, `os` (controlled by `VITE_TEMPLATE` env var).
- Follow existing component patterns in `ui/src/components/`.

### Commit Messages

Follow conventional commits:

```
feat: description          # New feature
fix: description           # Bug fix
chore: description         # Maintenance, deps, CI
docs: description          # Documentation only
refactor: description      # Code restructuring
test: description          # Adding/fixing tests
```

### Branching

- `main`: stable release branch
- `feat/*`: feature branches (e.g., `feat/wasi-filesystem-syscalls`)
- `fix/*`: bug fix branches
- `docs/*`: documentation branches
- PRs are squash-merged with descriptive titles.

---

## Versioning

- Version is the single source of truth in `Cargo.toml` (`version = "X.Y.Z"`).
- `just sync-version` propagates it to `docs/package.json` and `ui/package.json`.
- `just build` automatically syncs versions before building.
- Follow [Semantic Versioning](https://semver.org/).
- Update `CHANGELOG.md` with every meaningful change under `[Unreleased]`.

---

## CLI Commands Reference

```sh
# Server Mode
wasmrun <path>                    # Default: compile + serve with dev server
wasmrun run <path>                # Explicit run (same as default)
wasmrun compile <path>            # Compile project to WASM only
wasmrun verify <file.wasm>        # Validate WASM binary structure
wasmrun inspect <file.wasm>       # Analyze WASM binary (exports, imports, sections)
wasmrun stop                      # Stop running server
wasmrun clean <path>              # Clean build artifacts
wasmrun plugin list|install|update|uninstall  # Plugin management

# Exec Mode
wasmrun exec <file.wasm> [args]   # Execute WASM natively with interpreter
wasmrun exec <file.wasm> --call <func> [args]  # Call specific exported function

# OS Mode
wasmrun os <path>                 # Run project in browser-based OS environment
wasmrun os <path> --language python  # Force language detection
wasmrun os <path> --watch --port 3000  # With file watching and custom port

# Agent Mode
wasmrun agent                     # Start the REST sandbox API (default port 8430)
wasmrun agent --port 8430 --max-sessions 100  # With explicit port and session cap
wasmrun agent --auth ./tenants.toml  # Enable API-key auth and tenant isolation
```

---


## Important Files to Know

| File | Mode | Why It Matters |
|------|------|----------------|
| `Cargo.toml` | All | Dependencies, version, metadata; start here |
| `src/cli.rs` | All | All CLI arguments and subcommands defined here |
| `src/main.rs` | All | Command dispatch; maps CLI args to handlers |
| `src/error.rs` | All | All error types; extend here for new error categories |
| `src/commands/run.rs` | Server | Server mode entry point |
| `src/commands/exec.rs` | Exec | Exec mode entry point |
| `src/commands/os.rs` | OS | OS mode entry point |
| `src/commands/agent.rs` | Agent | Agent mode entry point |
| `src/agent/server.rs` | Agent | Agent HTTP server, routing, exec orchestration |
| `src/agent/session.rs` | Agent | Session lifecycle and sandbox isolation |
| `src/runtime/core/executor.rs` | Exec | The WASM interpreter (~4400 lines) |
| `src/runtime/core/module.rs` | Exec/Shared | WASM binary parser |
| `src/runtime/wasi/syscalls.rs` | Exec | WASI syscall implementations |
| `src/runtime/wasi/mod.rs` | Exec | WASI environment and linker setup |
| `src/runtime/os_server.rs` | OS | OS mode server (1500+ lines, all API endpoints) |
| `src/runtime/multilang_kernel.rs` | OS | Multi-language kernel |
| `src/config/server.rs` | Server | Server config and `run_server()` |
| `src/plugin/mod.rs` | Server | Plugin system definitions |
| `build.rs` | All | Build script; compiles UI into templates |
| `justfile` | All | All development task commands |
| `CHANGELOG.md` | - | Keep updated with every change |

---

## Common Tasks for Agents

### Adding a new CLI subcommand

1. Add the variant to `Commands` enum in `src/cli.rs`
2. Create handler in `src/commands/new_command.rs`
3. Export from `src/commands/mod.rs`
4. Add match arm in `src/main.rs`
5. Add tests
6. **Decide which mode it belongs to** and document accordingly

### Adding a new WASI syscall (Exec Mode)

1. Implement the syscall function in `src/runtime/wasi/syscalls.rs`
2. Register it in `create_wasi_linker()` in `src/runtime/wasi/mod.rs`
3. Ensure it reads/writes linear memory correctly via `&mut LinearMemory`
4. Return proper WASI errno values
5. Add unit tests
6. Update docs in `docs/docs/exec/wasi.md`
7. Agent mode shares this layer, so verify the syscall through `POST /exec` too

### Adding a new WASM instruction (Exec Mode)

1. Add the variant to `Instruction` enum in `src/runtime/core/executor.rs`
2. Add decoding logic in `decode_instruction()`
3. Add execution logic in `dispatch_instruction()`
4. Add unit test in `src/runtime/core/tests.rs` or inline `#[cfg(test)]`

### Adding an Agent Mode API endpoint

1. Add the route match in `AgentServer::handle_request()` in `src/agent/server.rs`
2. Implement the handler as a method returning `Result<T, ApiError>` (the HTTP layer stays separate from the logic, which is what makes handlers unit-testable)
3. Add request/response types to `src/agent/api.rs`
4. If agents should discover it, add a tool schema in `src/agent/tools.rs`
5. Update docs in `docs/docs/agent/`

### Adding an OS Mode API endpoint

1. Add the route match in `OsServer::handle_request()` in `src/runtime/os_server.rs`
2. Implement the handler method on `OsServer`
3. Add corresponding UI component in `ui/src/` if needed
4. Update docs in `docs/docs/os/`

### Adding a Server Mode plugin

1. Follow the plugin trait in `src/plugin/mod.rs`
2. Implement `Plugin` trait with `can_handle_project()` and `get_builder()`
3. Register in `src/plugin/manager.rs`
4. Update docs in `docs/docs/plugins/`

### Modifying the UI (OS Mode)

1. Edit components in `ui/src/`
2. Test with `cd ui && pnpm dev`
3. The `build.rs` will rebuild templates on `cargo build`
4. Three template modes: `app`, `console`, `os`: controlled via `VITE_TEMPLATE`

---

## Gotchas & Pitfalls

- **`build.rs` compiles the UI.** If you don't have `pnpm` or `node`, set `SKIP_UI_BUILD=1` to bypass.
- **The `templates/` directory is gitignored**: it's generated at build time. Don't commit it.
- **Executor is intentionally large.** Don't try to split `executor.rs` into multiple files; the dispatch loop benefits from being co-located.
- **WASM uses little-endian** byte order for all memory operations.
- **Division by zero** in WASM should trap (return error), not panic.
- **clippy must pass with zero warnings**: the CI enforces `-D warnings`.
- **Version must stay in sync** across `Cargo.toml`, `ui/package.json`, and `docs/package.json`. Use `just sync-version`.
- **Two different WASI systems exist:** `src/runtime/wasi/` is for Exec and Agent Mode (host functions linked to the interpreter). `src/runtime/wasi_fs.rs` is for OS Mode (virtual filesystem in browser). Don't confuse them.
- **Three different "server" concepts:** Server Mode's HTTP server (`src/server/`) serves WASM files for browser execution. OS Mode's HTTP server (`src/runtime/os_server.rs`) serves the OS UI and APIs. Agent Mode's HTTP server (`src/agent/server.rs`) serves the REST sandbox API. They are independent.
- **A WASI syscall change affects two modes.** `src/runtime/wasi/` is reached by `wasmrun exec` and by every agent-mode execution, so test both when touching it.

---

## Examples

The `examples/` directory contains sample projects in various languages:

- `rust-hello/`: Rust WASM project (Server Mode)
- `go-hello/`: Go WASM project (Server Mode)
- `c-hello/`: C WASM project via Emscripten (Server Mode)
- `asc-hello/`: AssemblyScript WASM project (Server Mode)
- `python-hello/`: Python WASM project (Server Mode)
- `native-rust/`: Native Rust → WASM (Exec Mode)
- `native-go/`: Native Go → WASM (Exec Mode)
- `web-leptos/`: Leptos web framework example (Server Mode)
- `web-asc/`: AssemblyScript web example (Server Mode)
- `nodejs-express-api/`: Node.js Express API (OS Mode)

Use these for testing. Integration tests in `tests/exec_integration_tests.rs` build and run some of these.
