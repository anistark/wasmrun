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

Wasmrun has **four distinct execution modes**, plus a process manager built on one of them. They are separate systems with separate philosophies. **Do not conflate them.** When working on one mode, do not break another mode's functionality.

### 1. Server Mode (`wasmrun` / `wasmrun run`)

**Philosophy:** A development server that compiles source code to WASM and serves it in a browser with a UI.

- **Trigger:** `wasmrun run ./project` or just `wasmrun ./project`
- **What it does:** Detects project language → compiles to WASM via plugins → starts a dev session on loopback. What it serves depends on the build:
  - **A module** goes to the console UI on the UI port (`--port`, default 8420), which loads it and lists its exports
  - **A web app** (the project has an `index.html`, or the build emitted JS glue) runs natively in the browser from its own page on the app port (`--app-port`, default the first free port in `8500-8599`, independent of the UI port). The UI port becomes a control center that frames the app and shows its console, requests, build log, module and metrics
  - With `--watch`, a source change rebuilds and pages reload themselves; a page or asset change reloads without building; a failed build keeps the last good one served
- **Key files:**
  - `src/commands/run.rs`: command handler; picks the builder, builds once, hands a rebuild closure to the dev session
  - `src/server/dev.rs`: the dev session: `DevState` shared by both servers and the rebuild loop (artifacts, build generation, log ring, metrics), `run()` which binds the ports and runs the watch loop
  - `src/server/app.rs`: the app port: path resolution (`/pkg/` from the build first, then the project, then the build root), containment, the injected dev client, the generated page for a wasm-bindgen library with none
  - `src/server/handler.rs`: the UI port: template page, `/api/dev`, `/api/logs`, `/api/module-info`, embedded assets
  - `src/server/`: also `api.rs` (module info, version, assets), `utils.rs` (ports, content types), `lifecycle.rs` (one registry entry per running session, which `wasmrun stop` reads)
  - `src/compiler/`: project compilation
  - `src/plugin/`: plugin system (compile plugins)
  - `src/watcher.rs`: file watching; classifies a change as rebuild, reload, or ignored
  - `src/template.rs`: the `console` and `app` templates, compiled into the binary with `include_str!`
  - `ui/src/console/`: the module console, with `imports.ts` stubbing every import so a module from any toolchain instantiates; `ui/src/app/`: the control center. Built into `templates/console/` and `templates/app/` at compile time via `build.rs`
- **Uses plugins:** Yes. Plugins provide compilation (wasmrust, wasmgo, waspy, wasmasc)
- **Uses browser:** Yes. A module is compiled and instantiated in the console, with a stub for every import; a web app runs from its own page. Never through OS mode's browser VM, which is a WASI worker with no DOM
- **Docs:** `docs/docs/server/`

### 2. Exec Mode (`wasmrun exec`)

**Philosophy:** A native WASM interpreter. No browser, no server, no compilation. Just parse and execute a `.wasm` binary directly.

- **Trigger:** `wasmrun exec ./file.wasm [args...]`
- **What it does:** Parses WASM binary → initializes memory → links WASI host functions → interprets bytecode → streams output → returns exit code
- **Flags:** `--call` (an export instead of `_start`), `--dir HOST[:GUEST]` (preopen, contained to `HOST`), `--env K=V`, `--max-memory-mb` and `--max-fuel` (agent mode's `ExecLimits`, via `ExecLimits::from_flags`), `-q` (only the program's output), `--tcplisten` and `--allow-net` (networking). With `--tcplisten`, `WASMHUB_LISTEN_FD` and `WASMHUB_LISTEN_ADDR` are set for the guest
- **Key files:**
  - `src/commands/exec.rs`: command handler
  - `src/runtime/core/`: **the entire WASM interpreter engine**
    - `module.rs`: binary parser
    - `executor.rs`: instruction executor (~7000 lines with its tests, all WASM opcodes). Forward scans for a block's `end`/`else` are memoized per function in `scan_targets`
    - `memory.rs`: linear memory (pages, bounds checking)
    - `values.rs`: value types (i32, i64, f32, f64)
    - `linker.rs`: host function imports/exports linking
    - `native_executor.rs`: high-level API: `execute_wasm_file()`, `execute_wasm_file_with_args()`
    - `control_flow.rs`: control flow analysis
    - `fuzz.rs`: mutation fuzzing of the parser, executor and memory, run by `cargo test` (`just fuzz` for long runs)
    - `spec_suite.rs`: the official spec test suite harness, against a per-file baseline (`just spec-suite`)
  - `src/runtime/wasi/`: WASI syscall implementations (fd_write, args_get, clock, etc.)
    - `mod.rs`: WasiEnv, create_wasi_linker(), `resolve_path()` (preopen containment)
    - `syscalls.rs`: individual syscall host functions
    - `network.rs`: egress policy for outbound sockets
