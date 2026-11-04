//! [Shared] `wasmrun update` and `wasmrun upgrade`: move this install to a newer
//! release through whatever installed it.
//!
//! `update` stays on the installed major version, `upgrade` takes the newest
//! release there is. cargo, `.deb` and `.rpm` installs take an exact version.
//! Homebrew and the AUR install whatever they offer at the time, so `update`
//! holds back when that could cross a major version, and what actually landed
//! is read back from `wasmrun --version`.
//!
//! The release sources can be overridden by environment variables only, never
//! by `wasmrun.toml`: that file comes from the current directory, and a cloned
//! project must not choose what gets installed with `sudo`.

use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::debug_println;
use crate::error::{Result, WasmrunError};
use crate::ui::{print_info, print_status, print_success};
use crate::utils::{http, SemVer};

const CRATE_INDEX: &str = "https://index.crates.io/wa/sm/wasmrun";
const CRATE_INDEX_ENV: &str = "WASMRUN_UPDATE_INDEX";
const GITHUB_RELEASES: &str = "https://api.github.com/repos/anistark/wasmrun/releases?per_page=100";
const GITHUB_RELEASES_ENV: &str = "WASMRUN_UPDATE_RELEASES";
const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";
const USER_AGENT: &str = concat!(
    "wasmrun/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/anistark/wasmrun)"
);

/// How `cargo install` records a package it took from crates.io.
const CRATES_IO_SOURCE: &str = "(registry+https://github.com/rust-lang/crates.io-index)";
const AUR_PACKAGE: &str = "wasmrun-bin";
const INSTALL_DOCS: &str = "https://wasmrun.readthedocs.io/en/latest/docs/installation";

/// How far a run may move the installed version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateScope {
    /// `wasmrun update`: minor and patch releases of the installed major version.
    SameMajor,
    /// `wasmrun upgrade`: the newest release, whatever its major version.
    Latest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstallMethod {
    Cargo,
    Homebrew,
    Aur,
    Deb,
    Rpm,
}

impl fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InstallMethod::Cargo => "cargo",
            InstallMethod::Homebrew => "Homebrew",
            InstallMethod::Aur => "the AUR",
            InstallMethod::Deb => "a .deb package",
            InstallMethod::Rpm => "an .rpm package",
        })
    }
}

impl InstallMethod {
    fn takes_exact_version(self) -> bool {
        !matches!(self, InstallMethod::Homebrew | InstallMethod::Aur)
    }

    fn package_extension(self) -> Option<&'static str> {
        match self {
            InstallMethod::Deb => Some("deb"),
            InstallMethod::Rpm => Some("rpm"),
            _ => None,
        }
    }
}

/// `<prefix>/Cellar/wasmrun/<version>/bin/wasmrun`, on any Homebrew prefix.
fn in_homebrew_cellar(exe: &Path) -> bool {
    let tail: Vec<_> = exe
        .components()
        .rev()
        .take(5)
        .map(|part| part.as_os_str())
        .collect();
    matches!(
        tail.as_slice(),
        [name, bin, _version, formula, cellar]
            if *name == "wasmrun" && *bin == "bin" && *formula == "wasmrun" && *cellar == "Cellar"
    )
}

/// Where `cargo install` puts things: `$CARGO_HOME`, or `~/.cargo` without it.
fn cargo_home() -> Option<PathBuf> {
    let home = env::var_os("CARGO_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")))?;
    Some(fs::canonicalize(&home).unwrap_or(home))
}

#[derive(Deserialize)]
struct CargoInstalls {
    installs: HashMap<String, IgnoredAny>,
}

/// Does cargo's own record say wasmrun came from crates.io? The binary from a
/// `cargo install --path .` sits in the same directory, and `cargo install
/// wasmrun` refuses to replace it.
fn recorded_from_crates_io(record: &str) -> bool {
    serde_json::from_str::<CargoInstalls>(record).is_ok_and(|record| {
        record
            .installs
            .keys()
            .any(|key| key.starts_with("wasmrun ") && key.ends_with(CRATES_IO_SOURCE))
    })
}

