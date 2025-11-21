# Changelog

All notable changes to wasmrun will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

Checkout all [releases](https://github.com/anistark/wasmrun/releases) and [tags](https://github.com/anistark/wasmrun/tags).