- **Uses plugins:** No
- **Uses browser:** No
- **Docs:** `docs/docs/exec/`

### 3. OS Mode (`wasmrun os`)

**Philosophy:** A browser-based micro-kernel environment. Runs projects inside a WASM VM in the browser with a full development UI (console, filesystem, logs, kernel status). Node.js today; Python waits on a wasmhub runtime.

- **Trigger:** `wasmrun os ./project`
- **What it does:** Detects language → starts HTTP server → serves Preact UI → fetches language runtime WASM from wasmhub → populates virtual FS with project files → boots WASM VM in browser → runs user code sandboxed. With `--watch`, a project change bumps `/api/project/version` and the page restarts a running program
- **Project config:** `wasmrun.toml` `[os]` (`language`, `entry`), `[os.limits]` (`max_memory_mb`, `max_output_mb`, `timeout_secs`; enforced in the page) and `[os.network]` (wasmnet policy), parsed in `src/config/project.rs` with unknown keys refused
- **Key files:**
  - `src/commands/os.rs`: command handler
  - `src/runtime/os_server.rs`: OS mode HTTP server (serves UI, APIs for kernel/fs/logs/tunnel)
  - `src/runtime/multilang_kernel.rs`: multi-language kernel (language detection, staging, process records)
  - `src/runtime/microkernel.rs`: base micro-kernel (process table, workspace VFS). It records and stages; the program itself runs in the browser worker, never on the host
  - `src/runtime/registry.rs`: the languages the kernel can stage
  - `src/runtime/network.rs`: the wasmnet proxy that gives the VM real sockets, on its own port
  - `src/runtime/wasi_fs.rs`: virtual filesystem (in-memory, mount points)
  - `src/runtime/project_files.rs`: project file collection for browser transfer
  - `src/runtime/runtime_cache.rs`: language runtime WASM caching (from wasmhub)
  - `src/runtime/tunnel/`: bore client that publishes the program's port on the internet (`--expose`)
  - `src/runtime/languages/`: language runtime traits and the Node.js implementation
  - `src/logging/`: structured log trail system
  - `ui/src/`: Preact UI source (components, OS panels, WASI shim; builds into `templates/os/` at compile time via `build.rs`)
    - `ui/src/wasi/wasmrun_wasi_impl.js`: the browser WASI shim the VM runs on
    - `ui/src/os/vm.worker.ts`: the VM, in a worker so a guest can block on `Atomics.wait`
    - `ui/src/os/NetBridge.ts`: the page's end of the network: the wasmnet client, the bound port, outbound connects
    - `ui/src/os/netInbox.ts`: the shared ring that carries socket events from the page to the worker
- **Uses plugins:** No (uses its own language detection and wasmhub runtimes)
- **Uses browser:** Yes. Full Preact UI with console, filesystem, kernel panels
- **Docs:** `docs/docs/os/`

### 4. Agent Mode (`wasmrun agent`)

**Philosophy:** A REST API that gives AI agents isolated sandboxes to execute code in. Exec mode's interpreter, wrapped in a long-lived HTTP server with sessions, resource limits, and multi-tenancy. No Docker, no daemon, no browser.

