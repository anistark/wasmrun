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
build: format lint test
    cargo build --release

# Clean the project
clean:
    cargo clean
    rm -rf examples || true
    rm -rf example.* || true
    find . -name ".DS_Store" -type f -delete || true

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

# Run clippy lints
lint-fix:
    cargo clippy --fix

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

# Create a pre-release tag with suffix (rc, alpha, beta, etc.)
publish-rc: (publish-tag "rc")
publish-alpha: (publish-tag "alpha")
publish-beta: (publish-tag "beta")
publish-dev: (publish-tag "dev")

# Generic publish with custom tag suffix
publish-tag TAG:
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

    # Build the project first
    echo "Building project..."
    cargo build --release

    # Create version with tag suffix
    VERSION_WITH_TAG="{{version}}-{{TAG}}"
    TAG_NAME="v$VERSION_WITH_TAG"

    echo "Creating pre-release: $TAG_NAME"

    # Check if tag already exists
    if git rev-parse "$TAG_NAME" >/dev/null 2>&1; then
        echo "Warning: Tag $TAG_NAME already exists"
        read -p "Do you want to delete and recreate it? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git tag -d "$TAG_NAME" || true
            git push --delete origin "$TAG_NAME" || true
        else
            echo "Cancelled"
            exit 1
        fi
    fi

    # Create annotated tag
    echo "Creating tag $TAG_NAME..."
    git tag -a "$TAG_NAME" -m "Pre-release $TAG_NAME"

    # Push the tag to remote
    echo "Pushing tag $TAG_NAME to remote..."
    git push origin "$TAG_NAME"

    # Create GitHub pre-release
    echo "Creating GitHub pre-release..."
    gh release create "$TAG_NAME" \
        --target "$(git rev-parse HEAD)" \
        --title "Wasmrun $VERSION_WITH_TAG" \
        --notes "Pre-release version $VERSION_WITH_TAG

    This is a pre-release version for testing and feedback.

    **Installation:**
    \`\`\`bash
    # Install from source with this specific tag
    cargo install --git https://github.com/{{repo}} --tag $TAG_NAME

    # Or download from releases
    # See assets below
    \`\`\`

    **Changes since last release:**
    $(git log --oneline $(git describe --tags --abbrev=0 HEAD^)..HEAD | head -10)
    " \
        --prerelease \
        "./target/release/wasmrun"

    echo "✓ Pre-release $TAG_NAME created successfully!"
    echo "View it at: https://github.com/{{repo}}/releases/tag/$TAG_NAME"

# List all available publish commands
publish-help:
    @echo "Available publish commands:"
    @echo "  just publish       - Full release to GitHub and crates.io"
    @echo "  just publish-rc    - Release candidate (v{{version}}-rc)"
    @echo "  just publish-alpha - Alpha release (v{{version}}-alpha)"
    @echo "  just publish-beta  - Beta release (v{{version}}-beta)"
    @echo "  just publish-dev   - Development release (v{{version}}-dev)"
    @echo "  just publish-tag X - Custom tag release (v{{version}}-X)"
    @echo ""
    @echo "Current version: {{version}}"
