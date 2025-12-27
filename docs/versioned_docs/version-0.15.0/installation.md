---
sidebar_position: 2
---

# Installation

Install Wasmrun using one of the following methods based on your operating system and preferences.

## Cargo (Recommended)

The easiest way to install Wasmrun is via Cargo, Rust's package manager:

```bash
cargo install wasmrun
```

This installs the latest stable version from [crates.io](https://crates.io/crates/wasmrun).

**Requirements:**
- Rust 1.70 or higher
- Cargo (comes with Rust)

If you don't have Rust installed, get it from [rustup.rs](https://rustup.rs/).

## Prebuilt Packages

### DEB Package (Debian/Ubuntu/Pop!_OS)

For Debian-based Linux distributions, download and install the DEB package:

1. **Download the latest `.deb` file** from [GitHub Releases](https://github.com/anistark/wasmrun/releases)

2. **Install the package:**

```bash
# Install the downloaded DEB package
sudo apt install ./wasmrun_*.deb

# If there are dependency issues, fix them
sudo apt install -f
```

**Supported distributions:**
- Ubuntu 20.04+
- Debian 11+
- Pop!_OS 20.04+
- Linux Mint 20+
- Other Debian-based distributions

### RPM Package (Fedora/RHEL/CentOS)

For Red Hat-based Linux distributions, download and install the RPM package:

1. **Download the latest `.rpm` file** from [GitHub Releases](https://github.com/anistark/wasmrun/releases)

2. **Install the package:**

```bash
# Install using rpm
sudo rpm -i wasmrun-*.rpm

# Or using dnf (Fedora/RHEL 8+)
sudo dnf install ./wasmrun-*.rpm

# Or using yum (older versions)
sudo yum install ./wasmrun-*.rpm
```

**Supported distributions:**
- Fedora 35+
- RHEL 8+
- CentOS Stream 8+
- Rocky Linux 8+
- AlmaLinux 8+

## From Source

Build and install Wasmrun from source for the latest development version or if prebuilt packages aren't available for your platform:

```bash
# Clone the repository
git clone https://github.com/anistark/wasmrun.git
cd wasmrun

# Install from source
cargo install --path .
```

This will compile Wasmrun and install it to `~/.cargo/bin/wasmrun`.

**Build requirements:**
- Rust 1.70 or higher
- Git

## Release Tracking

Stay updated with the latest releases:

- **GitHub Releases:** [github.com/anistark/wasmrun/releases](https://github.com/anistark/wasmrun/releases)
- **Release Feed (Atom):** [github.com/anistark/wasmrun/releases.atom](https://github.com/anistark/wasmrun/releases.atom)
- **Crates.io:** [crates.io/crates/wasmrun](https://crates.io/crates/wasmrun)

## Verify Installation

After installation, verify that Wasmrun is working correctly:

```bash
# Check version
wasmrun --version

# Should output something like: wasmrun 0.15.0

# View available commands
wasmrun --help
```

## Installing Language Plugins

Wasmrun supports multiple languages through plugins. After installing Wasmrun, you can install language support:

```bash
# List available plugins
wasmrun plugin list

# Install Rust support
wasmrun plugin install wasmrust

# Install Go support
wasmrun plugin install wasmgo

# Install Python support
wasmrun plugin install waspy

# Install AssemblyScript support
wasmrun plugin install wasmasc
```

**Note:** C/C++ support is built-in and doesn't require a separate plugin.

## Updating Wasmrun

### Cargo Installation

If you installed via Cargo, update with:

```bash
cargo install wasmrun --force
```

### Package Manager Installation

For DEB/RPM packages:
1. Download the latest package from GitHub Releases
2. Install it (will upgrade existing installation)

### From Source

```bash
cd wasmrun
git pull
cargo install --path . --force
```

## Common Installation Issues

### Command Not Found

If `wasmrun` is not found after installation:

1. **Check cargo bin directory is in PATH:**
```bash
echo $PATH | grep cargo
```

2. **Add to PATH** (if missing):
```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell
source ~/.bashrc  # or source ~/.zshrc
```

### Permission Denied (Linux/macOS)

If you get permission errors:

```bash
# Ensure the binary is executable
chmod +x ~/.cargo/bin/wasmrun
```

### Plugin Installation Fails

If plugin installation fails:

1. **Check internet connection** - Plugins are downloaded from crates.io
2. **Verify disk space** - Plugins are installed to `~/.wasmrun/plugins/`
3. **Check permissions** - Ensure you can write to `~/.wasmrun/`

```bash
# Create directory if it doesn't exist
mkdir -p ~/.wasmrun/plugins
```

## Uninstallation

### Cargo Installation

```bash
cargo uninstall wasmrun

# Also remove plugins and cache (optional)
rm -rf ~/.wasmrun
```

### DEB Package

```bash
sudo apt remove wasmrun
```

### RPM Package

```bash
sudo rpm -e wasmrun
# or
sudo dnf remove wasmrun
```

## Next Steps

Now that Wasmrun is installed, continue to the [Quick Start](./quick-start.md) guide to create your first WebAssembly project, or explore the [Language Guides](./plugins/languages/rust.md) to learn about using Wasmrun with your preferred programming language.