- **Trigger:** `wasmrun agent --port 8430`
- **What it does:** Starts an HTTP server → creates per-session sandboxes (isolated work dir + WASI env) → accepts code (shell command, JS/TS source, multi-file project, or `.wasm`) over REST → runs it on the exec interpreter → returns structured JSON (stdout, stderr, exit code, duration). A session can also run a **long-lived server** (`POST /sessions/:id/serve`), where the host binds a loopback port and hands the guest the listener as an fd
- **Key files:**
  - `src/commands/agent.rs`: command handler
  - `src/agent/server.rs`: HTTP server, routing, exec orchestration
  - `src/agent/session.rs`: session lifecycle, isolation, expiry
  - `src/agent/executor.rs`: source → runtime execution pipeline (JS, TS via swc)
  - `src/agent/esm.rs`: ESM → CommonJS lowering for vendored packages
  - `src/agent/vendor.rs`: npm dependency resolution and vendoring
  - `src/agent/shell.rs`: shell emulation over the session filesystem
  - `src/agent/serve.rs`: long-lived servers in a session (bind, spawn, stop)
  - `src/agent/auth.rs`, `limits.rs`, `metrics.rs`, `tools.rs`, `api.rs`
- **Executes with:** the exec-mode interpreter (`runtime/core/` + `runtime/wasi/`). A `.wasm` behaves identically under `wasmrun exec` and `POST /exec`
- **Uses plugins:** No
- **Uses browser:** No
- **Docs:** `docs/docs/agent/`

### Process Manager (`wasmrun ps`)

**Philosophy:** Keep WASM programs running, the way pm2 or systemd would. Not a fifth execution engine: every program is a `wasmrun exec -q` child process, so it runs on exec mode's interpreter and a crash takes down only that program.

- **Trigger:** `wasmrun ps start [--config processes.toml] [--foreground]`, then `ps list|status|logs|stop|restart <name>`
- **What it does:** Reads a `processes.toml` (`wasm = ` or `script = ` for JavaScript on the cached nodejs runtime) → starts a supervisor (a daemon in its own process group, or in the foreground) → one monitor thread per program spawns it, watches it and restarts it by policy (`always`, `on-failure`, `never`) with doubling backoff, giving up after `max_restarts` within a window. The CLI talks to the supervisor over a loopback control API with a token kept in a 0600 `supervisor.json`
- **Key files:**
  - `src/commands/ps.rs`: the client commands
  - `src/ps/mod.rs`: `run_supervisor()`
  - `src/ps/config.rs`: `processes.toml`, validated before anything starts
  - `src/ps/supervisor.rs`: monitor threads, restart policy, backoff
  - `src/ps/control.rs`: the control API, `ps_home()` (`WASMRUN_PS_HOME` or `~/.wasmrun/ps`)
  - `src/ps/logs.rs`: per-program logs, rotated by size, `tail` and `follow`
  - `src/utils/process.rs`: `is_wasmrun`, `rss_bytes`, `terminate` (shared with `wasmrun stop`)
- **Limits:** the same `ExecLimits` as agent mode, passed through `wasmrun exec --max-memory-mb/--max-fuel`; never a second limits model
- **Uses plugins / browser:** No
- **Docs:** `docs/docs/ps/`

`wasmrun init` (`src/commands/init.rs`, alias `new`) belongs to no single mode: it scaffolds a project for any of them from templates compiled in from `examples/` with `include_str!`. **Changing an example changes its template**, and `init`'s tests check every template still scaffolds.

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
| `src/utils/` | All | Path resolution, commands (`CommandExecutor`), HTTP, system utils |
| `src/utils/process.rs` | Server (`stop`), Process Manager | Is a PID a wasmrun, its RSS, SIGTERM/taskkill |
| `src/config/constants.rs` | Server, OS, Agent | Port defaults (`AGENT_DEFAULT_PORT`, which the server UI steps over), WASM magic |
| `src/runtime/core/` | Exec, Agent, Process Manager | WASM interpreter engine (`module.rs` also used by Verify/Inspect) |
| `src/runtime/wasi/` | Exec, Agent, Process Manager | WASI syscall host functions for interpreter |
| `src/runtime/runtime_cache.rs` | OS, Agent, Process Manager | wasmhub language runtime fetching + caching; prefers the `<latest>-net` nodejs build when the pinned release has one |
| `src/runtime/wasi_fs.rs` | OS | Virtual in-memory filesystem behind `/api/fs` |
| `src/commands/init.rs` | All | Project templates, embedded from `examples/` |
| `src/commands/verify.rs` | Standalone (uses core module parser) | WASM verification |

