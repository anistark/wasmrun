# Wasmrun project justfile
# Install just: https://github.com/casey/just

# Get version from Cargo.toml
version := `grep -m 1 'version = ' Cargo.toml | cut -d '"' -f 2`

# Repository information
repo := `if git remote -v >/dev/null 2>&1; then git remote get-url origin | sed -E 's/.*github.com[:/]([^/]+)\/([^/.]+).*/\1\/\2/'; else echo "anistark/wasmrun"; fi`

# Default recipe to display help information
default:
    @just --list
    @echo "\nCurrent version: {{version}}"

# Build the project in debug mode
build:
    cargo build --release

# Clean the project
clean:
    cargo clean
    rm -rf examples || true
    rm -rf example.* || true

# Run with a test WASM file (replace with your test file path)
run WASM_FILE="./examples/simple.wasm":
    cargo run -- --path {{WASM_FILE}}

# Run with a custom port
run-port WASM_FILE="./examples/simple.wasm" PORT="3000":
    cargo run -- --path {{WASM_FILE}} --port {{PORT}}

# Stop any running Wasmrun server
stop:
    cargo run -- stop

# Run tests
test:
    cargo test

# Check code formatting
check-format:
    cargo fmt -- --check

# Format code
format:
    cargo fmt

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Build documentation
docs:
    cargo doc --no-deps --open

# TODO: Fix Increment version (type can be major, minor, or patch)
# bump-version TYPE="patch":
#     cargo install cargo-edit
#     cargo set-version --bump {{TYPE}}

# Prepare for publishing (format, lint, test)
prepare-publish: format lint test build
    @echo "✓ Project is ready for publishing"

# Publish to crates.io (requires cargo login)
publish-crates: prepare-publish
    @echo "Publishing version {{version}} to crates.io..."
    cargo publish

# Generate an example WASM file using Emscripten
example-wasm-emcc:
    mkdir -p examples
    echo 'int main() { return 42; }' > examples/simple.c
    emcc -O2 examples/simple.c -o examples/simple.wasm
    @echo "✓ Created examples/simple.wasm"

# Generate an example WASM file using Rust
example-wasm-rust:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Store original directory to return to later
    ORIGINAL_DIR=$(pwd)
    
    # Create examples directory if it doesn't exist
    mkdir -p examples
    
    # Create a temporary Rust project
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Initialize a new Rust project
    cargo init --bin example-wasm
    cd example-wasm
    
    # Add wasm target
    rustup target add wasm32-unknown-unknown
    
    # Create a simple Rust file that will compile to WASM
    cat > src/main.rs << 'EOF'
    // Export functions to be called from JavaScript
    #[no_mangle]
    pub extern "C" fn add(a: i32, b: i32) -> i32 {
        a + b
    }
    
    #[no_mangle]
    pub extern "C" fn get_answer() -> i32 {
        42
    }
    
    // Standard main function with proper return type
    fn main() {
        println!("Hello from Rust!");
    }
    EOF
    
    # Compile to WebAssembly
    cargo build --target wasm32-unknown-unknown --release
    
    # Find the exact wasm file name in the target directory
    WASM_FILE=$(find target/wasm32-unknown-unknown/release -name "*.wasm" | head -n 1)
    
    if [ -n "$WASM_FILE" ]; then
        # Use absolute paths for copying
        cp "$WASM_FILE" "$ORIGINAL_DIR/examples/rust_example.wasm"
        echo "✓ Created examples/rust_example.wasm"
    else
        echo "❌ Error: No WASM file found in target directory!"
        ls -la target/wasm32-unknown-unknown/release/
        exit 1
    fi
    
    # Return to original directory and clean up
    cd "$ORIGINAL_DIR"
    rm -rf "$TEMP_DIR"

# Generate example WASM files (tries Rust first, falls back to emcc if available)
example-wasm:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Try Rust method first
    if command -v rustc &> /dev/null && rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
        @just example-wasm-rust
    # Fall back to emcc if available
    elif command -v emcc &> /dev/null; then
        @just example-wasm-emcc
    else
        echo "❌ Error: Neither Rust wasm32 target nor emcc found."
        echo "Please install one of the following:"
        echo "  - Rust with wasm target: rustup target add wasm32-unknown-unknown"
        echo "  - Emscripten: https://emscripten.org/docs/getting_started/downloads.html"
        exit 1
    fi

# Check if you're logged in to crates.io
check-crates-login:
    @if [ -f ~/.cargo/credentials ]; then \
        echo "Credentials found. You appear to be logged in to crates.io"; \
        echo "Ready to publish wasmrun v{{version}}"; \
    else \
        echo "No credentials found. Run 'cargo login' with your crates.io token"; \
    fi

# Install local binary
install:
    cargo install --path .

# Create a new release tag
tag-release:
    git tag v{{version}}
    @echo "Created tag v{{version}}"
    echo "Pushing tag v{{version}} to remote..."
    git push origin "v{{version}}"

# Create GitHub release
gh-release:
    #!/usr/bin/env bash
    set -euo pipefail

    # Check if gh CLI is installed
    if ! command -v gh &> /dev/null; then
        echo "Error: GitHub CLI not installed. Please install it from https://cli.github.com/"
        exit 1
    fi

    # Check if user is logged in to GitHub
    if ! gh auth status &> /dev/null; then
        echo "Error: Not logged in to GitHub. Please run 'gh auth login'"
        exit 1
    fi

    # Create a tag if it doesn't exist
    if ! git rev-parse "v{{version}}" >/dev/null 2>&1; then
        git tag -a "v{{version}}"
        echo "✓ Created tag v{{version}}"
    else
        echo "✓ Tag v{{version}} already exists"
    fi

    # Push the tag to remote
    echo "Pushing tag v{{version}} to remote..."
    git push origin "v{{version}}"

    # Create GitHub release with auto-generated release notes
    gh release create "v{{version}}" \
        "./target/release/wasmrun"

    echo "✓ GitHub release v{{version}} created successfully!"
    echo "View it at: https://github.com/{{repo}}/releases/tag/v{{version}}"

# Release to both GitHub and crates.io
publish: build publish-crates gh-release
    @echo "✓ Released v{{version}} to GitHub and crates.io"
