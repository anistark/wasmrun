//! `wasmrun update` and `wasmrun upgrade` end to end, against a stub release
//! server and fake package managers.
//!
//! Install detection reads the path the binary runs from, so each test copies
//! the real binary to where that install method puts it. `PATH` holds only the
//! fake tools, which append their arguments to a log, so nothing on the machine
//! running the tests is touched.

#![cfg(unix)]

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tempfile::TempDir;

const WASMRUN: &str = env!("CARGO_BIN_EXE_wasmrun");
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";
const ARCH: &str = std::env::consts::ARCH;

fn current() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn core() -> (u64, u64, u64) {
    let release = env!("CARGO_PKG_VERSION").split('-').next().unwrap();
    let parts: Vec<u64> = release.split('.').map(|n| n.parse().unwrap()).collect();
    (parts[0], parts[1], parts[2])
}

fn next_patch() -> String {
    let (major, minor, patch) = core();
    format!("{major}.{minor}.{}", patch + 1)
}

fn next_minor() -> String {
    let (major, minor, _) = core();
    format!("{major}.{}.0", minor + 1)
}

fn minor_after_next() -> String {
    let (major, minor, _) = core();
    format!("{major}.{}.0", minor + 2)
}

fn next_major() -> String {
    format!("{}.0.0", core().0 + 1)
}

fn index_of(rows: &[(String, bool)]) -> String {
    rows.iter()
        .map(|(version, yanked)| {
            format!("{{\"name\":\"wasmrun\",\"vers\":\"{version}\",\"yanked\":{yanked}}}\n")
        })
        .collect()
}

/// A GitHub release list. Every release carries a package of each kind for this
/// architecture, served from `base`, unless it is named in `without_package`.
fn releases_of(base: &str, versions: &[String], without_package: &[String]) -> String {
    let rows: Vec<String> = versions
        .iter()
        .map(|version| {
            let assets: Vec<String> = if without_package.contains(version) {
                Vec::new()
            } else {
                ["deb", "rpm"]
                    .iter()
                    .map(|extension| {
                        let name = format!("wasmrun-v{version}-{ARCH}.{extension}");
                        format!(
                            r#"{{"name":"{name}","browser_download_url":"{base}/download/{name}"}}"#
                        )
                    })
                    .collect()
            };
            format!(
                r#"{{"tag_name":"v{version}","assets":[{}]}}"#,
                assets.join(",")
            )
        })
        .collect();
    format!("[{}]", rows.join(","))
}

/// One request the stub received: its path and its `Authorization` header.
type Seen = (String, Option<String>);

struct Stub {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Stub {
    /// Answers each request with `respond(base, path)`, and `/download/*` with a
    /// stand-in package.
    fn new(respond: impl Fn(&str, &str) -> (u16, String) + Send + 'static) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("failed to bind");
        let port = server.server_addr().to_ip().expect("no address").port();
        let base = format!("http://127.0.0.1:{port}");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (own_base, own_seen) = (base.clone(), seen.clone());
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let path = request.url().to_string();
                let token = request
                    .headers()
                    .iter()
                    .find(|header| header.field.equiv("Authorization"))
                    .map(|header| header.value.to_string());
                own_seen.lock().unwrap().push((path.clone(), token));
                let (status, body) = if path.starts_with("/download/") {
                    (200, "package bytes".to_string())
                } else {
                    respond(&own_base, &path)
                };
                let response = tiny_http::Response::from_string(body).with_status_code(status);
                let _ = request.respond(response);
            }
        });
        Self { base, seen }
    }

    fn paths(&self) -> Vec<String> {
        let seen = self.seen.lock().unwrap();
        seen.iter().map(|(path, _)| path.clone()).collect()
    }
}

/// Published versions on the crate index. Anything else is a wrong turn.
fn crate_index(rows: Vec<(String, bool)>) -> Stub {
    Stub::new(move |_, path| {
        assert_eq!(path, "/index", "asked the wrong source");
        (200, index_of(&rows))
    })
}