### Mode Dependency Map

```
Server Mode                     Exec Mode                OS Mode                          Agent Mode                     Process Manager
─────────────                   ─────────                ───────                          ──────────                     ───────────────
src/commands/run.rs             src/commands/exec.rs     src/commands/os.rs               src/commands/agent.rs          src/commands/ps.rs
src/server/*                    src/runtime/core/*       src/runtime/os_server.rs         src/agent/*                    src/ps/*
src/compiler/*                  src/runtime/wasi/*       src/runtime/multilang_kernel.rs                                 src/utils/process.rs
src/plugin/*                                             src/runtime/microkernel.rs       ↓ executes via                 ↓ spawns
src/watcher.rs                                           src/runtime/registry.rs          src/runtime/core/*             wasmrun exec (Exec Mode)
src/template.rs                                          src/runtime/network.rs           src/runtime/wasi/*             src/runtime/runtime_cache.rs
src/utils/process.rs (stop)                              src/runtime/wasi_fs.rs           src/runtime/runtime_cache.rs
ui/src/ (→ templates/app/)                               src/runtime/project_files.rs
ui/src/ (→ templates/console/)                           src/runtime/runtime_cache.rs
                                                         src/runtime/tunnel/*
                                                         src/runtime/languages/*
                                                         src/config/project.rs
                                                         src/watcher.rs (--watch)
                                                         src/logging/*
                                                         ui/src/ (→ templates/os/)
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
- **Update `docs/docs/`** whenever behaviour, CLI usage, or features change. Place docs in the correct mode's section (server/exec/os/agent/ps).
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
│   ├── serve.rs         #   Long-lived servers in a session (bind, spawn, stop)
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
│   ├── ps.rs            #   [Process Manager] client commands for the supervisor
│   ├── init.rs          #   [Shared] project templates, embedded from examples/
│   ├── compile.rs       #   [Server Mode] compile only
│   ├── verify.rs        #   [Shared] WASM binary verification
│   ├── stop.rs          #   [Server Mode] stop every running dev session
│   ├── clean.rs         #   [Shared] clean build artifacts
│   ├── plugin.rs        #   [Server Mode] plugin management
│   ├── module_display.rs #  [Shared] WASM module display formatting
│   └── issue_detector.rs #  [Shared] WASM module issue detection
├── compiler/             # [Server Mode] Project compilation
├── config/               # Constants, plugin config, project config (wasmrun.toml [os])
├── logging/              # [OS Mode] Structured log trail system
├── plugin/               # [Server Mode] Plugin system
├── ps/                   # [Process Manager] supervisor, processes.toml, control API, logs
├── runtime/
│   ├── core/             # [Exec, Agent, PS] ★ WASM interpreter engine
│   │   ├── module.rs     #   Binary parser (shared with verify/inspect)
│   │   ├── executor.rs   #   Instruction executor (~7000 lines with tests)
│   │   ├── memory.rs     #   Linear memory
│   │   ├── values.rs     #   Value types
│   │   ├── linker.rs     #   Host function linking
│   │   ├── native_executor.rs  # High-level exec API
│   │   ├── control_flow.rs     # Control flow analysis
│   │   ├── fuzz.rs       #   Mutation fuzzing (tests)
│   │   ├── spec_suite.rs #   Spec test suite harness (tests)
│   │   └── tests.rs      #   Unit tests
│   ├── wasi/             # [Exec, Agent] WASI syscall implementations
│   │   ├── mod.rs        #   WasiEnv, linker setup, path containment
│   │   ├── network.rs    #   Egress policy
│   │   └── syscalls.rs   #   fd_write, args_get, clock, sockets, etc.
│   ├── os_server.rs      # [OS Mode] HTTP server + API endpoints
│   ├── multilang_kernel.rs # [OS Mode] Multi-language kernel
│   ├── microkernel.rs    # [OS Mode] Base micro-kernel
│   ├── network.rs        # [OS Mode] wasmnet proxy for the browser VM
│   ├── wasi_fs.rs        # [OS Mode] Virtual in-memory filesystem
│   ├── project_files.rs  # [OS Mode] Project file bundling
│   ├── runtime_cache.rs  # [OS, Agent, PS] Wasmhub runtime caching
│   ├── languages/        # [OS Mode] Language runtime traits, Node.js
│   ├── tunnel/           # [OS Mode] Bore client for --expose
│   └── registry.rs       # [OS Mode] Languages the kernel can stage
├── server/               # [Server Mode] Dev session: UI port, app port, rebuild loop
│   ├── dev.rs            #   DevState, run(), watch loop
│   ├── app.rs            #   App port: a web app's page, files and build output
│   ├── handler.rs        #   UI port: console or control center, JSON APIs
│   ├── api.rs            #   Module info, version, embedded assets
│   ├── utils.rs          #   Ports, content types
│   └── lifecycle.rs      #   Per-session registry that `wasmrun stop` reads
├── utils/                # [Shared] Paths, commands, HTTP, processes (process.rs)
├── template.rs           # [Server Mode] Embedded console/app templates
├── ui.rs                 # UI asset embedding
└── watcher.rs            # [Server Mode] File watcher: rebuild, reload, or ignore
```