fn detect_install_method(
    exe: &Path,
    cargo_home: Option<&Path>,
    system_owner: impl FnOnce() -> Option<InstallMethod>,
) -> Option<InstallMethod> {
    if in_homebrew_cellar(exe) {
        return Some(InstallMethod::Homebrew);
    }
    if let Some(home) = cargo_home.filter(|home| exe.parent() == Some(&home.join("bin"))) {
        return fs::read_to_string(home.join(".crates2.json"))
            .is_ok_and(|record| recorded_from_crates_io(&record))
            .then_some(InstallMethod::Cargo);
    }
    system_owner()
}

/// Ask the system package managers whether one of them owns `exe`.
fn system_owner(exe: &Path) -> Option<InstallMethod> {
    let owner = |tool: &str, query: &str| {
        let output = Command::new(tool).arg(query).arg(exe).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    if owner("pacman", "-Qqo").is_some_and(|package| package == AUR_PACKAGE) {
        Some(InstallMethod::Aur)
    } else if owner("dpkg-query", "-S").is_some() {
        Some(InstallMethod::Deb)
    } else if owner("rpm", "-qf").is_some() {
        Some(InstallMethod::Rpm)
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Package {
    name: String,
    url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Release {
    version: SemVer,
    package: Option<Package>,
}

#[derive(Deserialize)]
struct IndexEntry {
    vers: String,
    #[serde(default)]
    yanked: bool,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn parse_crate_index(body: &str) -> Vec<Release> {
    body.lines()
        .filter_map(|line| serde_json::from_str::<IndexEntry>(line).ok())
        .filter(|entry| !entry.yanked)
        .filter_map(|entry| SemVer::parse(&entry.vers).ok())
        .map(|version| Release {
            version,
            package: None,
        })
        .collect()
}

/// A `.deb` or `.rpm` install skips releases with no package for this
/// architecture, which is every release until its build job finishes.
fn parse_github_releases(body: &str, extension: &str) -> Result<Vec<Release>> {
    let releases: Vec<GithubRelease> = serde_json::from_str(body)
        .map_err(|e| WasmrunError::self_update(format!("Could not read the release list: {e}")))?;
    Ok(releases
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|release| {
            let version = SemVer::parse(&release.tag_name).ok()?;
            let name = format!("wasmrun-v{version}-{}.{extension}", env::consts::ARCH);
            let asset = release.assets.into_iter().find(|a| a.name == name)?;
            Some(Release {
                version,
                package: Some(Package {
                    name,
                    url: asset.browser_download_url,
                }),
            })
        })
        .collect())
}

fn get(url: &str, token: Option<&str>) -> std::result::Result<String, ureq::Error> {
    let mut request = http::agent().get(url).header("User-Agent", USER_AGENT);
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    request.call()?.into_body().read_to_string()
}

fn env_override(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// The release list to read and the token to read it with. A token is for
/// GitHub, so a mirror never receives it.
fn github_source(mirror: Option<String>, token: Option<String>) -> (String, Option<String>) {
    match mirror {
        Some(url) => (url, None),
        None => (GITHUB_RELEASES.to_string(), token),
    }
}

fn unreadable(url: &str, error: ureq::Error) -> WasmrunError {
    WasmrunError::self_update(format!(
        "Could not read the published releases from {url}: {error}"
    ))
}

/// Only `.deb` and `.rpm` installs ask GitHub, for the package files. It allows
/// 60 requests an hour per address without a token, and the crate index has no
/// such limit, so anything that only needs a version number asks the index.
fn fetch_releases(method: InstallMethod) -> Result<Vec<Release>> {
    let Some(extension) = method.package_extension() else {
        let url = env_override(CRATE_INDEX_ENV).unwrap_or_else(|| CRATE_INDEX.to_string());
        debug_println!("Reading versions from {url}");
        let releases = parse_crate_index(&get(&url, None).map_err(|e| unreadable(&url, e))?);
        if releases.is_empty() {
            return Err(WasmrunError::self_update(format!(
                "{url} listed no versions of wasmrun"
            )));
        }
        return Ok(releases);
    };

    let (url, token) = github_source(
        env_override(GITHUB_RELEASES_ENV),
        env_override(GITHUB_TOKEN_ENV),
    );
    debug_println!("Reading releases from {url}");
    let body = match get(&url, token.as_deref()) {
        Ok(body) => body,
        Err(ureq::Error::StatusCode(403 | 429)) => {
            return Err(WasmrunError::self_update(format!(
                "GitHub is limiting requests from this address. Set {GITHUB_TOKEN_ENV} to raise \
                 the limit, or try again in an hour"
            )));
        }
        Err(e) => return Err(unreadable(&url, e)),
    };
    let releases = parse_github_releases(&body, extension)?;
    if releases.is_empty() {
        return Err(WasmrunError::self_update(format!(
            "No release has a .{extension} package for {}",
            env::consts::ARCH
        )));
    }
    Ok(releases)
}

fn newest<'a>(
    releases: &'a [Release],
    current: &SemVer,
    allowed: impl Fn(&SemVer) -> bool,
) -> Option<&'a Release> {
    releases
        .iter()
        .filter(|release| {
            let version = &release.version;
            version.pre.is_none() && version > current && allowed(version)
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

#[derive(Debug, PartialEq, Eq)]
enum Decision<'a> {
    UpToDate,
    /// Only `wasmrun upgrade` can move this install: the newest release is a new
    /// major version, and either nothing newer exists on this one or the install
    /// method cannot be held to it.
    NeedsUpgrade(&'a SemVer),
    Install {
        target: &'a Release,
        newer_major: Option<&'a SemVer>,
    },
}

fn decide<'a>(
    releases: &'a [Release],
    current: &SemVer,
    scope: UpdateScope,
    method: InstallMethod,
) -> Decision<'a> {
    let Some(latest) = newest(releases, current, |_| true) else {
        return Decision::UpToDate;
    };
    if scope == UpdateScope::Latest || latest.version.major == current.major {
        return Decision::Install {
            target: latest,
            newer_major: None,
        };
    }
    let same_major = newest(releases, current, |version| version.major == current.major);
    match same_major {
        Some(target) if method.takes_exact_version() => Decision::Install {
            target,
            newer_major: Some(&latest.version),
        },
        _ => Decision::NeedsUpgrade(&latest.version),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Plan {
    download: Option<Package>,
    command: Vec<String>,
}

impl Plan {
    fn run_only(command: &[&str]) -> Self {
        Plan {
            download: None,
            command: argv(command),
        }
    }

    fn describe(&self) {
        if let Some(package) = &self.download {
            println!("Would download: {}", package.url);
        }
        println!("Would run: {}", self.command.join(" "));
    }

    /// The command inherits the terminal and has no deadline: `cargo install`
    /// compiles for minutes and `sudo` asks for a password. It runs from a
    /// temporary directory, which holds the download and keeps a project's
    /// `rust-toolchain.toml` or `.cargo/config.toml` out of `cargo install`.
    fn run(&self) -> Result<()> {
        let workdir = tempfile::tempdir().map_err(|e| {
            WasmrunError::self_update(format!("Could not create a temporary directory: {e}"))
        })?;
        if let Some(package) = &self.download {
            print_status(&format!("Downloading {}", package.url));
            download(package, workdir.path()).map_err(|e| {
                WasmrunError::self_update(format!("Could not download {}: {e}", package.name))
            })?;
        }
        let line = self.command.join(" ");
        print_status(&format!("Running: {line}"));
        let status = Command::new(&self.command[0])
            .args(&self.command[1..])
            .current_dir(workdir.path())
            .status()
            .map_err(|e| WasmrunError::self_update(format!("Could not run `{line}`: {e}")))?;
        if !status.success() {
            return Err(WasmrunError::self_update(format!(
                "`{line}` failed ({status})"
            )));
        }
        Ok(())
    }
}

fn download(package: &Package, dir: &Path) -> std::result::Result<(), String> {
    let response = http::agent()
        .get(&package.url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    let mut file = fs::File::create(dir.join(&package.name)).map_err(|e| e.to_string())?;
    std::io::copy(&mut response.into_body().into_reader(), &mut file).map_err(|e| e.to_string())?;
    Ok(())
}

fn argv(command: &[&str]) -> Vec<String> {
    command.iter().map(|arg| arg.to_string()).collect()
}

fn on_path(tool: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| dir.join(tool).is_file()))
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|output| output.stdout.trim_ascii() == b"0")
}

/// `command` as the current user can run it with root's rights, or `None` when
/// they cannot.
fn as_root(command: &[&str]) -> Option<Vec<String>> {
    if is_root() {
        Some(argv(command))
    } else if on_path("sudo") {
        let mut elevated = argv(&["sudo"]);
        elevated.extend(argv(command));
        Some(elevated)
    } else {
        None
    }
}

/// What to run for this install, or instructions for the user when wasmrun
/// cannot do it itself.
fn plan_for(method: InstallMethod, target: &Release) -> std::result::Result<Plan, String> {
    match method {
        InstallMethod::Cargo => {
            let version = target.version.to_string();
            let command = [
                "cargo",
                "install",
                "wasmrun",
                "--version",
                &version,
                "--locked",
            ];
            if cfg!(windows) {
                return Err(format!(
                    "Windows can't replace a program while it runs, so run this yourself: {}",
                    command.join(" ")
                ));
            }
            Ok(Plan::run_only(&command))
        }
        InstallMethod::Homebrew => Ok(Plan::run_only(&["brew", "upgrade", "wasmrun"])),
        InstallMethod::Aur => ["paru", "yay"]
            .into_iter()
            .find(|helper| on_path(helper))
            .map(|helper| Plan::run_only(&[helper, "-S", AUR_PACKAGE]))
            .ok_or_else(|| {
                format!(
                    "No AUR helper (paru or yay) found. Rebuild the package from \
                     https://aur.archlinux.org/packages/{AUR_PACKAGE}"
                )
            }),
        InstallMethod::Deb | InstallMethod::Rpm => {
            let package = target
                .package
                .clone()
                .ok_or_else(|| format!("v{} has no package to install", target.version))?;
            let local_file = format!("./{}", package.name);
            let install = match method {
                InstallMethod::Deb => ["apt", "install", &local_file],
                _ => ["rpm", "-Uvh", &local_file],
            };
            let command = as_root(&install).ok_or_else(|| {
                format!(
                    "Download {} and run this as root: {}",
                    package.url,
                    install.join(" ")
                )
            })?;
            Ok(Plan {
                download: Some(package),
                command,
            })
        }
    }
}

fn version_in_banner(banner: &str) -> Option<SemVer> {
    let pattern = Regex::new(r"wasmrun v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)").ok()?;
    SemVer::parse(pattern.captures(banner)?.get(1)?.as_str()).ok()
}

/// What typing `wasmrun` gives now. The path this process started from can be
/// gone by this point: `brew upgrade` removes the old Cellar directory.
fn version_on_path() -> Option<SemVer> {
    let output = Command::new("wasmrun").arg("--version").output().ok()?;
    version_in_banner(&String::from_utf8_lossy(&output.stdout))
}

/// A package manager that had nothing newer to offer exits 0, so the only proof
/// of the update is the version it left behind. That case is not an error.
fn report_outcome(target: &SemVer, current: &SemVer, method: InstallMethod) {
    let why = if method.takes_exact_version() {
        "Another copy of wasmrun may come first on PATH.".to_string()
    } else {
        format!("v{target} may not have reached {method} yet, or another copy comes first on PATH.")
    };
    match version_on_path() {
        Some(version) if version >= *target => {
            print_success("Wasmrun Updated", &format!("wasmrun is now v{version}"));
        }
        Some(version) if version == *current => {
            println!("⚠️  `wasmrun` is still v{version}. {why}");
        }
        Some(version) => println!("⚠️  `wasmrun` is v{version}, not v{target}. {why}"),
        None => println!(
            "⚠️  The install finished, but `wasmrun --version` could not confirm it. Is `wasmrun` \
             on PATH?"
        ),
    }
}

fn point_to_upgrade(latest: &SemVer) {
    println!("v{latest} is a new major version. Move to it with `wasmrun upgrade`.");
}

fn install(target: &Release, current: &SemVer, method: InstallMethod, check: bool) -> Result<()> {
    println!("New version: v{}", target.version);
    let plan = plan_for(method, target);
    if check {
        match plan {
            Ok(plan) => plan.describe(),
            Err(instructions) => println!("{instructions}"),
        }
        if !method.takes_exact_version() {
            println!(
                "This install takes the newest version {method} offers, which can trail a release."
            );
        }
        return Ok(());
    }
    plan.map_err(WasmrunError::self_update)?.run()?;
    report_outcome(&target.version, current, method);
    Ok(())
}

/// Handle `wasmrun update` and `wasmrun upgrade`
pub fn handle_update_command(scope: UpdateScope, check: bool) -> Result<()> {
    let current = SemVer::parse(env!("CARGO_PKG_VERSION")).map_err(WasmrunError::self_update)?;
    let exe = env::current_exe()
        .and_then(fs::canonicalize)
        .unwrap_or_default();
    let cargo_home = cargo_home();
    let Some(method) = detect_install_method(&exe, cargo_home.as_deref(), || system_owner(&exe))
    else {
        return Err(WasmrunError::self_update(format!(
            "Can't tell how this copy of wasmrun ({}) was installed, so it was left as it is.\n   \
             If you built it from source, pull and rebuild it. Otherwise reinstall it the way you \
             installed it: {INSTALL_DOCS}",
            exe.display()
        )));
    };
    debug_println!("{method:?} install, scope {scope:?}");
    println!(
        "wasmrun v{current}, installed with {method} ({})",
        exe.display()
    );

    let releases = fetch_releases(method)?;
    match decide(&releases, &current, scope, method) {
        Decision::UpToDate => print_info(&format!("wasmrun v{current} is the latest version")),
        Decision::NeedsUpgrade(latest) => {
            if !method.takes_exact_version() {
                println!(
                    "This install follows {method}, which only offers its newest version, so it \
                     can't stay on v{}.",
                    current.major
                );
            }
            point_to_upgrade(latest);
        }
        Decision::Install {
            target,
            newer_major,
        } => {
            install(target, &current, method, check)?;
            if let Some(latest) = newer_major {
                point_to_upgrade(latest);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> SemVer {
        SemVer::parse(text).unwrap()
    }

    fn release(text: &str) -> Release {
        Release {
            version: version(text),
            package: None,
        }
    }

    fn releases(versions: &[&str]) -> Vec<Release> {
        versions.iter().map(|text| release(text)).collect()
    }

    fn detect(exe: &str, owner: Option<InstallMethod>) -> Option<InstallMethod> {
        detect_install_method(Path::new(exe), None, || owner)
    }

    /// A `CARGO_HOME` holding `record` as cargo's list of what it installed.
    fn cargo_home_with(record: Option<&str>) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        if let Some(record) = record {
            fs::write(home.path().join(".crates2.json"), record).unwrap();
        }
        home
    }

    const FROM_CRATES_IO: &str = r#"{"installs":{
        "wasm-pack 0.13.1 (registry+https://github.com/rust-lang/crates.io-index)":{},
        "wasmrun 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["wasmrun"]}
    }}"#;

    #[test]
    fn test_homebrew_is_read_from_the_cellar_on_every_prefix() {
        for exe in [
            "/opt/homebrew/Cellar/wasmrun/1.0.0/bin/wasmrun",
            "/usr/local/Cellar/wasmrun/1.0.0_1/bin/wasmrun",
            "/home/linuxbrew/.linuxbrew/Cellar/wasmrun/1.0.0/bin/wasmrun",
        ] {
            assert_eq!(detect(exe, None), Some(InstallMethod::Homebrew));
        }
    }

    #[test]
    fn test_a_checkout_is_not_mistaken_for_a_package_manager() {
        for exe in [
            "/home/dev/homebrew-tools/target/debug/wasmrun",
            "/home/dev/Cellar/wasmrun/target/debug/wasmrun",
            "/home/dev/Cellar/other/1.0.0/bin/wasmrun",
            "/home/dev/.cargo/registry/wasmrun/target/release/wasmrun",
            "/usr/local/bin/wasmrun",
        ] {
            assert_eq!(detect(exe, None), None, "{exe}");
        }
    }

    #[test]
    fn test_cargo_needs_the_binary_in_its_bin_directory_and_a_crates_io_record() {
        let home = cargo_home_with(Some(FROM_CRATES_IO));
        let installed = home.path().join("bin/wasmrun");
        assert_eq!(
            detect_install_method(&installed, Some(home.path()), || None),
            Some(InstallMethod::Cargo)
        );
        assert_eq!(
            detect_install_method(Path::new("/opt/bin/wasmrun"), Some(home.path()), || None),
            None
        );
    }

    #[test]
    fn test_a_copy_cargo_did_not_take_from_crates_io_is_left_alone() {
        let from_source = r#"{"installs":{
            "wasmrun 1.0.0 (path+file:///home/dev/wasmrun)":{"bins":["wasmrun"]}
        }}"#;
        let from_git = r#"{"installs":{
            "wasmrun 1.0.0 (git+https://github.com/anistark/wasmrun#abc)":{"bins":["wasmrun"]}
        }}"#;
        let someone_else = r#"{"installs":{
            "wasmrun-tools 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)":{}
        }}"#;
        for record in [
            Some(from_source),
            Some(from_git),
            Some(someone_else),
            Some("not json"),
            None,
        ] {
            let home = cargo_home_with(record);
            let installed = home.path().join("bin/wasmrun");
            assert_eq!(
                detect_install_method(&installed, Some(home.path()), || None),
                None,
                "{record:?}"
            );
        }
    }

    #[test]
    fn test_a_system_package_manager_is_believed_when_it_owns_the_file() {
        for method in [InstallMethod::Aur, InstallMethod::Deb, InstallMethod::Rpm] {
            assert_eq!(detect("/usr/bin/wasmrun", Some(method)), Some(method));
        }
        assert_eq!(detect("/usr/bin/wasmrun", None), None);
    }

    #[test]
    fn test_the_crate_index_skips_yanked_versions() {
        let body = concat!(
            r#"{"name":"wasmrun","vers":"1.0.0","yanked":false}"#,
            "\n",
            r#"{"name":"wasmrun","vers":"1.1.0","yanked":true}"#,
            "\n",
            "not json\n",
        );
        assert_eq!(parse_crate_index(body), releases(&["1.0.0"]));
    }

    #[test]
    fn test_github_drafts_prereleases_and_unversioned_tags_are_skipped() {
        let arch = env::consts::ARCH;
        let asset = |version: &str| {
            format!(
                r#"[{{"name":"wasmrun-v{version}-{arch}.deb","browser_download_url":"https://x/{version}"}}]"#
            )
        };
        let body = format!(
            r#"[
                {{"tag_name":"v2.0.0","draft":true,"assets":{}}},
                {{"tag_name":"v1.9.0","prerelease":true,"assets":{}}},
                {{"tag_name":"v1.8.0","assets":{}}},
                {{"tag_name":"nightly","assets":{}}}
            ]"#,
            asset("2.0.0"),
            asset("1.9.0"),
            asset("1.8.0"),
            asset("nightly"),
        );
        let found = parse_github_releases(&body, "deb").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, version("1.8.0"));
    }

    #[test]
    fn test_a_package_install_needs_a_package_for_this_architecture() {
        let arch = env::consts::ARCH;
        let body = format!(
            r#"[
                {{"tag_name":"v1.9.0","assets":[
                    {{"name":"wasmrun","browser_download_url":"https://x/raw"}},
                    {{"name":"wasmrun-v1.9.0-other.deb","browser_download_url":"https://x/other"}}
                ]}},
                {{"tag_name":"v1.8.0","assets":[
                    {{"name":"wasmrun-v1.8.0-{arch}.rpm","browser_download_url":"https://x/rpm"}},
                    {{"name":"wasmrun-v1.8.0-{arch}.deb","browser_download_url":"https://x/deb"}}
                ]}}
            ]"#
        );
        let found = parse_github_releases(&body, "deb").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, version("1.8.0"));
        assert_eq!(found[0].package.as_ref().unwrap().url, "https://x/deb");
    }

    #[test]
    fn test_a_github_token_only_goes_to_github() {
        let token = Some("secret".to_string());
        assert_eq!(
            github_source(None, token.clone()),
            (GITHUB_RELEASES.to_string(), token.clone())
        );
        assert_eq!(
            github_source(Some("https://mirror.example/releases".to_string()), token),
            ("https://mirror.example/releases".to_string(), None)
        );
    }

    #[test]
    fn test_update_takes_the_newest_release_of_the_installed_major() {
        let published = releases(&["1.18.0", "1.18.1", "1.19.0", "2.0.0"]);
        assert_eq!(
            decide(
                &published,
                &version("1.18.0"),
                UpdateScope::SameMajor,
                InstallMethod::Cargo
            ),
            Decision::Install {
                target: &published[2],
                newer_major: Some(&published[3].version),
            }
        );
    }

    #[test]
    fn test_upgrade_takes_the_newest_release_there_is() {
        let published = releases(&["1.19.0", "2.0.0", "2.1.0"]);
        assert_eq!(
            decide(
                &published,
                &version("1.18.0"),
                UpdateScope::Latest,
                InstallMethod::Cargo
            ),
            Decision::Install {
                target: &published[2],
                newer_major: None,
            }
        );
    }

    #[test]
    fn test_upgrade_still_takes_a_minor_release_when_no_major_is_out() {
        let published = releases(&["1.19.0"]);
        assert_eq!(
            decide(
                &published,
                &version("1.18.0"),
                UpdateScope::Latest,
                InstallMethod::Homebrew
            ),
            Decision::Install {
                target: &published[0],
                newer_major: None,
            }
        );
    }

    #[test]
    fn test_update_stops_at_a_new_major_when_nothing_else_is_newer() {
        let published = releases(&["1.18.0", "2.0.0"]);
        assert_eq!(
            decide(
                &published,
                &version("1.18.0"),
                UpdateScope::SameMajor,
                InstallMethod::Cargo
            ),
            Decision::NeedsUpgrade(&published[1].version)
        );
    }

    #[test]
    fn test_update_holds_back_when_the_install_method_would_cross_a_major() {
        let published = releases(&["1.19.0", "2.0.0"]);
        for method in [InstallMethod::Homebrew, InstallMethod::Aur] {
            assert_eq!(
                decide(
                    &published,
                    &version("1.18.0"),
                    UpdateScope::SameMajor,
                    method
                ),
                Decision::NeedsUpgrade(&published[1].version)
            );
        }
    }

    #[test]
    fn test_nothing_newer_means_up_to_date() {
        let published = releases(&["1.17.0", "1.18.0", "1.19.0-rc.1"]);
        for scope in [UpdateScope::SameMajor, UpdateScope::Latest] {
            assert_eq!(
                decide(&published, &version("1.18.0"), scope, InstallMethod::Cargo),
                Decision::UpToDate
            );
        }
    }

    #[test]
    fn test_a_prerelease_build_moves_to_its_final_release() {
        let published = releases(&["1.19.0"]);
        assert_eq!(
            decide(
                &published,
                &version("1.19.0-rc.1"),
                UpdateScope::SameMajor,
                InstallMethod::Cargo
            ),
            Decision::Install {
                target: &published[0],
                newer_major: None,
            }
        );
    }

    #[test]
    fn test_cargo_is_pinned_to_the_chosen_version() {
        let plan = plan_for(InstallMethod::Cargo, &release("1.19.0"));
        if cfg!(windows) {
            assert!(plan.unwrap_err().contains("cargo install wasmrun"));
        } else {
            assert_eq!(
                plan.unwrap().command,
                argv(&[
                    "cargo",
                    "install",
                    "wasmrun",
                    "--version",
                    "1.19.0",
                    "--locked"
                ])
            );
        }
    }

    #[test]
    fn test_the_version_is_read_out_of_the_banner() {
        let banner = "\n\x1b[1;34m│\x1b[0m  🅦 \x1b[1;36mwasmrun v1.19.0\x1b[0m\n";
        assert_eq!(version_in_banner(banner), Some(version("1.19.0")));
        assert_eq!(
            version_in_banner("wasmrun v2.0.0-rc.1\x1b[0m"),
            Some(version("2.0.0-rc.1"))
        );
        assert_eq!(version_in_banner("wasmrun: command not found"), None);
    }
}