fn published(versions: &[String]) -> Stub {
    crate_index(versions.iter().map(|v| (v.clone(), false)).collect())
}

/// GitHub releases. Anything else is a wrong turn.
fn github_releases(versions: Vec<String>, without_package: Vec<String>) -> Stub {
    Stub::new(move |base, path| {
        assert_eq!(path, "/releases", "asked the wrong source");
        (200, releases_of(base, &versions, &without_package))
    })
}

/// Answers every request with `status`, the way GitHub does once an address
/// runs out of quota.
fn refusing(status: u16) -> Stub {
    Stub::new(move |_, _| (status, String::new()))
}

struct Install {
    root: TempDir,
    exe: PathBuf,
    tools: PathBuf,
    log: PathBuf,
}

impl Install {
    /// The real binary, copied to `relative_path` under a fresh root. A copy and
    /// not a hard link: macOS kills the odd process run through a hard link while
    /// another runs through the original.
    fn at(relative_path: &str) -> Self {
        let root = tempfile::tempdir().expect("failed to create temp root");
        let exe = root.path().join(relative_path);
        fs::create_dir_all(exe.parent().expect("no parent")).expect("failed to create bin dir");
        fs::copy(WASMRUN, &exe).expect("failed to copy wasmrun");
        let tools = root.path().join("tools");
        fs::create_dir_all(&tools).expect("failed to create tools dir");
        let log = root.path().join("tools.log");
        fs::write(&log, "").expect("failed to create log");
        Self {
            root,
            exe,
            tools,
            log,
        }
    }

    /// What `cargo install wasmrun` leaves behind: the binary, and cargo's
    /// record of it under `source`.
    fn cargo_from(source: &str) -> Self {
        let install = Self::at(".cargo/bin/wasmrun");
        let record = format!(
            r#"{{"installs":{{"wasmrun {} ({source})":{{"bins":["wasmrun"]}}}}}}"#,
            current()
        );
        fs::write(install.root.path().join(".cargo/.crates2.json"), record)
            .expect("failed to write cargo's record");
        install.tool("cargo", "");
        install
    }

    fn cargo() -> Self {
        Self::cargo_from(CRATES_IO)
    }

    fn homebrew() -> Self {
        let install = Self::at("Cellar/wasmrun/1.0.0/bin/wasmrun");
        install.tool("brew", "");
        install
    }

    fn aur() -> Self {
        let install = Self::at("usr/bin/wasmrun");
        install.tool("pacman", "echo wasmrun-bin");
        install
    }

    fn deb() -> Self {
        let install = Self::at("usr/bin/wasmrun");
        install.tool("dpkg-query", "");
        install
    }

    fn rpm() -> Self {
        let install = Self::at("usr/bin/wasmrun");
        install.tool("rpm", "");
        install
    }