### Key Architectural Patterns

- **Plugin-based compilation (Server Mode only):** C/C++ (Emscripten), AssemblyScript, Go (TinyGo) and Rust have built-in plugins in `src/plugin/languages/`; Python (`waspy`) and the external `wasmrust`, `wasmgo` and `wasmasc` are crates.io plugins loaded as dynamic libraries, on every platform including Windows. Plugins spawn their toolchains through `CommandExecutor::execute_command`, which resolves Windows `.cmd`/`.bat` shims (`npm`, `npx`, `asc`, `emcc`) through `PATHEXT`; spawn tools through it rather than `Command::new` directly.
- **Self-contained WASM interpreter (Exec Mode):** The `runtime/core/` module is a from-scratch WASM bytecode interpreter, with no dependency on wasmtime/wasmer.
- **WASI syscalls** are implemented as host functions linked via `Linker`, operating on `LinearMemory`. Used by Exec Mode, Agent Mode and the process manager's children.
- **Virtual filesystem (OS Mode):** `wasi_fs.rs` provides an in-memory filesystem with mount points, used by the OS mode kernel and its `/api/fs` endpoints.
- **UI is embedded:** The `build.rs` script compiles the Preact UI (`ui/`) into `templates/`, and every mode compiles its pages from there with `include_str!`/`include_bytes!` (`src/template.rs`, `src/server/api.rs`, `src/runtime/os_server.rs`). Nothing is read from disk at runtime, so an installed binary needs no files beside it.
- **Error handling:** Uses `thiserror` + `anyhow`. Custom `WasmrunError` enum in `src/error.rs` with sub-error types.

---

## Documentation Structure

The docs mirror the four-mode architecture, plus the process manager:

