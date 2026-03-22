# Changelog

All notable changes to wasmrun will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.15.2](https://github.com/anistark/wasmrun/releases/tag/v0.15.2) - 2026-03-22

### Added
- **Console Output in UI**: Live stdout/stderr display from WASM execution
  - New `ConsolePanel` component (`ui/src/components/os/ConsolePanel.tsx`) — displays color-coded stdout (green), stderr (red), and system (blue) output with timestamps
  - WasmRunner integrated into `OSMode.tsx` — Run/Stop buttons, status lifecycle tracking, auto-scroll
  - `StatusIndicator` updated with `stopped` state styling (blue)
  - Console panel activated in sidebar (removed "Coming Soon")
  - Added `ConsoleLine` type to `osTypes.ts`
- **Browser Runtime Loader**: Client-side WASM runtime loader for browser-based execution
  - New `WasmRunner` class (`ui/src/os/WasmRunner.ts`) — fetches runtime + project files, populates WASI virtual FS, instantiates and runs WASM
  - Fetches runtime `.wasm` and project files from server APIs in parallel
  - Decodes base64 project files and writes them to the WASI virtual filesystem with proper directory structure
  - Entry file auto-detection: parses `package.json` main field, falls back to common candidates (`index.js`, `main.py`, etc.)
  - Status lifecycle: `idle` → `loading-runtime` → `loading-files` → `populating-fs` → `starting` → `running` → `stopped`/`error`
  - Callbacks for stdout, stderr, status changes, errors, and exit codes
  - Added ES module exports to WASI shim + TypeScript declarations (`wasmrun_wasi_impl.d.ts`)
- **Project Files API**: Serve project files to browser for WASI virtual filesystem population
  - New `project_files` module (`src/runtime/project_files.rs`) — recursively reads project directory, encodes files as base64
  - `GET /api/project/files` endpoint — returns all project files as a JSON bundle `{ files: { "path": "base64content" }, ... }`
  - `.gitignore` support with glob pattern matching (`*`, `**`, `?`)
  - Default ignore patterns for common directories (`node_modules`, `target`, `.git`, `__pycache__`, etc.) and binary extensions (`.o`, `.so`, `.wasm`, etc.)
  - Size limits: 10MB per file, 50MB total, 5000 file count cap
  - Skipped files reported in response with reasons
  - 20 new unit tests for file collection, ignore patterns, glob matching, base64 encoding, and edge cases
- **Runtime Binary Management**: Fetch, cache, and serve wasmhub language runtimes for browser WASM execution
  - New `runtime_cache` module (`src/runtime/runtime_cache.rs`) — downloads `.wasm` runtimes from wasmhub on first use, caches to `~/.wasmrun/runtimes/`
  - `GET /api/runtime/<language>` endpoint — serves cached runtime binaries with `application/wasm` content type
  - `GET /api/runtimes` endpoint — returns detected language, cache status, and available wasmhub runtimes
  - SHA-256 checksum validation on all downloaded runtimes
  - Language name mapping for wasmhub (nodejs→quickjs, python→rustpython)
  - Cache integrity checks with automatic re-download on corruption
  - 16 new unit tests for cache roundtrip, integrity, language detection, and checksums

- **Exec Agent Mode Implementation Plan**: Design document for AI agent sandbox using exec mode
  - `EXEC_AGENT_IMPLEMENTATION.md` — full roadmap from v0.16 to v0.20
  - REST API design for agent sessions, code execution, file operations, and tool schemas
  - Gap analysis vs NVIDIA OpenShell with positioning as lightweight/secure/embeddable alternative
  - Version-segregated task checklist (187 tasks across 5 releases)

### Changed
- **Documentation Restructure**: Reorganized docs from flat CLI-centric layout to mode-based sections
  - New top-level sections: Server, Exec, OS, Plugins, Contributing
  - Each section follows: Overview → Features → Usage (with per-command sub-pages) → deeper topics
  - Server usage split into individual pages: run, compile, verify, inspect, stop, clean
  - Exec usage split into: running WASM files, function calling, argument passing
  - OS usage split into: running projects, language selection, server options
  - Moved changelog into Contributing section
  - Moved creating-plugins from Development to Plugins section
  - Moved troubleshooting, architecture, debugging into Contributing section
  - Removed empty Web section and flat CLI Reference section
  - Updated all cross-references across 40+ files
  - Tutorials hidden from navbar (route preserved)

### Dependencies
- Added `base64` (0.22) for project file content encoding
- Added `sha2` (0.10) for cryptographic checksum validation
- Moved `ureq` from dev-dependencies to dependencies for runtime fetching

## [0.15.1](https://github.com/anistark/wasmrun/releases/tag/v0.15.1) - 2026-02-22

### Added
- **Public Tunneling with Bore Client**: Expose local WASM apps to the internet via bore.pub (#62)
- **OS Mode Network Isolation**: Per-process network namespace with full WASI socket API (#51)
- **DNS Resolution**: GetAddrInfo syscall with IPv4/IPv6 support (#61)
- **Port Forwarding**: Forward host ports to isolated WASM processes (#52)
- **Rmdir Syscall**: Implemented `Rmdir` dispatching to `wasi_fs.path_remove_directory()`
- **Official Documentation**: Docusaurus site at [wasmrun.readthedocs.io](https://wasmrun.readthedocs.io) (#53, #54)

### Fixed
- **Unified Dual Filesystems**: Removed disconnected in-memory VFS; all FS operations route through `WasiFilesystem`
- **Dev Server Reads Through WASI FS**: Uses `wasi_fs.read_file()` instead of broken host FS reads
- **Embedded OS Templates**: All templates embedded via `include_str!`/`include_bytes!` — works from any CWD
- **OsServer Race Conditions**: Start/restart handlers hold a single write lock (no TOCTOU)
- **sock_open Broken Implementation**: TCP uses `SocketHandle::Placeholder` with deferred creation at bind/connect
- **Port Allocation Overflow**: `calculate_base_port()` uses `u64` arithmetic; `allocate_port()` tracks used ports and skips conflicts on wraparound
- **Dev Server Stop Signal**: Replaced blocking `incoming_requests()` with `recv_timeout()` polling
- **Dead Code Cleanup**: Removed unimplemented syscalls (`Fork`/`Exec`/`Exit`/`Wait`/`Mmap`/`Munmap`/`Input`); consolidated `#[allow(dead_code)]` annotations
- **Docs Build**: Added missing `@docusaurus/plugin-content-pages` dependency

### Security
- **CORS Restricted by Default**: Defaults to `http://127.0.0.1:{port}`; opt-in `--allow-cors` for wildcard
- **Path Traversal Protection**: Syscall interface rejects `..` segments and relative paths
- **Kill Permission Checks**: Only self-kill or parent→child allowed

## [0.15.0](https://github.com/anistark/wasmrun/releases/tag/v0.15.0) - 2025-12-21

### Added
- **Native WASM Execution via `exec` command**: Direct interpreter execution with full argument passing and function selection
  - New `wasmrun exec <WASM_FILE> [ARGS...]` subcommand for executing WASM files
  - Full argument passing to WASM programs via WASI syscalls: `wasmrun exec file.wasm arg1 arg2`
  - Function selection with `-c` / `--call` flag: `wasmrun exec file.wasm -c function_name args`
  - Automatic entry point detection (_start, main, start functions)
  - Full WASI syscall support for file I/O, environment, arguments, and time
  - Direct stdout/stderr output to terminal for CLI tools
- Complete WASM runtime implementation
  - Comprehensive WASM binary parser supporting all standard sections
  - All numeric operations (i32, i64, f32, f64) with proper type handling
  - Memory operations with bounds checking and sign extension
  - Control flow instructions (blocks, loops, branching) with proper stack management
  - Function calls and indirect calls via tables
  - Global variable support
  - WASI syscall implementations (fd_read/write, environ, args, clock, random, proc_exit)

### Changed
- **BREAKING**: Replaced `--native` flag with dedicated `exec` subcommand for better CLI organization
  - `wasmrun file.wasm --native` → `wasmrun exec file.wasm`
  - Now supports passing arguments directly: `wasmrun exec file.wasm arg1 arg2 arg3`
  - Replaced `-f/--function` with `-c/--call` for clearer function selection semantics

### Improved
- Plugin system now gracefully handles invalid/non-existent plugins by creating templates
- Better error messages for WASM execution failures
- Proper metadata handling for plugins without crates.io entries
- Cleaner CLI with dedicated subcommands for different execution modes


## [0.14.0](https://github.com/anistark/wasmrun/releases/tag/v0.14.0) - 2025-11-15

### Added
- **NEW FEATURE**: Real-time logs panel in OS mode with filtering and export
- RPM distribution support (#38)
- GitHub Actions CI/CD workflow (#36)
- APT installation support for wasmrun (#30)

### Fixed
- Templates for UI in installed or global versions (#37)

### Changed
- **BREAKING**: AssemblyScript (asc) moved from built-in to external plugin as wasmasc (#39)

## [0.13.0](https://github.com/anistark/wasmrun/releases/tag/v0.13.0) - 2025-10-12

### Added
- **NEW FEATURE**: OS Mode for system-level interactions (#34)
- Serve flag to make browser to wasmrun server optional (#27)
- Version route to server modules and version display on UI
- Memory allocation and cleaning info to console UI
- Module inspection tab with detailed WASI module information
- Plugin information in module info UI
- Light theme support for UI
- Icons in assets for better visual experience
- Full-width layout for UI
- Examples with Python support via waspy

### Fixed
- Templates for UI in packaging
- Cached temp files cleanup
- Browser hang time issues
- External plugin fallback functionality
- Plugin detection and API improvements
- Module initialization on playground
- Wasmrun server WASM loading
- Waspy integration with dynamic loading FFI
- Go example file issues
- Typing issues across the codebase

### Changed
- **BREAKING**: Removed py2wasm built-in plugin (use waspy instead)
- Refactored UI from templates to separate ui/ directory using Preact
- Refactor wasmrun server UI/UX
- Refactored server modules with new version route
- Cleaned up logs for better output
- UI adjusted for larger screens with responsiveness improvements
- Improved plugin detection system
- Allow dirty release in build process
- Exclude cargo binary from packaging
- Enhanced module inspection functionality

## [0.11.3](https://github.com/anistark/wasmrun/releases/tag/v0.11.3) - 2025-09-03

### Changed
- **BREAKING**: Refactored configuration and constants to dedicated config module
- Externalized templates and improved code organization
- Restructured codebase for better maintainability

### Added
- Examples with web applications for AssemblyScript and Rust Leptos
- Comprehensive test suite
- Debug flag for better troubleshooting
- Playground UI for interactive development
- Custom publish functionality

### Fixed
- Improved error classification and handling
- OS junk file cleanup on clean command

## [0.10.14](https://github.com/anistark/wasmrun/releases/tag/v0.10.14) - 2025-08-27

### Fixed
- External plugin execution and installation issues
- Plugin install and usage problems
- Dynamic loading for wasmrust-specific functionality
- Version display in info command

### Changed  
- Updated working Rust version requirement to 1.88
- Improved external plugin dynamic loading system
- Enhanced plugin install and listing functionality

### Removed
- OS junk files are now properly cleaned up

## [0.10.2](https://github.com/anistark/wasmrun/releases/tag/v0.10.2) - 2025-06-30

### Fixed
- Configuration version handling
- **BREAKING**: Renamed from `chakra` to `wasmrun` throughout codebase

### Changed
- Updated logo and visual designs
- Enhanced external plugin system

## [0.10.1](https://github.com/anistark/wasmrun/releases/tag/v0.10.1) - 2025-06-28

### Fixed
- Code formatting issues
- Minor linting problems

## [0.9.6](https://github.com/anistark/wasmrun/releases/tag/v0.9.6) - 2025-06-23

### Added
- **NEW FEATURE**: Python support via py2wasm integration
- Python project compilation and execution capabilities

### Fixed
- Linting issues and code quality improvements
- Removed entry file requirements for Python projects

### Changed
- Enhanced plugin system for Python support
- Updated documentation for Python integration

## [0.8.2](https://github.com/anistark/wasmrun/releases/tag/v0.8.2) - 2025-06-01

### Added
- Enhanced CLI output with better user experience
- Plugin system foundation

### Changed
- Improved command-line interface presentation
- Better error messages and user feedback

## [0.7.2](https://github.com/anistark/wasmrun/releases/tag/v0.7.2) - 2025-05-21

### Fixed
- Web output rendering issues
- CSS loading problems for web applications
- Project reload functionality

### Changed
- Modularized server architecture
- Improved project structure documentation

## [0.6.4](https://github.com/anistark/wasmrun/releases/tag/v0.6.4) - 2025-05-09

### Added
- **MAJOR FEATURE**: AOT (Ahead-of-Time) compilation support
- AssemblyScript AOT compilation and runtime
- Go language AOT compilation and runtime  
- C language AOT compilation and runtime
- Rust bindgen support for web applications

### Changed
- Enhanced language detection and compilation pipeline
- Improved WebAssembly runtime capabilities

## [0.4.0](https://github.com/anistark/wasmrun/releases/tag/v0.4.0) - 2025-05-06

### Added
- **BREAKING**: WASI (WebAssembly System Interface) support
- WebAssembly verification command (`verify`)
- SVG loader support
- Default path handling (current directory)

### Changed
- Updated logo and binary configuration
- Enhanced asset handling for subdirectories
- Improved path information in documentation

## [0.3.0](https://github.com/anistark/wasmrun/releases/tag/v0.3.0) - 2025-05-05

### Added
- **NEW FEATURE**: Compilation command (`compile`)
- WebAssembly file verification capabilities
- Enhanced asset rendering on web pages

### Fixed
- Asset handling for subdirectories
- Web page rendering issues

## [0.2.0](https://github.com/anistark/wasmrun/releases/tag/v0.2.0) - 2025-05-03

### Added
- **BREAKING**: Compile command foundation (work in progress)
- Enhanced terminal UI
- Linter integration in publish workflow

### Changed
- Updated project name and branding
- Improved command structure

## [0.1.4](https://github.com/anistark/wasmrun/releases/tag/v0.1.4) - 2025-04-28

### Added
- Enhanced terminal user interface
- Improved CLI experience

### Fixed
- Lint warnings resolved
- Code quality improvements

## [0.1.3](https://github.com/anistark/wasmrun/releases/tag/v0.1.3) - 2025-04-27

### Fixed
- Publishing pipeline issues
- GitHub release automation

## [0.1.2](https://github.com/anistark/wasmrun/releases/tag/v0.1.2) - 2025-04-27

### Fixed
- GitHub release title formatting
- Release automation improvements

## [0.1.1](https://github.com/anistark/wasmrun/releases/tag/v0.1.1) - 2025-04-27

### Fixed
- Git tagging issues in CI/CD pipeline

## [0.1.0](https://github.com/anistark/wasmrun/releases/tag/v0.1.0) - 2025-04-27

### Added
- Initial release of wasmrun
- WebAssembly runtime and compilation support
- Basic CLI interface
- Rust example support
- Contribution and setup guides

### Features
- WebAssembly file execution
- Development server with hot reload
- Multi-language compilation pipeline
- Asset serving capabilities

## Project History

**wasmrun** started as a WebAssembly runtime and development tool, evolving from a simple WASM executor to a comprehensive development environment supporting multiple programming languages that compile to WebAssembly.

### Key Milestones
- **v0.1.x**: Initial release and CI/CD setup
- **v0.2.x**: Added compilation capabilities
- **v0.3.x**: Introduced WASI support and verification
- **v0.4.x**: Major AOT compilation features
- **v0.6.x**: Multi-language support (AssemblyScript, Go, C)
- **v0.7.x**: Web application support and server improvements
- **v0.8.x**: Enhanced CLI and plugin system foundation
- **v0.9.x**: Python language support via py2wasm
- **v0.10.x**: Project rename and external plugin system
- **v0.11.x**: Configuration refactoring and examples
- **v0.13.x**: OS Mode, UI refactor with Preact, and waspy integration
- **v0.14.x**: Real-time logs panel, distribution packages (RPM/APT), external plugin migration
- **v0.15.x**: Native WASM execution with exec command, complete runtime implementation

Checkout all [releases](https://github.com/anistark/wasmrun/releases) and [tags](https://github.com/anistark/wasmrun/tags).
