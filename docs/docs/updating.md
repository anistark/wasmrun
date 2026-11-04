---
sidebar_position: 5
title: Updating Wasmrun
description: Keep wasmrun current with `wasmrun update` and `wasmrun upgrade`, through whatever installed it
---

# Updating Wasmrun

Both commands work out how this copy of wasmrun was installed, find the release to move to, and hand over to the tool that owns the install.

| Command | Moves to |
|---|---|
| `wasmrun update` | The newest minor or patch release of the installed major version |
| `wasmrun upgrade` | The newest release, including a new major version |

`wasmrun upgrade` does not need a new major version to exist: when the newest release is a minor or patch release, it installs that, as `wasmrun update` would.

```sh
# Stay on the installed major version
wasmrun update

# Take the newest release there is
wasmrun upgrade

# See what would be installed and how, without changing anything
wasmrun update --check
wasmrun upgrade --check
```

`--check` shows the install method, the new version and the exact command, and changes nothing.

A project directory named `update` or `upgrade` needs a path now, as any directory named after a command does: `wasmrun ./update`.

## Install methods

| Installed with | Recognised by | Runs |
|---|---|---|
| cargo | The binary is in `$CARGO_HOME/bin` (`~/.cargo/bin` by default), and cargo's own record says it came from crates.io | `cargo install wasmrun --version <version> --locked` |
| Homebrew | The binary is at `Cellar/wasmrun/<version>/bin/wasmrun` | `brew upgrade wasmrun` |
| AUR | `pacman` says `wasmrun-bin` owns the binary | `paru -S wasmrun-bin`, or `yay -S wasmrun-bin` |
| `.deb` package | `dpkg-query` says a package owns the binary | Downloads the package from the GitHub release, then `sudo apt install ./<package>` |
| `.rpm` package | `rpm` says a package owns the binary | Downloads the package from the GitHub release, then `sudo rpm -Uvh ./<package>` |

`sudo` is left out when you are already root. Without it, the command prints the download and what to run as root, and exits with status 1.

These tools inherit your terminal, so a password prompt or a confirmation works as usual, and nothing cuts short a `cargo install` that compiles for several minutes. They run from a temporary directory, which also keeps a project's `rust-toolchain.toml` out of `cargo install`.

On Windows a running program can't be replaced, so with a cargo install wasmrun prints the command for you to run.

## A copy wasmrun can't place

A copy built from source, moved by hand or running from a checkout is left as it is, whether or not anything newer exists. The command says so and exits with status 1, with `--check` too. That includes `cargo install --path .`, whose binary sits in `~/.cargo/bin` beside a crates.io install but which `cargo install wasmrun` refuses to replace. To update one of these, pull and rebuild it, or follow the [Installation guide](./installation.md) to reinstall it.

## Major versions

cargo, `.deb` and `.rpm` installs take an exact version, so `wasmrun update` can install the newest `1.x` even after `2.0.0` is out. It finishes by telling you the new major version exists.

Homebrew and the AUR only install the newest version they offer. Once a new major version is out, `wasmrun update` leaves those installs alone rather than cross it, and points you to `wasmrun upgrade`. See [Stability and Upgrading](./stability.md) for what a major version can change.

## Package managers that lag

Homebrew and the AUR can trail a release by hours or days, and the AUR package is maintained by the community. After installing, wasmrun runs `wasmrun --version` from your `PATH` and reports what it finds. When the package manager had nothing newer to offer, you are told `wasmrun` is still on the old version rather than that the update worked. That is not an error, so the exit status stays 0. The same message appears when another copy of wasmrun comes first on `PATH`.

## Where versions come from

Most installs ask the [crates.io index](https://index.crates.io/wa/sm/wasmrun), skipping yanked versions and prereleases.

`.deb` and `.rpm` installs ask the GitHub releases API instead, since that is where the package files are, skipping drafts and prereleases. They also skip any release with no package for your architecture, which is every release until its build finishes. That API allows 60 unauthenticated requests an hour per address, which a shared office or CI address can run out of: set `GITHUB_TOKEN` to raise the limit.

To read from a mirror, set `WASMRUN_UPDATE_INDEX` or `WASMRUN_UPDATE_RELEASES` to the URL of the index file or the release list. Your `GITHUB_TOKEN` is not sent to a mirror.

:::note
These two can only be set in the environment. `wasmrun.toml` is read from the current directory, and a cloned project should not be able to choose what gets installed with `sudo`.
:::