    /// A tool that logs how it was called and then runs `body`.
    fn tool(&self, name: &str, body: &str) -> &Self {
        let path = self.tools.join(name);
        let log = self.log.display();
        fs::write(
            &path,
            format!("#!/bin/sh\necho \"{name} $*\" >> \"{log}\"\n{body}\n"),
        )
        .expect("failed to write fake tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("failed to mark fake tool executable");
        self
    }

    /// What `wasmrun --version` prints once the install has finished.
    fn reports_version(&self, version: &str) -> &Self {
        let path = self.tools.join("wasmrun");
        fs::write(
            &path,
            format!("#!/bin/sh\nprintf '\\033[1;36mwasmrun v{version}\\033[0m\\n'\n"),
        )
        .expect("failed to write fake wasmrun");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("failed to mark fake wasmrun executable");
        self
    }

    fn run(&self, stub: &Stub, args: &[&str]) -> Output {
        self.run_with(stub, args, &[])
    }

    fn run_with(&self, stub: &Stub, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = Command::new(&self.exe);
        command
            .args(args)
            .current_dir(self.root.path())
            .env_clear()
            .env("PATH", &self.tools)
            .env("HOME", self.root.path())
            .env("WASMRUN_UPDATE_INDEX", format!("{}/index", stub.base))
            .env("WASMRUN_UPDATE_RELEASES", format!("{}/releases", stub.base))
            .envs(env.iter().copied());
        // Linux won't run a file open for writing, and a process another test
        // thread is forking holds the copy's descriptor until it execs.
        for _ in 0..100 {
            match command.output() {
                Err(e) if e.kind() == ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => return result.expect("failed to run wasmrun"),
            }
        }
        panic!("{} stayed busy", self.exe.display());
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(&self.log)
            .expect("no log")
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The calls that install something, without the ones that only ask who
    /// owns the binary.
    fn installs(&self) -> Vec<String> {
        let queries = ["pacman -Qqo", "dpkg-query -S", "rpm -qf"];
        let mut calls = self.calls();
        calls.retain(|call| !queries.iter().any(|query| call.starts_with(query)));
        calls
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_ok(output: &Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout(output),
        stderr(output)
    );
}

fn cargo_install_of(version: &str) -> Vec<String> {
    vec![format!(
        "cargo install wasmrun --version {version} --locked"
    )]
}

#[test]
fn update_installs_the_newest_release_of_the_current_major() {
    let install = Install::cargo();
    install.reports_version(&next_minor());
    let stub = crate_index(vec![
        (next_patch(), false),
        (next_minor(), false),
        (minor_after_next(), true),
        (format!("{}-rc.1", next_major()), false),
        (next_major(), false),
    ]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    assert_eq!(install.calls(), cargo_install_of(&next_minor()));
    let text = stdout(&output);
    assert!(
        text.contains(&format!("wasmrun is now v{}", next_minor())),
        "{text}"
    );
    assert!(text.contains("wasmrun upgrade"), "{text}");
}

#[test]
fn upgrade_crosses_a_major_version() {
    let install = Install::cargo();
    install.reports_version(&next_major());
    let stub = published(&[next_minor(), next_major()]);

    let output = install.run(&stub, &["upgrade"]);

    assert_ok(&output);
    assert_eq!(install.calls(), cargo_install_of(&next_major()));
}

#[test]
fn upgrade_takes_the_newest_minor_when_no_major_is_out() {
    let install = Install::cargo();
    install.reports_version(&next_minor());
    let stub = published(&[next_patch(), next_minor()]);

    let output = install.run(&stub, &["upgrade"]);

    assert_ok(&output);
    assert_eq!(install.calls(), cargo_install_of(&next_minor()));
}

#[test]
fn update_leaves_a_new_major_to_upgrade() {
    let install = Install::cargo();
    let stub = published(&[next_major()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    assert_eq!(install.calls(), Vec::<String>::new());
    assert!(stdout(&output).contains("wasmrun upgrade"));
}

#[test]
fn check_says_what_would_happen_and_does_nothing() {
    let install = Install::cargo();
    let stub = published(&[next_minor(), next_major()]);

    for (command, target) in [("update", next_minor()), ("upgrade", next_major())] {
        let output = install.run(&stub, &[command, "--check"]);
        assert_ok(&output);
        let text = stdout(&output);
        assert!(text.contains("installed with cargo"), "{text}");
        assert!(text.contains(&format!("New version: v{target}")), "{text}");
        assert!(
            text.contains(&format!(
                "Would run: cargo install wasmrun --version {target} --locked"
            )),
            "{text}"
        );
    }
    assert_eq!(install.calls(), Vec::<String>::new());
}

#[test]
fn nothing_newer_runs_nothing() {
    let install = Install::cargo();
    let stub = published(&[current()]);

    for command in ["update", "upgrade"] {
        let output = install.run(&stub, &[command]);
        assert_ok(&output);
        assert!(stdout(&output).contains("latest version"));
    }
    assert_eq!(install.calls(), Vec::<String>::new());
}

#[test]
fn a_failed_install_fails_the_command() {
    let install = Install::cargo();
    install.tool("cargo", "exit 3");
    install.reports_version(&next_minor());
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("failed"), "{}", stderr(&output));
    assert!(!stdout(&output).contains("wasmrun is now"));
}

#[test]
fn the_install_keeps_the_terminal_and_stays_out_of_the_project() {
    let install = Install::cargo();
    install.tool(
        "cargo",
        "echo compiling wasmrun\ntest ! -e rust-toolchain.toml || exit 8",
    );
    install.reports_version(&next_minor());
    fs::write(
        install.root.path().join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"nightly\"\n",
    )
    .expect("failed to write rust-toolchain.toml");
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    assert!(stdout(&output).contains("compiling wasmrun"));
}

#[test]
fn a_custom_cargo_home_is_followed() {
    let install = Install::at("rust/bin/wasmrun");
    let record = format!(
        r#"{{"installs":{{"wasmrun {} ({CRATES_IO})":{{}}}}}}"#,
        current()
    );
    let home = install.root.path().join("rust");
    fs::write(home.join(".crates2.json"), record).expect("failed to write cargo's record");
    install.tool("cargo", "");
    let stub = published(&[next_minor()]);

    let home = home.to_str().expect("path is not UTF-8");
    let output = install.run_with(&stub, &["update", "--check"], &[("CARGO_HOME", home)]);

    assert_ok(&output);
    assert!(stdout(&output).contains("installed with cargo"));
}

#[test]
fn homebrew_runs_brew_when_the_update_stays_on_this_major() {
    let install = Install::homebrew();
    install.reports_version(&next_minor());
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    assert_eq!(install.calls(), vec!["brew upgrade wasmrun"]);
}

#[test]
fn homebrew_cannot_be_held_to_a_major_so_update_holds_back() {
    let install = Install::homebrew();
    let stub = published(&[next_major(), next_minor()]);

    let update = install.run(&stub, &["update"]);
    assert_ok(&update);
    assert_eq!(install.calls(), Vec::<String>::new());
    assert!(stdout(&update).contains("wasmrun upgrade"));

    install.reports_version(&next_major());
    let upgrade = install.run(&stub, &["upgrade"]);
    assert_ok(&upgrade);
    assert_eq!(install.calls(), vec!["brew upgrade wasmrun"]);
}

#[test]
fn the_new_version_is_read_from_path_after_brew_removes_this_copy() {
    let install = Install::homebrew();
    let cellar = install.root.path().join("Cellar");
    install.tool("brew", &format!("/bin/rm -rf \"{}\"", cellar.display()));
    install.reports_version(&next_minor());
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    assert!(!cellar.exists());
    assert!(stdout(&output).contains(&format!("wasmrun is now v{}", next_minor())));
}

#[test]
fn a_formula_behind_the_release_is_reported_rather_than_claimed() {
    let install = Install::homebrew();
    install.reports_version(&current());
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let text = stdout(&output);
    assert!(text.contains("is still"), "{text}");
    assert!(!text.contains("wasmrun is now"), "{text}");
}

#[test]
fn a_formula_partway_to_the_release_says_where_it_landed() {
    let install = Install::homebrew();
    install.reports_version(&next_patch());
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let text = stdout(&output);
    let expected = format!("`wasmrun` is v{}, not v{}", next_patch(), next_minor());
    assert!(text.contains(&expected), "{text}");
    assert!(!text.contains("wasmrun is now"), "{text}");
}

#[test]
fn no_wasmrun_on_path_afterwards_is_not_called_a_success() {
    let install = Install::homebrew();
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let text = stdout(&output);
    assert!(text.contains("could not confirm"), "{text}");
    assert!(!text.contains("wasmrun is now"), "{text}");
}

#[test]
fn the_aur_package_is_wasmrun_bin_through_either_helper() {
    for helper in ["paru", "yay"] {
        let install = Install::aur();
        install.tool(helper, "");
        install.reports_version(&next_minor());
        let stub = published(&[next_minor()]);

        let output = install.run(&stub, &["update"]);

        assert_ok(&output);
        assert_eq!(install.installs(), vec![format!("{helper} -S wasmrun-bin")]);
        assert_eq!(stub.paths(), vec!["/index"]);
    }
}

#[test]
fn the_aur_cannot_be_held_to_a_major_so_update_holds_back() {
    let install = Install::aur();
    install.tool("paru", "");
    let stub = published(&[next_major(), next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    assert_eq!(install.installs(), Vec::<String>::new());
    assert!(stdout(&output).contains("wasmrun upgrade"));
}

#[test]
fn an_aur_package_behind_the_release_is_reported_rather_than_claimed() {
    let install = Install::aur();
    install.tool("yay", "");
    install.reports_version(&current());
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let text = stdout(&output);
    assert!(text.contains("is still"), "{text}");
    assert!(text.contains("the AUR"), "{text}");
}

#[test]
fn without_an_aur_helper_the_command_says_what_to_do() {
    let install = Install::aur();
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("aur.archlinux.org/packages/wasmrun-bin"));
}

#[test]
fn a_deb_install_downloads_the_package_and_hands_it_to_apt() {
    let install = Install::deb();
    install.tool("sudo", r#"for last; do :; done; test -s "$last" || exit 9"#);
    install.reports_version(&next_minor());
    let stub = github_releases(vec![next_minor()], vec![]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let package = format!("wasmrun-v{}-{ARCH}.deb", next_minor());
    assert_eq!(
        install.installs(),
        vec![format!("sudo apt install ./{package}")]
    );
    assert_eq!(
        stub.paths(),
        vec!["/releases".to_string(), format!("/download/{package}")]
    );
}

#[test]
fn an_rpm_install_downloads_the_package_and_hands_it_to_rpm() {
    let install = Install::rpm();
    install.tool("sudo", r#"for last; do :; done; test -s "$last" || exit 9"#);
    install.reports_version(&next_minor());
    let stub = github_releases(vec![next_minor()], vec![]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let package = format!("wasmrun-v{}-{ARCH}.rpm", next_minor());
    assert_eq!(
        install.installs(),
        vec![format!("sudo rpm -Uvh ./{package}")]
    );
}

#[test]
fn root_needs_no_sudo() {
    let install = Install::deb();
    install.tool("id", "echo 0");
    install.tool("apt", "");
    install.reports_version(&next_minor());
    let stub = github_releases(vec![next_minor()], vec![]);

    let output = install.run(&stub, &["update"]);

    assert_ok(&output);
    let package = format!("wasmrun-v{}-{ARCH}.deb", next_minor());
    assert_eq!(
        install.installs().last().expect("no calls"),
        &format!("apt install ./{package}")
    );
}

#[test]
fn a_deb_install_can_stay_on_this_major_after_a_new_one_is_out() {
    let install = Install::deb();
    install.tool("sudo", "");
    let stub = github_releases(vec![next_minor(), next_major()], vec![]);

    let output = install.run(&stub, &["update", "--check"]);

    assert_ok(&output);
    let text = stdout(&output);
    assert!(
        text.contains(&format!("New version: v{}", next_minor())),
        "{text}"
    );
    assert!(text.contains("wasmrun upgrade"), "{text}");
}

#[test]
fn a_release_still_building_its_package_is_passed_over() {
    let install = Install::deb();
    install.tool("sudo", "");
    let stub = github_releases(vec![next_minor(), next_patch()], vec![next_minor()]);

    let output = install.run(&stub, &["update", "--check"]);

    assert_ok(&output);
    assert!(stdout(&output).contains(&format!("New version: v{}", next_patch())));
}

#[test]
fn no_package_for_this_architecture_is_an_error_not_up_to_date() {
    let install = Install::deb();
    let stub = github_releases(vec![next_minor()], vec![next_minor()]);

    let output = install.run(&stub, &["update"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains(ARCH), "{}", stderr(&output));
    assert!(!stdout(&output).contains("latest version"));
}

#[test]
fn a_rate_limited_github_says_how_to_raise_the_limit() {
    for status in [403, 429] {
        let install = Install::rpm();

        let output = install.run(&refusing(status), &["update"]);

        assert_eq!(output.status.code(), Some(1));
        let text = stderr(&output);
        assert!(text.contains("GITHUB_TOKEN"), "{text}");
        assert!(!text.contains(&status.to_string()), "{text}");
    }
}

#[test]
fn a_github_token_is_not_sent_to_a_mirror() {
    let install = Install::deb();
    install.tool("sudo", "");
    let stub = github_releases(vec![next_minor()], vec![]);

    let output = install.run_with(
        &stub,
        &["update", "--check"],
        &[("GITHUB_TOKEN", "secret-token")],
    );

    assert_ok(&output);
    let seen = stub.seen.lock().unwrap();
    assert_eq!(seen.as_slice(), [("/releases".to_string(), None)]);
}

#[test]
fn without_sudo_a_package_install_prints_what_to_do() {
    let install = Install::deb();
    let stub = github_releases(vec![next_minor()], vec![]);

    let output = install.run(&stub, &["update"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("as root"), "{}", stderr(&output));
    assert_eq!(install.installs(), Vec::<String>::new());
    assert_eq!(stub.paths(), vec!["/releases"]);
}

#[test]
fn a_crate_index_that_is_not_one_is_an_error_not_up_to_date() {
    let install = Install::cargo();
    let stub = Stub::new(|_, _| (200, "<html>sign in to the wifi</html>".to_string()));

    let output = install.run(&stub, &["update"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(!stdout(&output).contains("latest version"));
}

/// Nobody claims this copy: it is left alone with instructions, whether or not
/// anything newer exists, and no release source is asked.
fn assert_left_alone(install: &Install) {
    for args in [&["update"][..], &["upgrade"], &["update", "--check"]] {
        let stub = published(&[next_minor()]);
        let output = install.run(&stub, args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let text = stderr(&output);
        assert!(text.contains("Can't tell"), "{text}");
        assert!(text.contains("readthedocs"), "{text}");
        assert_eq!(install.installs(), Vec::<String>::new());
        assert_eq!(stub.paths(), Vec::<String>::new());
    }
}

#[test]
fn an_install_nobody_claims_is_left_alone() {
    let install = Install::at("somewhere/bin/wasmrun");
    install.tool("cargo", "");
    install.tool("brew", "");
    assert_left_alone(&install);
}

#[test]
fn a_build_in_a_folder_named_homebrew_is_left_alone() {
    let install = Install::at("homebrew-tools/target/debug/wasmrun");
    install.tool("cargo", "");
    install.tool("brew", "");
    assert_left_alone(&install);
}

#[test]
fn a_copy_built_from_source_into_cargos_bin_is_left_alone() {
    let install = Install::cargo_from("path+file:///home/dev/wasmrun");
    assert_left_alone(&install);
}

#[test]
fn a_copy_moved_into_cargos_bin_by_hand_is_left_alone() {
    let install = Install::at(".cargo/bin/wasmrun");
    install.tool("cargo", "");
    assert_left_alone(&install);
}

#[test]
fn a_package_pacman_owns_under_another_name_is_left_alone() {
    let install = Install::at("usr/bin/wasmrun");
    install.tool("pacman", "echo wasmrun-git");
    install.tool("paru", "");
    assert_left_alone(&install);
}

#[test]
fn a_project_config_cannot_redirect_the_release_source() {
    let install = Install::cargo();
    fs::write(
        install.root.path().join("wasmrun.toml"),
        "[update]\nindex = \"http://127.0.0.1:1/index\"\nreleases = \"http://127.0.0.1:1/r\"\n",
    )
    .expect("failed to write wasmrun.toml");
    let stub = published(&[next_minor()]);

    let output = install.run(&stub, &["update", "--check"]);

    assert_ok(&output);
    assert!(stdout(&output).contains(&format!("New version: v{}", next_minor())));
}