```
docs/docs/
├── server/               # Server Mode documentation
│   ├── index.md          #   Overview
│   ├── features.md       #   Feature list
│   ├── web-apps.md       #   App port, control center, file resolution
│   ├── live-reload.md    #   What --watch rebuilds, reloads, and ignores
│   └── usage/            #   Commands: init, run, compile, verify, inspect, stop, clean
├── exec/                 # Exec Mode documentation
│   ├── index.md          #   Overview
│   ├── features.md       #   Feature list
│   ├── wasi.md           #   WASI support details
│   ├── networking.md     #   Sockets, --tcplisten, --allow-net, tenant policy, Node outbound
│   ├── performance.md    #   The benchmark baseline (`just bench`)
│   └── usage/            #   Running, flags, arguments, function calls
├── os/                   # OS Mode documentation
│   ├── index.md          #   Overview
│   ├── features.md       #   Feature list
│   ├── network-isolation.md   # OS mode network policy ([os.network])
│   ├── port-forwarding.md     # Where a sandbox can actually serve
│   ├── public-tunneling.md
│   └── usage/            #   Running, language selection, server options, wasmrun.toml [os]
├── agent/                # Agent Mode documentation
│   ├── index.md          #   Overview, CLI flags, auth, tenancy
│   └── usage/            #   Sessions, execution, files, environment, observability, serving
├── ps/                   # Process Manager: overview, processes.toml, commands
├── plugins/              # Plugin system (Server Mode)
├── contributing/         # Development guides
├── installation.md
├── intro.md
├── quick-start.md
└── stability.md          # What 1.0 keeps stable (semver surface), upgrading from 0.23
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
just spec-suite     # Fetch the spec test suite and print the per-file table
just bench          # Performance baseline (release; WASMRUN_BENCH_NODEJS for JS rows)
just fuzz 200000 7  # A longer fuzzing run: iterations, seed
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
- **Integration tests** are in `tests/`: `exec_integration_tests.rs`, `agent_lifecycle_tests.rs`, and `ps_integration_tests.rs` (drives the real binary through a crash loop, stop, restart and shutdown, with `WASMRUN_PS_HOME` pointed at a temp dir).
- **Benchmark:** `tests/benchmark.rs` is `#[ignore]`d; `just bench` runs it in release and writes `target/benchmark.json`. Compare against `docs/docs/exec/performance.md` on the same machine.
- **Fuzzing and the spec suite** run inside `cargo test`: `fuzz.rs` with a small default iteration count (`WASMRUN_FUZZ_ITERS`, `WASMRUN_FUZZ_SEED`), `spec_suite.rs` against a per-file baseline when `_tests/testsuite` exists (`just spec-suite-fetch`). A fuzz panic message carries the seed that reproduces it.
- **Test count:** ~900 tests across unit and integration suites.
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
- Follow [Semantic Versioning](https://semver.org/). `docs/docs/stability.md` lists what the 1.0 promise covers; a change to one of those surfaces is a major version, and the page changes with it.
- Update `CHANGELOG.md` with every meaningful change under `[Unreleased]`.

---

## CLI Commands Reference

```sh
# Server Mode
wasmrun <path>                    # Default: compile + serve with dev server
wasmrun run <path>                # Explicit run (same as default)
wasmrun run <path> --watch        # Rebuild on change, reload the page
wasmrun run <path> --app-port 9100  # A web app's own page on 9100, the UI on --port
wasmrun compile <path>            # Compile project to WASM only
wasmrun verify <file.wasm>        # Validate WASM binary structure
wasmrun inspect <file.wasm>       # Analyze WASM binary (exports, imports, sections)
wasmrun stop                      # Stop every running dev session
wasmrun clean <path>              # Clean build artifacts
wasmrun plugin list|install|update|uninstall  # Plugin management

# Exec Mode
wasmrun exec <file.wasm> [args]   # Execute WASM natively with interpreter
wasmrun exec <file.wasm> --call <func> [args]  # Call specific exported function
wasmrun exec --tcplisten 127.0.0.1:8080 <file.wasm>  # Bind a port, pass it in as fd 3
wasmrun exec --allow-net "api.example.com:443" <file.wasm>  # Allow outbound, one rule at a time
wasmrun exec --dir ./data:/data --env KEY=V <file.wasm>  # Preopen a host directory, set a variable
wasmrun exec --max-memory-mb 64 --max-fuel 1000000 -q <file.wasm>  # Limits, and only the program's output

# OS Mode
wasmrun os <path>                 # Run project in browser-based OS environment
wasmrun os <path> --language python  # Force language detection
wasmrun os <path> --watch --port 3000  # With file watching and custom port
wasmrun os <path> --expose          # Publish the program's port through bore.pub
wasmrun os <path> --expose --tunnel-server tunnel.example.com --tunnel-secret S  # Private bore server

# Agent Mode
wasmrun agent                     # Start the REST sandbox API (default port 8430)
wasmrun agent --port 8430 --max-sessions 100  # With explicit port and session cap
wasmrun agent --auth ./tenants.toml  # Enable API-key auth and tenant isolation
wasmrun agent --max-servers 8        # Cap sessions running a server at once
wasmrun agent --allow-net "api.example.com:443"  # Default tenant network policy

# Process Manager
wasmrun ps start                  # Supervise the programs in ./processes.toml (daemon)
wasmrun ps start --config p.toml --foreground  # In the foreground, for systemd and containers
wasmrun ps list|status <name>|logs <name> -f   # Inspect; --json on list and status
wasmrun ps stop [name] | restart <name>        # Stop one, or the supervisor and everything

# Any mode
wasmrun init <name> --template rust|go|c|asc|python|nodejs|wasi  # New project (alias: new)
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
| `src/server/dev.rs` | Server | Dev session: shared state, both ports, watch loop |
| `src/server/app.rs` | Server | App port; where a web app's file paths are resolved and contained |
| `src/commands/exec.rs` | Exec | Exec mode entry point |
| `src/commands/os.rs` | OS | OS mode entry point |
| `src/commands/agent.rs` | Agent | Agent mode entry point |
| `src/agent/server.rs` | Agent | Agent HTTP server, routing, exec orchestration |
| `src/agent/session.rs` | Agent | Session lifecycle and sandbox isolation |
| `src/agent/serve.rs` | Agent | Long-lived servers in a session; where a loopback port is bound |
| `src/runtime/wasi/network.rs` | Exec/Agent | Egress policy: what a sandbox may connect to |
| `src/runtime/core/executor.rs` | Exec | The WASM interpreter (~7000 lines with tests) |
| `src/runtime/core/module.rs` | Exec/Shared | WASM binary parser |
| `src/runtime/wasi/syscalls.rs` | Exec | WASI syscall implementations |
| `src/runtime/wasi/mod.rs` | Exec | WASI environment and linker setup |
| `src/runtime/os_server.rs` | OS | OS mode server (1600+ lines, all API endpoints) |
| `src/runtime/multilang_kernel.rs` | OS | Multi-language kernel |
| `src/config/project.rs` | OS | `wasmrun.toml` `[os]`, `[os.limits]`, `[os.network]` |
| `src/ps/supervisor.rs` | Process Manager | Monitor threads, restart policy, backoff |
| `src/ps/control.rs` | Process Manager | Loopback control API, token, state file |
| `src/commands/init.rs` | All | Templates embedded from `examples/` |
| `src/utils/command.rs` | Server | `CommandExecutor`: how plugins spawn toolchains, incl. Windows shims |
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

### Modifying the UI

1. Edit components in `ui/src/` (`console/` and `app/` are server mode, `os/` is OS mode)
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
- **Two different WASI systems exist:** `src/runtime/wasi/` is for Exec and Agent Mode (host functions linked to the interpreter). OS Mode's VM runs on the browser shim in `ui/src/wasi/wasmrun_wasi_impl.js`, with `src/runtime/wasi_fs.rs` as its host-side virtual filesystem. Don't confuse them, but keep their behaviour the same: a syscall bug in one is usually in the other (append and truncate were missing from both).
- **Three different "server" concepts:** Server Mode's HTTP servers (`src/server/`) serve a module to the console or a web app from its own page, on two loopback ports. OS Mode's HTTP server (`src/runtime/os_server.rs`) serves the OS UI and APIs. Agent Mode's HTTP server (`src/agent/server.rs`) serves the REST sandbox API. They are independent.
- **A WASI syscall change affects two modes.** `src/runtime/wasi/` is reached by `wasmrun exec` and by every agent-mode execution, so test both when touching it.
- **A guest never binds its own port.** WASI Preview 1 has no call that creates a socket, so the *host* binds and passes the listener in as an fd (`--tcplisten` in exec mode, `POST /sessions/:id/serve` in agent mode). `sock_bind` and `sock_listen` are deliberately unimplemented. Don't add them without the `bind_ports` half of the policy meaning something first, or a sandbox gains the ability to choose a port it was never granted.
- **Guest output is capture-only by default.** `fd_write` sends stdout and stderr straight to the host's (uncaptured, so a long-running program streams) only when the `WasiEnv` was built `with_stdio_passthrough()`, which `wasmrun exec` does and agent mode must not: an agent server's stderr is its access log, and a tenant's output does not belong there.
- **Preopen containment fails closed.** `WasiEnv::resolve_path` refuses a `..` that climbs above the directory lexically, before touching the disk, then canonicalizes the deepest part that exists to catch symlinks. Checking only canonical paths fails open when part of the path does not exist yet (`x/../../escape` let `path_create_directory` build its way out). Keep both steps.
- **Bytecode is immutable once an executor is built.** Each function body is an `Arc<[u8]>` shared by every call, and `scan_targets` remembers where a forward scan from a position lands. Anything that rewrote a body at runtime would make those targets wrong.
- **The process manager runs programs as `wasmrun exec` children,** not in-process. That is what isolates a crash and what lets `ps` reuse exec's flags for limits; don't move execution into the supervisor.
- **The listener fd is not always 3.** Preopens are handed out in order, so a session that preopens its work directory first puts the listener on 4. Guests read `WASMHUB_LISTEN_FD`; host code should use the fd returned by `WasiEnv::add_tcp_listener` rather than assuming one.
- **The OS mode tunnel publishes the program's port, never the OS server's.** The OS server's APIs read and write project files, so publishing them would hand the project to anyone with the URL. wasmnet binds the program's port on the page's behalf and has no hook the host can watch, so the page reports the port with `POST /api/tunnel/target`. That endpoint refuses the OS server's and the proxy's ports (both sit inside the default `bind_ports` range of `3000-9999`), any port outside `bind_ports` or with nothing listening, and any `Origin` other than the OS page's own. Keep all four checks.
- **Server mode's app port serves the project directory.** That is why both server-mode ports bind `127.0.0.1` and why `app.rs` refuses dotfiles and any path that leaves the project or build directory, including through a symlink. Keep the loopback bind and the `file_under` containment check; a request for `/.env` or `/../x` must stay a 404.
- **Server mode's dev client posts only to the UI's origin.** The script the app port injects forwards the page's console with `postMessage`, targeting the framing origin it finds in `location.ancestorOrigins` or the referrer, and only if that is the UI port. Never target `'*'`: the app's console output would go to whatever page frames it.
- **Outbound sockets have one ABI in two places.** `sock_open` and `sock_connect` are wasmrun extensions (Preview 1 has neither), implemented in `src/runtime/wasi/syscalls.rs` and again in the browser shim. The host is a string so the policy sees the name, and a refused destination is `EACCES`. Change both together. The shim's `sock_connect` blocks on `Atomics.wait` until the page answers through `netInbox.ts`, so it only works inside the VM worker.
- **`wasmrun stop` finds sessions through a registry, not a PID file.** Each dev session writes `<temp>/wasmrun/servers/<pid>` holding its UI URL and removes it on exit (Ctrl+C or SIGTERM through a `ctrlc` handler). An entry counts only while its process is a `wasmrun` and its port answers; keep both checks, or `stop` can signal a reused PID.
- **`cargo search` is fuzzy.** It lists whatever ranks first, so `SystemUtils::get_latest_crates_version` compares the crate name (treating `-` and `_` as equal) before trusting a version. `plugin install` relies on that to refuse a name crates.io does not have.
- **A sandbox has no network unless configured.** `NetworkAccess::denied()` is the default in both modes; egress comes from `--allow-net` or a tenant's `[tenants.network]` table. Don't "fix" a failing connection by widening the default.

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
- `nodejs-http-api/`: Node.js HTTP API on the built-in `http` module, no npm dependencies (OS Mode, Agent Mode)

Use these for testing. Integration tests in `tests/exec_integration_tests.rs` build and run some of these, and the examples CI builds the server-mode ones on Linux and Windows. Seven of them are also `wasmrun init` templates (`src/commands/init.rs`), so an edit to one ships to every new project.
