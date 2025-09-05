# Changelog

All notable changes to wasmrun will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Updated dependencies

## [0.11.3] - 2025-09-03

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

## [0.10.14] - 2025-08-27

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

## [0.10.2] - 2025-06-30

### Fixed
- Configuration version handling
- **BREAKING**: Renamed from `chakra` to `wasmrun` throughout codebase

### Changed
- Updated logo and visual designs
- Enhanced external plugin system

## [0.10.1] - 2025-06-28

### Fixed
- Code formatting issues
- Minor linting problems

## [0.9.6] - 2025-06-23

### Added
- **NEW FEATURE**: Python support via py2wasm integration
- Python project compilation and execution capabilities

### Fixed
- Linting issues and code quality improvements
- Removed entry file requirements for Python projects

### Changed
- Enhanced plugin system for Python support
- Updated documentation for Python integration

## [0.8.2] - 2025-06-01

### Added
- Enhanced CLI output with better user experience
- Plugin system foundation

### Changed
- Improved command-line interface presentation
- Better error messages and user feedback

## [0.7.2] - 2025-05-21

### Fixed
- Web output rendering issues
- CSS loading problems for web applications
- Project reload functionality

### Changed
- Modularized server architecture
- Improved project structure documentation

## [0.6.4] - 2025-05-09

### Added
- **MAJOR FEATURE**: AOT (Ahead-of-Time) compilation support
- AssemblyScript AOT compilation and runtime
- Go language AOT compilation and runtime  
- C language AOT compilation and runtime
- Rust bindgen support for web applications

### Changed
- Enhanced language detection and compilation pipeline
- Improved WebAssembly runtime capabilities

## [0.4.0] - 2025-05-06

### Added
- **BREAKING**: WASI (WebAssembly System Interface) support
- WebAssembly verification command (`verify`)
- SVG loader support
- Default path handling (current directory)

### Changed
- Updated logo and binary configuration
- Enhanced asset handling for subdirectories
- Improved path information in documentation

## [0.3.0] - 2025-05-05

### Added
- **NEW FEATURE**: Compilation command (`compile`)
- WebAssembly file verification capabilities
- Enhanced asset rendering on web pages

### Fixed
- Asset handling for subdirectories
- Web page rendering issues

## [0.2.0] - 2025-05-03

### Added
- **BREAKING**: Compile command foundation (work in progress)
- Enhanced terminal UI
- Linter integration in publish workflow

### Changed
- Updated project name and branding
- Improved command structure

## [0.1.4] - 2025-04-28

### Added
- Enhanced terminal user interface
- Improved CLI experience

### Fixed
- Lint warnings resolved
- Code quality improvements

## [0.1.3] - 2025-04-27

### Fixed
- Publishing pipeline issues
- GitHub release automation

## [0.1.2] - 2025-04-27

### Fixed
- GitHub release title formatting
- Release automation improvements

## [0.1.1] - 2025-04-27

### Fixed
- Git tagging issues in CI/CD pipeline

## [0.1.0] - 2025-04-27

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

[Unreleased]: https://github.com/anistark/wasmrun/compare/v0.11.3...HEAD
[0.11.3]: https://github.com/anistark/wasmrun/compare/v0.10.14...v0.11.3
[0.10.14]: https://github.com/anistark/wasmrun/compare/v0.10.2...v0.10.14
[0.10.2]: https://github.com/anistark/wasmrun/compare/v0.10.1...v0.10.2
[0.10.1]: https://github.com/anistark/wasmrun/compare/v0.9.6...v0.10.1
[0.9.6]: https://github.com/anistark/wasmrun/compare/v0.8.2...v0.9.6
[0.8.2]: https://github.com/anistark/wasmrun/compare/v0.7.2...v0.8.2
[0.7.2]: https://github.com/anistark/wasmrun/compare/v0.6.4...v0.7.2
[0.6.4]: https://github.com/anistark/wasmrun/compare/v0.4.0...v0.6.4
[0.4.0]: https://github.com/anistark/wasmrun/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/anistark/wasmrun/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/anistark/wasmrun/compare/v0.1.4...v0.2.0
[0.1.4]: https://github.com/anistark/wasmrun/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/anistark/wasmrun/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/anistark/wasmrun/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/anistark/wasmrun/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/anistark/wasmrun/releases/tag/v0.1.0