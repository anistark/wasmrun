//! Agent mode: npm dependency vendoring.
//!
//! The sandbox has no network, so dependencies are resolved and fetched
//! host-side at the ingress boundary, then laid out in the session's
//! `node_modules` tree where the wasmhub nodejs runtime's own `require()`
//! resolves them. wasmrun talks to the npm registry directly — no `npm`
//! binary on the host, and package lifecycle scripts are **never** executed.
//!
//! Layout is npm2-style fully nested: each package's own dependencies live in
//! its private `node_modules`, with a walk-up dedupe check (mirroring node's
//! resolution order) that both avoids duplicates and breaks dependency
//! cycles. Nesting is always correct with a walk-up resolver; hoisting is an
//! optimization we can add later.
//!
//! Only pure-JS packages are supported: anything with an install script or
//! native-binding artifacts (`binding.gyp`, `*.node`) is rejected with a
//! clear error naming the package.

use crate::agent::api::ApiError;
use crate::agent::limits::{dir_size, ResourceLimits};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DEFAULT_NPM_REGISTRY: &str = "https://registry.npmjs.org";

/// Hard ceilings that bound a malicious or runaway dependency tree
/// independently of the session's configurable resource limits.
const MAX_PACKAGES_PER_REQUEST: usize = 256;
const MAX_DEPTH: usize = 64;
const MAX_TARBALL_BYTES: u64 = 100 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 500 * 1024 * 1024;

pub struct Vendor {
    registry: String,
    cache_dir: PathBuf,
}

/// Abbreviated registry document (`application/vnd.npm.install-v1+json`).
#[derive(Debug, Deserialize)]
struct PackageDoc {
    #[serde(rename = "dist-tags", default)]
    dist_tags: HashMap<String, String>,
    #[serde(default)]
    versions: HashMap<String, VersionDoc>,
}

#[derive(Debug, Clone, Deserialize)]
struct VersionDoc {
    version: String,
    #[serde(default)]
    dependencies: HashMap<String, String>,
    dist: DistDoc,
    #[serde(rename = "hasInstallScript", default)]
    has_install_script: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct DistDoc {
    tarball: String,
    /// Subresource-integrity string, e.g. `sha512-<base64>`. Registry docs
    /// published since ~2017 always carry it; we require it.
    integrity: Option<String>,
}

/// Per-request state: memoized registry docs, a package counter, and the
/// lockfile accumulated as the tree is walked.
struct Ctx {
    docs: HashMap<String, PackageDoc>,
    installed: usize,
    lock: Lockfile,
}

/// A resolved dependency tree: install path (relative to the session root,
/// e.g. `node_modules/a/node_modules/b`) → the package that goes there.
/// Replaying one pins every version, including transitive ones.
pub type Lockfile = BTreeMap<String, LockEntry>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    pub version: String,
    pub resolved: String,
    /// Verified on every download.
    pub integrity: String,
}

/// Validate a `dependencies` map without touching the network, so obviously
/// bad input (invalid names, unsupported ranges) fails fast with a 400
/// before an exec worker is spawned.
pub fn validate_deps(deps: &HashMap<String, String>) -> std::result::Result<(), ApiError> {
    for (name, range) in deps {
        validate_package_name(name)?;
        Range::parse(range).map_err(|e| {
            ApiError::BadRequest(format!(
                "Unsupported version range '{range}' for '{name}': {e}"
            ))
        })?;
    }
    Ok(())
}

impl Vendor {
    pub fn new(registry: &str) -> std::result::Result<Self, ApiError> {
        let cache_dir = dirs::home_dir()
            .ok_or_else(|| ApiError::Internal("Could not determine home directory".into()))?
            .join(".wasmrun")
            .join("npm");
        Ok(Self {
            registry: registry.trim_end_matches('/').to_string(),
            cache_dir,
        })
    }

    #[cfg(test)]
    pub fn with_cache_dir(registry: &str, cache_dir: PathBuf) -> Self {
        Self {
            registry: registry.trim_end_matches('/').to_string(),
            cache_dir,
        }
    }

    /// Vendor `deps` (name → npm range) into `{work_dir}/node_modules`,
    /// returning a lockfile describing the tree that was resolved.
    ///
    /// Idempotent per session: a dependency already present at a satisfying
    /// version is skipped, so repeat execs with the same `dependencies` map
    /// cost nothing. A package skipped because an *earlier exec* installed it
    /// is not in the returned lockfile: recording it would need the registry
    /// lookup the skip exists to avoid.
    pub fn vendor(
        &self,
        deps: &HashMap<String, String>,
        work_dir: &Path,
        limits: &ResourceLimits,
    ) -> std::result::Result<Lockfile, ApiError> {
        let mut ctx = Ctx {
            docs: HashMap::new(),
            installed: 0,
            lock: Lockfile::new(),
        };
        let root_nm = work_dir.join("node_modules");
        let mut chain = vec![root_nm];

        // Sorted for deterministic installation order and error reporting.
        let mut names: Vec<&String> = deps.keys().collect();
        names.sort();
        for name in names {
            self.install_into(name, &deps[name], &mut chain, limits, &mut ctx, 0)?;
        }
        Ok(ctx.lock)
    }

    /// Install exactly the tree a previous `vendor` call recorded.
    ///
    /// Replay walks the lockfile rather than the dependency graph, so nothing
    /// is re-resolved and with a warm cache it runs offline. Tarballs are
    /// still integrity-verified and scanned for native artifacts.
    pub fn vendor_locked(
        &self,
        lock: &Lockfile,
        work_dir: &Path,
        limits: &ResourceLimits,
    ) -> std::result::Result<(), ApiError> {
        if lock.len() > MAX_PACKAGES_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Lockfile exceeds {MAX_PACKAGES_PER_REQUEST} packages",
            )));
        }
        // BTreeMap order puts a parent before anything nested inside it.
        for (path, entry) in lock {
            let name = validate_lock_path(path)?;
            let dest = work_dir.join(path);

            if installed_version(&dest).as_deref() == Some(entry.version.as_str()) {
                continue;
            }

            let cached =
                self.ensure_cached(name, &entry.version, &entry.resolved, &entry.integrity)?;
            if let Some(max_disk) = limits.max_disk_bytes {
                let current = dir_size(work_dir);
                if current.saturating_add(dir_size(&cached)) > max_disk {
                    return Err(ApiError::BadRequest(format!(
                        "Disk usage limit exceeded while vendoring '{name}@{}'",
                        entry.version
                    )));
                }
            }
            copy_dir(&cached, &dest, limits)?;
        }
        Ok(())
    }

    /// Install `name@range` into the last `node_modules` dir of `chain`,
    /// then recurse into its dependencies (npm2-style nesting). `chain`
    /// holds the `node_modules` ancestry for walk-up dedupe.
    fn install_into(
        &self,
        name: &str,
        range: &str,
        chain: &mut Vec<PathBuf>,
        limits: &ResourceLimits,
        ctx: &mut Ctx,
        depth: usize,
    ) -> std::result::Result<(), ApiError> {
        if depth > MAX_DEPTH {
            return Err(ApiError::BadRequest(format!(
                "Dependency tree too deep (> {MAX_DEPTH}) while installing '{name}'",
            )));
        }
        validate_package_name(name)?;
        let parsed_range = Range::parse(range).map_err(|e| {
            ApiError::BadRequest(format!(
                "Unsupported version range '{range}' for '{name}': {e}"
            ))
        })?;

        // Walk-up dedupe: if any ancestor node_modules already provides a
        // satisfying version, node's resolver will find it — skip. This is
        // also what terminates dependency cycles.
        for nm in chain.iter().rev() {
            if let Some(existing) = installed_version(&nm.join(name)) {
                if let Ok(v) = SemVer::parse(&existing) {
                    if parsed_range.matches_or_any_tag(&v, &existing) {
                        return Ok(());
                    }
                }
            }
        }

        ctx.installed += 1;
        if ctx.installed > MAX_PACKAGES_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Dependency tree exceeds {MAX_PACKAGES_PER_REQUEST} packages",
            )));
        }

        let vdoc = self.resolve(name, &parsed_range, ctx)?;
        if vdoc.has_install_script {
            return Err(ApiError::BadRequest(format!(
                "Package '{name}@{}' declares an install script; packages with lifecycle scripts (usually native bindings) are not supported in the sandbox",
                vdoc.version
            )));
        }

        let integrity = vdoc.dist.integrity.as_deref().ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Package '{name}@{}' has no sha512 integrity in the registry; refusing to vendor unverifiable tarballs",
                vdoc.version
            ))
        })?;
        let cached = self.ensure_cached(name, &vdoc.version, &vdoc.dist.tarball, integrity)?;

        let parent_nm = chain.last().expect("chain never empty").clone();
        let dest = parent_nm.join(name);
        let session_root = chain[0].parent().expect("root nm has parent").to_path_buf();

        if let Ok(rel) = dest.strip_prefix(&session_root) {
            ctx.lock.insert(
                rel.to_string_lossy().replace('\\', "/"),
                LockEntry {
                    version: vdoc.version.clone(),
                    resolved: vdoc.dist.tarball.clone(),
                    integrity: integrity.to_string(),
                },
            );
        }

        // Bound the copy by the session's disk quota before writing.
        if let Some(max_disk) = limits.max_disk_bytes {
            let pkg_size = dir_size(&cached);
            let current = dir_size(&session_root);
            if current.saturating_add(pkg_size) > max_disk {
                return Err(ApiError::BadRequest(format!(
                    "Disk usage limit exceeded while vendoring '{name}@{}'",
                    vdoc.version
                )));
            }
        }
        copy_dir(&cached, &dest, limits)?;

        // Recurse into the package's own (production) dependencies.
        let mut dep_names: Vec<&String> = vdoc.dependencies.keys().collect();
        dep_names.sort();
        chain.push(dest.join("node_modules"));
        let result = (|| {
            for dep in dep_names {
                self.install_into(dep, &vdoc.dependencies[dep], chain, limits, ctx, depth + 1)?;
            }
            Ok(())
        })();
        chain.pop();
        result
    }

    /// Resolve `range` against the registry document for `name`.
    fn resolve(
        &self,
        name: &str,
        range: &Range,
        ctx: &mut Ctx,
    ) -> std::result::Result<VersionDoc, ApiError> {
        if !ctx.docs.contains_key(name) {
            let url = format!("{}/{}", self.registry, urlencode_name(name));
            let body = http_get_string(&url).map_err(|e| {
                ApiError::Internal(format!("npm registry request failed for '{name}': {e}"))
            })?;
            let doc: PackageDoc = serde_json::from_str(&body).map_err(|e| {
                ApiError::Internal(format!("Invalid registry metadata for '{name}': {e}"))
            })?;
            ctx.docs.insert(name.to_string(), doc);
        }
        let doc = &ctx.docs[name];

        // A dist-tag range ("latest", "next", ...) resolves through dist-tags.
        if let Range::Tag(tag) = range {
            let ver = doc.dist_tags.get(tag).ok_or_else(|| {
                ApiError::BadRequest(format!("Unknown dist-tag '{tag}' for package '{name}'"))
            })?;
            return doc.versions.get(ver).cloned().ok_or_else(|| {
                ApiError::Internal(format!(
                    "dist-tag '{tag}' points at missing version {ver} for '{name}'"
                ))
            });
        }

        let mut best: Option<(SemVer, &VersionDoc)> = None;
        for (ver_str, vdoc) in &doc.versions {
            let Ok(ver) = SemVer::parse(ver_str) else {
                continue;
            };
            if range.matches(&ver) && best.as_ref().is_none_or(|(b, _)| ver > *b) {
                best = Some((ver, vdoc));
            }
        }
        best.map(|(_, v)| v.clone()).ok_or_else(|| {
            ApiError::BadRequest(format!(
                "No version of '{name}' satisfies '{}'",
                range.display()
            ))
        })
    }

    /// Return the cache directory for `name@version`, downloading, verifying,
    /// and extracting the tarball on first use.
    fn ensure_cached(
        &self,
        name: &str,
        version: &str,
        tarball_url: &str,
        integrity: &str,
    ) -> std::result::Result<PathBuf, ApiError> {
        let pkg_cache = self.cache_dir.join(name).join(version);
        if pkg_cache.join("package.json").exists() {
            return Ok(pkg_cache);
        }

        let tarball = http_get_bytes(tarball_url, MAX_TARBALL_BYTES).map_err(|e| {
            ApiError::Internal(format!("Failed to download '{name}@{version}': {e}"))
        })?;
        verify_integrity(&tarball, integrity).map_err(|e| {
            ApiError::BadRequest(format!(
                "Integrity check failed for '{name}@{version}': {e}"
            ))
        })?;

        // Extract into a temp sibling, then rename into place so a crash
        // mid-extract never leaves a half-populated cache entry behind.
        let tmp =
            self.cache_dir
                .join(name)
                .join(format!(".tmp-{}-{}", version, std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        extract_tarball(&tarball, &tmp).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&tmp);
        })?;
        reject_native_artifacts(name, version, &tmp).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&tmp);
        })?;

        if let Some(parent) = pkg_cache.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ApiError::Internal(format!("npm cache dir: {e}")))?;
        }
        match std::fs::rename(&tmp, &pkg_cache) {
            Ok(()) => {}
            // A concurrent exec may have populated the entry first — fine.
            Err(_) if pkg_cache.join("package.json").exists() => {
                let _ = std::fs::remove_dir_all(&tmp);
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(ApiError::Internal(format!("npm cache rename: {e}")));
            }
        }
        Ok(pkg_cache)
    }
}

/// The installed version at `pkg_dir`, read from its package.json.
fn installed_version(pkg_dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(pkg_dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("version")?.as_str().map(String::from)
}

/// npm package-name rules (subset): lowercase URL-safe, optionally scoped.
/// Validate a lockfile install path and return the package name it installs.
///
/// Caller-supplied paths decide where files land, so anything but a
/// well-formed `node_modules/<name>` chain is refused rather than normalized.
fn validate_lock_path(path: &str) -> std::result::Result<&str, ApiError> {
    let bad = |why: &str| {
        Err(ApiError::BadRequest(format!(
            "Invalid lockfile path '{path}': {why}"
        )))
    };
    if path.contains('\\') || path.starts_with('/') {
        return bad("must be a relative path with / separators");
    }

    // Repeated "node_modules/<name>" pairs; <name> is two segments if scoped.
    let mut segments = path.split('/').peekable();
    let mut name = None;
    let mut depth = 0usize;
    while segments.peek().is_some() {
        if segments.next() != Some("node_modules") {
            return bad("every package must sit directly inside a node_modules directory");
        }
        depth += 1;
        if depth > MAX_DEPTH {
            return bad("nesting too deep");
        }
        let Some(first) = segments.next() else {
            return bad("trailing node_modules with no package");
        };
        let pkg = if first.starts_with('@') {
            let Some(second) = segments.next() else {
                return bad("scoped name must be @scope/name");
            };
            format!("{first}/{second}")
        } else {
            first.to_string()
        };
        validate_package_name(&pkg)?;
        name = Some(pkg);
    }

    let Some(name) = name else {
        return bad("empty path");
    };
    Ok(&path[path.len() - name.len()..])
}

fn validate_package_name(name: &str) -> std::result::Result<(), ApiError> {
    let bad = |why: &str| {
        Err(ApiError::BadRequest(format!(
            "Invalid package name '{name}': {why}"
        )))
    };
    if name.is_empty() || name.len() > 214 {
        return bad("empty or too long");
    }
    let unscoped = if let Some(rest) = name.strip_prefix('@') {
        let Some((scope, pkg)) = rest.split_once('/') else {
            return bad("scoped name must be @scope/name");
        };
        if scope.is_empty() || pkg.is_empty() || pkg.contains('/') {
            return bad("scoped name must be @scope/name");
        }
        for part in [scope, pkg] {
            if part.starts_with('.') || part.starts_with('_') {
                return bad("segments must not start with . or _");
            }
        }
        pkg
    } else {
        if name.starts_with('.') || name.starts_with('_') {
            return bad("must not start with . or _");
        }
        name
    };
    let valid_chars = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.~".contains(c))
    };
    let scope_ok = name
        .strip_prefix('@')
        .and_then(|r| r.split_once('/'))
        .map(|(s, _)| valid_chars(s))
        .unwrap_or(true);
    if !valid_chars(unscoped) || !scope_ok {
        return bad("only lowercase letters, digits, and - _ . ~ are allowed");
    }
    Ok(())
}

fn urlencode_name(name: &str) -> String {
    // Only '/' (in scoped names) needs escaping given validate_package_name.
    name.replace('/', "%2F")
}

/// Reject packages that ship native-binding artifacts even without an
/// install script (prebuilt `.node` binaries can't run in the sandbox).
fn reject_native_artifacts(
    name: &str,
    version: &str,
    dir: &Path,
) -> std::result::Result<(), ApiError> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d)
            .map_err(|e| ApiError::Internal(format!("scan extracted package: {e}")))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let fname = entry.file_name();
                let fname = fname.to_string_lossy();
                if fname == "binding.gyp" || fname.ends_with(".node") {
                    return Err(ApiError::BadRequest(format!(
                        "Package '{name}@{version}' contains native-binding artifacts ('{fname}'); only pure-JS packages are supported in the sandbox"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Safely extract an npm tarball (gzip'd tar with a single root directory,
/// usually `package/`) into `dest`, stripping the root component. Entries
/// that traverse out, are absolute, or are not regular files/directories
/// are skipped; symlinks and hardlinks are never materialized.
fn extract_tarball(tarball: &[u8], dest: &Path) -> std::result::Result<(), ApiError> {
    let gz = flate2::read::GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(gz);
    let mut unpacked: u64 = 0;

    std::fs::create_dir_all(dest).map_err(|e| ApiError::Internal(format!("extract dir: {e}")))?;
    let entries = archive
        .entries()
        .map_err(|e| ApiError::BadRequest(format!("Corrupt package tarball: {e}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| ApiError::BadRequest(format!("Corrupt package tarball: {e}")))?;
        let kind = entry.header().entry_type();
        if !matches!(kind, tar::EntryType::Regular | tar::EntryType::Directory) {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| ApiError::BadRequest(format!("Corrupt package tarball: {e}")))?
            .into_owned();

        // Strip the root component ("package/") and sanitize the rest.
        let mut components = path.components();
        components.next();
        let rel: PathBuf = components
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .collect();
        if rel.as_os_str().is_empty()
            || path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            continue;
        }

        let out = dest.join(&rel);
        if kind == tar::EntryType::Directory {
            std::fs::create_dir_all(&out)
                .map_err(|e| ApiError::Internal(format!("extract mkdir: {e}")))?;
            continue;
        }
        unpacked = unpacked.saturating_add(entry.size());
        if unpacked > MAX_UNPACKED_BYTES {
            return Err(ApiError::BadRequest(
                "Package unpacks to an unreasonable size".into(),
            ));
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ApiError::Internal(format!("extract mkdir: {e}")))?;
        }
        let mut file = std::fs::File::create(&out)
            .map_err(|e| ApiError::Internal(format!("extract file: {e}")))?;
        std::io::copy(&mut entry, &mut file)
            .map_err(|e| ApiError::Internal(format!("extract write: {e}")))?;
    }
    Ok(())
}

/// Verify an SRI string of the form `sha512-<base64>`.
fn verify_integrity(data: &[u8], integrity: &str) -> std::result::Result<(), String> {
    use base64::Engine;
    use sha2::Digest;
    // npm may list several space-separated SRI entries; accept any sha512 one.
    for entry in integrity.split_whitespace() {
        if let Some(expected_b64) = entry.strip_prefix("sha512-") {
            let expected = base64::engine::general_purpose::STANDARD
                .decode(expected_b64)
                .map_err(|e| format!("bad integrity encoding: {e}"))?;
            let actual = sha2::Sha512::digest(data);
            return if actual.as_slice() == expected.as_slice() {
                Ok(())
            } else {
                Err("sha512 digest mismatch".into())
            };
        }
    }
    Err(format!("no sha512 entry in integrity string '{integrity}'"))
}

fn copy_dir(src: &Path, dest: &Path, limits: &ResourceLimits) -> std::result::Result<(), ApiError> {
    std::fs::create_dir_all(dest).map_err(|e| ApiError::Internal(format!("vendor copy: {e}")))?;
    let entries =
        std::fs::read_dir(src).map_err(|e| ApiError::Internal(format!("vendor copy: {e}")))?;
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to, limits)?;
        } else {
            if let Some(max_file) = limits.max_file_size {
                let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if len > max_file {
                    return Err(ApiError::BadRequest(format!(
                        "File size limit exceeded by vendored file {}",
                        from.display()
                    )));
                }
            }
            std::fs::copy(&from, &to)
                .map_err(|e| ApiError::Internal(format!("vendor copy: {e}")))?;
        }
    }
    Ok(())
}

// ── HTTP helpers ────────────────────────────────────────────────────

fn http_get_string(url: &str) -> std::result::Result<String, String> {
    let mut body = ureq::get(url)
        .header("Accept", "application/vnd.npm.install-v1+json")
        .call()
        .map_err(|e| format!("HTTP request failed for {url}: {e}"))?
        .into_body();
    let mut buf = String::new();
    body.as_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("Failed to read response body: {e}"))?;
    Ok(buf)
}

fn http_get_bytes(url: &str, max_bytes: u64) -> std::result::Result<Vec<u8>, String> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed for {url}: {e}"))?
        .into_body();
    let mut buf = Vec::new();
    body.into_reader()
        .take(max_bytes + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read response body: {e}"))?;
    if buf.len() as u64 > max_bytes {
        return Err(format!("response exceeds {max_bytes} byte limit"));
    }
    Ok(buf)
}

// ── npm semver (subset) ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Option<String>,
}

impl SemVer {
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        let s = s.trim().trim_start_matches('v');
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p.split('+').next().unwrap_or(p).to_string())),
            None => (s.split('+').next().unwrap_or(s), None),
        };
        let mut parts = core.split('.');
        let mut next_num = |what: &str| {
            parts
                .next()
                .ok_or(format!("missing {what}"))?
                .parse::<u64>()
                .map_err(|_| format!("invalid {what}"))
        };
        let major = next_num("major")?;
        let minor = next_num("minor")?;
        let patch = next_num("patch")?;
        if parts.next().is_some() {
            return Err("too many version components".into());
        }
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => std::cmp::Ordering::Equal,
                // A prerelease sorts below its release.
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(a), Some(b)) => cmp_prerelease(a, b),
            })
    }
}

/// Semver prerelease ordering: numeric identifiers compare numerically and
/// sort below alphanumeric ones. String comparison gets `alpha.10` vs
/// `alpha.2` backwards.
fn cmp_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ai = a.split('.');
    let mut bi = b.split('.');
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

impl SemVer {
    fn core(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
}

/// An operator plus a version bound. Every range form desugars into one or
/// two of these, which is what removes the special case per shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparator {
    op: Op,
    ver: SemVer,
}

impl Comparator {
    fn matches(&self, v: &SemVer) -> bool {
        let ord = v.cmp(&self.ver);
        match self.op {
            Op::Lt => ord.is_lt(),
            Op::Lte => ord.is_le(),
            Op::Gt => ord.is_gt(),
            Op::Gte => ord.is_ge(),
            Op::Eq => ord.is_eq(),
        }
    }
}

/// How many components a written version specified: what distinguishes `1.2`
/// from `1.2.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Precision {
    Major,
    Minor,
    Patch,
}

/// An npm version range: a union of comparator sets. Alternatives separated
/// by `||` are OR'd, comparators within one are AND'd, and every written shape
/// (`^1.2.3`, `1.x`, `1.2.0 - 1.4.0`, `>=1.2.0 <2.0.0`) desugars into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Range {
    /// `*`, `x`, or empty: any release, never a prerelease.
    Any,
    /// Resolved through the registry's dist-tags rather than by matching.
    Tag(String),
    Set {
        /// The range as written, kept verbatim for error messages.
        raw: String,
        alts: Vec<Vec<Comparator>>,
    },
}

impl Range {
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        let raw = s.trim();
        if raw.is_empty() || raw == "*" || raw == "x" || raw == "X" {
            return Ok(Range::Any);
        }
        if is_dist_tag(raw) {
            return Ok(Range::Tag(raw.to_string()));
        }
        let mut alts = Vec::new();
        for part in raw.split("||") {
            alts.push(parse_comparator_set(part)?);
        }
        // `* || ^1` and friends collapse to "anything".
        if alts.iter().any(|a| a.is_empty()) {
            return Ok(Range::Any);
        }
        Ok(Range::Set {
            raw: raw.to_string(),
            alts,
        })
    }

    pub fn matches(&self, v: &SemVer) -> bool {
        match self {
            Range::Any => v.pre.is_none(),
            Range::Tag(_) => false, // resolved via dist-tags, not matching
            Range::Set { alts, .. } => alts.iter().any(|alt| set_matches(alt, v)),
        }
    }

    /// `matches`, but a dist-tag range accepts the exact version string the
    /// tag resolved to (used by the walk-up dedupe check).
    fn matches_or_any_tag(&self, v: &SemVer, _raw: &str) -> bool {
        match self {
            // For dedupe purposes any installed version satisfies a tag range
            // only if it is what the tag currently resolves to — we can't
            // know that offline, so be conservative and never dedupe tags.
            Range::Tag(_) => false,
            _ => self.matches(v),
        }
    }

    fn display(&self) -> String {
        match self {
            Range::Any => "*".into(),
            Range::Tag(t) => t.clone(),
            Range::Set { raw, .. } => raw.clone(),
        }
    }
}

/// A version satisfies an alternative when every comparator in it matches.
///
/// Prereleases carry the extra npm rule: `1.2.3-beta` may only satisfy a set
/// that explicitly names a prerelease of `1.2.3`. Without it, `<2.0.0` would
/// quietly pull in `2.0.0-rc.1`, which sorts below `2.0.0`.
fn set_matches(alt: &[Comparator], v: &SemVer) -> bool {
    if !alt.iter().all(|c| c.matches(v)) {
        return false;
    }
    if v.pre.is_some() {
        return alt
            .iter()
            .any(|c| c.ver.pre.is_some() && c.ver.core() == v.core());
    }
    true
}

/// A bare identifier that is not a version or an operator is a dist-tag.
fn is_dist_tag(s: &str) -> bool {
    if s.contains("||") || s.chars().any(char::is_whitespace) {
        return false;
    }
    match s.chars().next() {
        None => false,
        Some('^' | '~' | '>' | '<' | '=') => false,
        Some(_) => !s
            .trim_start_matches('v')
            .starts_with(|c: char| c.is_ascii_digit()),
    }
}

/// Parse one `||` alternative into an AND-ed list of comparators. An empty
/// result means "anything".
fn parse_comparator_set(part: &str) -> std::result::Result<Vec<Comparator>, String> {
    let part = part.trim();
    if part.is_empty() || part == "*" || part == "x" || part == "X" {
        return Ok(Vec::new());
    }

    // Hyphen range: "1.2.0 - 1.4.0". Checked before tokenizing because the
    // separator is itself whitespace-delimited.
    if let Some((lo, hi)) = split_hyphen(part) {
        let (lo_ver, lo_prec) = parse_partial_spec(lo)?;
        let (hi_ver, hi_prec) = parse_partial_spec(hi)?;
        let mut out = vec![Comparator {
            op: Op::Gte,
            ver: zeroed(&lo_ver, lo_prec),
        }];
        out.push(match hi_prec {
            Precision::Patch => Comparator {
                op: Op::Lte,
                ver: hi_ver,
            },
            _ => Comparator {
                op: Op::Lt,
                ver: upper_bound(&hi_ver, hi_prec),
            },
        });
        return Ok(out);
    }

    let mut out = Vec::new();
    for token in normalize_spacing(part).split_whitespace() {
        out.extend(parse_comparator(token)?);
    }
    if out.is_empty() {
        return Err(format!("could not parse range '{part}'"));
    }
    Ok(out)
}

/// `>= 1.2.0` is legal npm; drop the spaces so the set splits on whitespace.
fn normalize_spacing(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if matches!(c, '>' | '<' | '=' | '^' | '~') {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        }
    }
    out
}

/// Split a hyphen range, ignoring hyphens inside prerelease identifiers.
fn split_hyphen(part: &str) -> Option<(&str, &str)> {
    let idx = part.find(" - ")?;
    Some((&part[..idx], &part[idx + 3..]))
}

fn parse_comparator(token: &str) -> std::result::Result<Vec<Comparator>, String> {
    if token == "*" || token == "x" || token == "X" {
        return Ok(Vec::new());
    }
    if let Some(rest) = token.strip_prefix('^') {
        let (ver, prec) = parse_partial_spec(rest)?;
        return Ok(vec![
            Comparator {
                op: Op::Gte,
                ver: zeroed(&ver, prec),
            },
            Comparator {
                op: Op::Lt,
                ver: caret_upper(&ver, prec),
            },
        ]);
    }
    if let Some(rest) = token.strip_prefix('~') {
        let (ver, prec) = parse_partial_spec(rest)?;
        let upper = match prec {
            Precision::Major => SemVer::exact(ver.major + 1, 0, 0),
            _ => SemVer::exact(ver.major, ver.minor + 1, 0),
        };
        return Ok(vec![
            Comparator {
                op: Op::Gte,
                ver: zeroed(&ver, prec),
            },
            Comparator {
                op: Op::Lt,
                ver: upper,
            },
        ]);
    }

    let (op, rest) = if let Some(r) = token.strip_prefix(">=") {
        (Some(Op::Gte), r)
    } else if let Some(r) = token.strip_prefix("<=") {
        (Some(Op::Lte), r)
    } else if let Some(r) = token.strip_prefix('>') {
        (Some(Op::Gt), r)
    } else if let Some(r) = token.strip_prefix('<') {
        (Some(Op::Lt), r)
    } else if let Some(r) = token.strip_prefix('=') {
        (Some(Op::Eq), r)
    } else {
        (None, token)
    };

    let (ver, prec) = parse_partial_spec(rest)?;

    let Some(op) = op else {
        // No operator: exact if fully specified, x-range otherwise.
        return Ok(match prec {
            Precision::Patch => vec![Comparator { op: Op::Eq, ver }],
            _ => vec![
                Comparator {
                    op: Op::Gte,
                    ver: zeroed(&ver, prec),
                },
                Comparator {
                    op: Op::Lt,
                    ver: upper_bound(&ver, prec),
                },
            ],
        });
    };

    // A partial version widens the bound: ">1.2" excludes all of 1.2.x.
    Ok(match (op, prec) {
        (Op::Eq, Precision::Patch) => vec![Comparator { op: Op::Eq, ver }],
        (Op::Eq, _) => vec![
            Comparator {
                op: Op::Gte,
                ver: zeroed(&ver, prec),
            },
            Comparator {
                op: Op::Lt,
                ver: upper_bound(&ver, prec),
            },
        ],
        (Op::Gt, Precision::Patch) => vec![Comparator { op: Op::Gt, ver }],
        (Op::Gt, _) => vec![Comparator {
            op: Op::Gte,
            ver: upper_bound(&ver, prec),
        }],
        (Op::Lte, Precision::Patch) => vec![Comparator { op: Op::Lte, ver }],
        (Op::Lte, _) => vec![Comparator {
            op: Op::Lt,
            ver: upper_bound(&ver, prec),
        }],
        (op, _) => vec![Comparator {
            op,
            ver: zeroed(&ver, prec),
        }],
    })
}

/// Exclusive upper bound of a caret range. `^0` and `^0.0` widen: an
/// unwritten component is not a zero the caret has to respect.
fn caret_upper(v: &SemVer, prec: Precision) -> SemVer {
    if v.major > 0 || prec == Precision::Major {
        return SemVer::exact(v.major + 1, 0, 0);
    }
    if v.minor > 0 || prec == Precision::Minor {
        return SemVer::exact(0, v.minor + 1, 0);
    }
    SemVer::exact(0, 0, v.patch + 1)
}

/// The exclusive upper bound of an x-range at the given precision.
fn upper_bound(v: &SemVer, prec: Precision) -> SemVer {
    match prec {
        Precision::Major => SemVer::exact(v.major + 1, 0, 0),
        Precision::Minor => SemVer::exact(v.major, v.minor + 1, 0),
        Precision::Patch => SemVer::exact(v.major, v.minor, v.patch),
    }
}

/// Drop a prerelease from an unwritten component, so `>=1.2` is `>=1.2.0`.
fn zeroed(v: &SemVer, prec: Precision) -> SemVer {
    match prec {
        Precision::Patch => v.clone(),
        _ => SemVer::exact(v.major, v.minor, v.patch),
    }
}

impl SemVer {
    fn exact(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
        }
    }
}

/// Parse a possibly partial version into a SemVer with missing components
/// zeroed, plus how much of it was written.
fn parse_partial_spec(s: &str) -> std::result::Result<(SemVer, Precision), String> {
    let s = s.trim().trim_start_matches('v');
    if s.is_empty() {
        return Err("empty version".into());
    }
    let is_x = |p: &str| ["x", "X", "*"].contains(&p);
    // Split the prerelease off first: it contains dots of its own.
    let (core, pre) = match s.split_once('-') {
        Some((c, p)) => (c, Some(p.split('+').next().unwrap_or(p).to_string())),
        None => (s.split('+').next().unwrap_or(s), None),
    };
    let mut parts = core.split('.');
    let first = parts.next();
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return Err("too many version components".into());
    }
    let num = |p: Option<&str>| -> std::result::Result<u64, String> {
        match p {
            None => Ok(0),
            Some(x) if is_x(x) => Ok(0),
            Some(x) => x.parse().map_err(|_| format!("invalid component '{x}'")),
        }
    };
    let precision = match (first, second, third) {
        (Some(f), _, _) if is_x(f) => Precision::Major,
        (_, Some(s), _) if is_x(s) => Precision::Major,
        (_, None, _) => Precision::Major,
        (_, _, Some(t)) if is_x(t) => Precision::Minor,
        (_, _, None) => Precision::Minor,
        _ => Precision::Patch,
    };
    Ok((
        SemVer {
            major: num(first)?,
            minor: num(second)?,
            patch: num(third)?,
            pre: if precision == Precision::Patch {
                pre
            } else {
                None
            },
        },
        precision,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> SemVer {
        SemVer::parse(s).unwrap()
    }

    #[test]
    fn test_semver_parse_and_order() {
        assert!(v("1.2.3") < v("1.2.4"));
        assert!(v("1.2.3") < v("1.10.0"));
        assert!(v("2.0.0-alpha") < v("2.0.0"));
        assert!(v("2.0.0-alpha") < v("2.0.0-beta"));
        assert_eq!(v("v1.2.3"), v("1.2.3"));
        assert_eq!(v("1.2.3+build5"), v("1.2.3"));
        assert!(SemVer::parse("1.2").is_err());
        assert!(SemVer::parse("not-a-version").is_err());
    }

    #[test]
    fn test_range_caret() {
        let r = Range::parse("^1.2.3").unwrap();
        assert!(r.matches(&v("1.2.3")));
        assert!(r.matches(&v("1.9.0")));
        assert!(!r.matches(&v("2.0.0")));
        assert!(!r.matches(&v("1.2.2")));

        // ^0.x pins the minor; ^0.0.x pins the patch.
        let r0 = Range::parse("^0.3.1").unwrap();
        assert!(r0.matches(&v("0.3.9")));
        assert!(!r0.matches(&v("0.4.0")));
        let r00 = Range::parse("^0.0.4").unwrap();
        assert!(r00.matches(&v("0.0.4")));
        assert!(!r00.matches(&v("0.0.5")));
    }

    #[test]
    fn test_range_tilde_exact_gte_any() {
        let t = Range::parse("~1.2.3").unwrap();
        assert!(t.matches(&v("1.2.9")));
        assert!(!t.matches(&v("1.3.0")));

        let e = Range::parse("1.2.3").unwrap();
        assert!(e.matches(&v("1.2.3")));
        assert!(!e.matches(&v("1.2.4")));

        let g = Range::parse(">=2.1.0").unwrap();
        assert!(g.matches(&v("3.0.0")));
        assert!(!g.matches(&v("2.0.9")));

        assert!(Range::parse("*").unwrap().matches(&v("0.0.1")));
        assert!(Range::parse("").unwrap().matches(&v("9.9.9")));
    }

    fn accepts(range: &str, version: &str) -> bool {
        Range::parse(range)
            .unwrap_or_else(|e| panic!("parse '{range}': {e}"))
            .matches(&v(version))
    }

    #[test]
    fn test_range_x_ranges_and_tags() {
        // "1" and "1.x" mean ^1.0.0; "1.2" and "1.2.x" mean ~1.2.0.
        for r in ["1", "1.x", "1.X", "1.x.x", "^1.0.0"] {
            assert!(accepts(r, "1.9.9"), "{r} should accept 1.9.9");
            assert!(!accepts(r, "2.0.0"), "{r} should reject 2.0.0");
            assert!(!accepts(r, "0.9.9"), "{r} should reject 0.9.9");
        }
        for r in ["1.2", "1.2.x", "~1.2.0"] {
            assert!(accepts(r, "1.2.9"), "{r} should accept 1.2.9");
            assert!(!accepts(r, "1.3.0"), "{r} should reject 1.3.0");
        }
        assert!(matches!(Range::parse("latest").unwrap(), Range::Tag(t) if t == "latest"));
        assert!(matches!(Range::parse("beta").unwrap(), Range::Tag(t) if t == "beta"));
    }

    #[test]
    fn test_range_comparator_sets() {
        assert!(accepts(">=1.2.0 <2.0.0", "1.9.0"));
        assert!(!accepts(">=1.2.0 <2.0.0", "2.0.0"));
        assert!(!accepts(">=1.2.0 <2.0.0", "1.1.9"));
        // Whitespace after an operator is legal npm.
        assert!(accepts(">= 1.2.0 < 2.0.0", "1.9.0"));

        assert!(accepts("<2.0.0", "1.0.0"));
        assert!(!accepts("<2.0.0", "2.0.0"));
        assert!(accepts("<=2.0.0", "2.0.0"));
        assert!(accepts(">1.2.3", "1.2.4"));
        assert!(!accepts(">1.2.3", "1.2.3"));
        assert!(accepts("=1.2.3", "1.2.3"));

        // A partial version widens the bound: ">1.2" excludes all of 1.2.x.
        assert!(!accepts(">1.2", "1.2.9"));
        assert!(accepts(">1.2", "1.3.0"));
        assert!(accepts("<=1.2", "1.2.9"));
        assert!(!accepts("<=1.2", "1.3.0"));
    }

    #[test]
    fn test_range_unions() {
        assert!(accepts("^1 || ^2", "1.5.0"));
        assert!(accepts("^1 || ^2", "2.5.0"));
        assert!(!accepts("^1 || ^2", "3.0.0"));
        assert!(accepts("1.0.0 || >=3.0.0 <4.0.0", "1.0.0"));
        assert!(accepts("1.0.0 || >=3.0.0 <4.0.0", "3.2.1"));
        assert!(!accepts("1.0.0 || >=3.0.0 <4.0.0", "2.0.0"));
        // An alternative that accepts anything collapses the whole range.
        assert_eq!(Range::parse("^1 || *").unwrap(), Range::Any);
    }

    #[test]
    fn test_range_hyphen() {
        assert!(accepts("1.2.0 - 1.4.0", "1.3.0"));
        assert!(
            accepts("1.2.0 - 1.4.0", "1.4.0"),
            "upper bound is inclusive"
        );
        assert!(!accepts("1.2.0 - 1.4.0", "1.4.1"));
        assert!(!accepts("1.2.0 - 1.4.0", "1.1.9"));
        // A partial upper bound covers the whole minor.
        assert!(accepts("1.2 - 2.3", "2.3.9"));
        assert!(!accepts("1.2 - 2.3", "2.4.0"));
    }

    #[test]
    fn test_caret_partial_widening() {
        // An unwritten component is not a zero the caret has to respect.
        assert!(accepts("^0", "0.9.9"));
        assert!(!accepts("^0", "1.0.0"));
        assert!(accepts("^0.0", "0.0.9"));
        assert!(!accepts("^0.0", "0.1.0"));
        assert!(accepts("^0.2", "0.2.9"));
        assert!(!accepts("^0.2", "0.3.0"));
        assert!(accepts("^0.0.3", "0.0.3"));
        assert!(!accepts("^0.0.3", "0.0.4"));
    }

    #[test]
    fn test_prerelease_rules() {
        // A prerelease never satisfies a range that does not name one.
        assert!(!accepts("^1.0.0", "2.0.0-beta.1"));
        assert!(!accepts("<2.0.0", "2.0.0-rc.1"));
        assert!(!accepts("*", "1.0.0-rc.1"));
        assert!(accepts("^1.0.0", "1.5.0"));
        assert!(accepts("2.0.0-beta.1", "2.0.0-beta.1"));
        assert!(accepts(">=2.0.0-beta.1 <3.0.0", "2.0.0-beta.2"));
        assert!(!accepts(">=2.0.0-beta.1 <3.0.0", "2.1.0-beta.1"));
    }

    #[test]
    fn test_prerelease_ordering_is_numeric() {
        assert!(v("1.0.0-alpha.2") < v("1.0.0-alpha.10"));
        assert!(v("1.0.0-alpha.1") < v("1.0.0-alpha.beta"));
        assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"));
        assert!(v("1.0.0-rc.1") < v("1.0.0"));
    }

    #[test]
    fn test_validate_package_name() {
        assert!(validate_package_name("lodash").is_ok());
        assert!(validate_package_name("left-pad").is_ok());
        assert!(validate_package_name("@scope/pkg").is_ok());
        assert!(validate_package_name("pkg.js").is_ok());

        assert!(validate_package_name("").is_err());
        assert!(validate_package_name("UPPER").is_err());
        assert!(validate_package_name("../evil").is_err());
        assert!(validate_package_name(".dot").is_err());
        assert!(validate_package_name("_under").is_err());
        assert!(validate_package_name("@scope").is_err());
        assert!(validate_package_name("@scope/a/b").is_err());
        assert!(validate_package_name("has space").is_err());
        assert!(validate_package_name("has/slash").is_err());
    }

    #[test]
    fn test_verify_integrity() {
        use base64::Engine;
        use sha2::Digest;
        let data = b"tarball bytes";
        let digest = sha2::Sha512::digest(data);
        let sri = format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(digest)
        );
        assert!(verify_integrity(data, &sri).is_ok());
        assert!(verify_integrity(b"other bytes", &sri).is_err());
        assert!(verify_integrity(data, "sha1-abcdef").is_err());
    }

    /// Build a gzip'd npm-style tarball in memory from (path, contents) pairs.
    /// Paths containing `..` are written by forging the raw GNU header name
    /// (the tar crate's safe API refuses them), so extraction hardening can
    /// be tested against genuinely malicious archives.
    fn make_tarball(files: &[(&str, &[u8])]) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut builder = tar::Builder::new(gz);
        for (path, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            if path.contains("..") {
                let gnu = header.as_gnu_mut().unwrap();
                gnu.name[..path.len()].copy_from_slice(path.as_bytes());
                header.set_cksum();
                builder.append(&header, *contents).unwrap();
            } else {
                header.set_cksum();
                builder.append_data(&mut header, path, *contents).unwrap();
            }
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn test_extract_tarball_strips_root_and_writes_files() {
        let tmp = tempfile::tempdir().unwrap();
        let tb = make_tarball(&[
            ("package/package.json", br#"{"version":"1.0.0"}"# as &[u8]),
            ("package/lib/index.js", b"module.exports = 1;"),
        ]);
        extract_tarball(&tb, tmp.path()).unwrap();
        assert!(tmp.path().join("package.json").exists());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("lib/index.js")).unwrap(),
            "module.exports = 1;"
        );
    }

    #[test]
    fn test_extract_tarball_ignores_traversal_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("out");
        let tb = make_tarball(&[
            ("package/ok.js", b"1" as &[u8]),
            ("package/../../escape.js", b"pwned"),
        ]);
        extract_tarball(&tb, &dest).unwrap();
        assert!(dest.join("ok.js").exists());
        assert!(!tmp.path().join("escape.js").exists());
        assert!(!dest.parent().unwrap().join("escape.js").exists());
    }

    #[test]
    fn test_reject_native_artifacts() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("build")).unwrap();
        std::fs::write(tmp.path().join("index.js"), "1").unwrap();
        assert!(reject_native_artifacts("p", "1.0.0", tmp.path()).is_ok());

        std::fs::write(tmp.path().join("build/binding.gyp"), "{}").unwrap();
        let err = reject_native_artifacts("p", "1.0.0", tmp.path()).unwrap_err();
        assert!(err.to_string().contains("native-binding"));
    }

    // ── Fake-registry end-to-end tests ────────────────────────────

    /// Drop-guard around a tiny_http server serving a fixed path → body map.
    struct FakeRegistry {
        server: std::sync::Arc<tiny_http::Server>,
        #[allow(dead_code)]
        url: String,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for FakeRegistry {
        fn drop(&mut self) {
            self.server.unblock();
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    /// Bind a server, then serve `docs` until the guard drops. Binding first
    /// is what lets metadata embed absolute tarball URLs with the real port.
    fn serve_docs(
        server: std::sync::Arc<tiny_http::Server>,
        url: String,
        docs: HashMap<String, Vec<u8>>,
    ) -> FakeRegistry {
        let srv = server.clone();
        let handle = std::thread::spawn(move || {
            for request in srv.incoming_requests() {
                let path = request.url().trim_start_matches('/').to_string();
                match docs.get(&path) {
                    Some(body) => {
                        let _ = request.respond(tiny_http::Response::from_data(body.clone()));
                    }
                    None => {
                        let _ = request.respond(
                            tiny_http::Response::from_string("nope").with_status_code(404),
                        );
                    }
                }
            }
        });
        FakeRegistry {
            server,
            url,
            handle: Some(handle),
        }
    }

    fn sri_for(data: &[u8]) -> String {
        use base64::Engine;
        use sha2::Digest;
        format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha512::digest(data))
        )
    }

    /// Registry metadata for one package version.
    fn version_doc(
        name: &str,
        version: &str,
        registry_url_placeholder: &str,
        tarball: &[u8],
        deps: &[(&str, &str)],
        has_install_script: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "version": version,
            "dependencies": deps.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_,_>>(),
            "dist": {
                "tarball": format!("{registry_url_placeholder}/tarballs/{name}-{version}.tgz"),
                "integrity": sri_for(tarball),
            },
            "hasInstallScript": has_install_script,
        })
    }

    fn pkg_tarball(name: &str, version: &str, main_src: &str) -> Vec<u8> {
        let pkg_json = format!(r#"{{"name":"{name}","version":"{version}","main":"index.js"}}"#);
        make_tarball(&[
            ("package/package.json", pkg_json.as_bytes()),
            ("package/index.js", main_src.as_bytes()),
        ])
    }

    /// One-package registry: greet@1.4.2 with no deps.
    fn simple_registry() -> (FakeRegistry, tempfile::TempDir, Vendor) {
        let tb = pkg_tarball("greet", "1.4.2", "module.exports = n => 'hello ' + n;");
        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}", server.server_addr());
        let meta = serde_json::json!({
            "dist-tags": {"latest": "1.4.2"},
            "versions": {"1.4.2": version_doc("greet", "1.4.2", &url, &tb, &[], false)},
        });
        let mut docs = HashMap::new();
        docs.insert("greet".to_string(), meta.to_string().into_bytes());
        docs.insert("tarballs/greet-1.4.2.tgz".to_string(), tb);

        let registry = serve_docs(server, url.clone(), docs);
        let cache = tempfile::tempdir().unwrap();
        let vendor = Vendor::with_cache_dir(&url, cache.path().join("npm"));
        (registry, cache, vendor)
    }

    #[test]
    fn test_vendor_simple_package_end_to_end() {
        let (_reg, _cache, vendor) = simple_registry();
        let session = tempfile::tempdir().unwrap();

        let deps = HashMap::from([("greet".to_string(), "^1.0.0".to_string())]);
        vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap();

        let installed = session.path().join("node_modules/greet");
        assert!(installed.join("package.json").exists());
        let src = std::fs::read_to_string(installed.join("index.js")).unwrap();
        assert!(src.contains("hello"));
    }

    #[test]
    fn test_vendor_returns_lockfile() {
        let (_reg, _cache, vendor) = simple_registry();
        let session = tempfile::tempdir().unwrap();

        let deps = HashMap::from([("greet".to_string(), "^1.0.0".to_string())]);
        let lock = vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap();

        let entry = lock
            .get("node_modules/greet")
            .expect("lockfile records the install path");
        assert_eq!(entry.version, "1.4.2");
        assert!(entry.integrity.starts_with("sha512-"));
        assert!(entry.resolved.ends_with("greet-1.4.2.tgz"));
    }

    #[test]
    fn test_vendor_locked_replays_without_the_registry() {
        let cache = tempfile::tempdir().unwrap();
        let cache_dir = cache.path().join("npm");

        // Resolve once to get a lockfile and warm the cache.
        let lock = {
            let (_reg, _c, _v) = simple_registry();
            let url = _v.registry.clone();
            let vendor = Vendor::with_cache_dir(&url, cache_dir.clone());
            let first = tempfile::tempdir().unwrap();
            let deps = HashMap::from([("greet".to_string(), "^1.0.0".to_string())]);
            vendor
                .vendor(&deps, first.path(), &ResourceLimits::default())
                .unwrap()
        };
        // The registry is now down; replay must still work.
        let offline = Vendor::with_cache_dir("http://127.0.0.1:1", cache_dir);
        let session = tempfile::tempdir().unwrap();
        offline
            .vendor_locked(&lock, session.path(), &ResourceLimits::default())
            .unwrap();

        let installed = session.path().join("node_modules/greet");
        assert_eq!(installed_version(&installed).as_deref(), Some("1.4.2"));
        assert!(installed.join("index.js").exists());
    }

    #[test]
    fn test_vendor_locked_rejects_unsafe_paths() {
        let vendor = Vendor::with_cache_dir("http://127.0.0.1:1", PathBuf::from("/nonexistent"));
        let session = tempfile::tempdir().unwrap();
        let entry = LockEntry {
            version: "1.0.0".into(),
            resolved: "http://127.0.0.1:1/x.tgz".into(),
            integrity: "sha512-x".into(),
        };
        for path in [
            "../escape",
            "/abs/path",
            "node_modules/../../escape",
            "not_node_modules/greet",
            "node_modules",
            "greet",
        ] {
            let lock = Lockfile::from([(path.to_string(), entry.clone())]);
            let err = vendor
                .vendor_locked(&lock, session.path(), &ResourceLimits::default())
                .unwrap_err();
            assert_eq!(err.status_code(), 400, "path '{path}' should be refused");
        }

        // Well-formed shapes get past validation and fail later, differently.
        for path in ["node_modules/greet", "node_modules/a/node_modules/b"] {
            let lock = Lockfile::from([(path.to_string(), entry.clone())]);
            let err = vendor
                .vendor_locked(&lock, session.path(), &ResourceLimits::default())
                .unwrap_err();
            assert!(
                !err.to_string().contains("Invalid lockfile path"),
                "path '{path}' should be accepted"
            );
        }
    }

    #[test]
    fn test_vendor_skips_already_satisfied() {
        let (_reg, _cache, vendor) = simple_registry();
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("greet".to_string(), "^1.0.0".to_string())]);
        vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap();

        // Poison the installed copy; a second vendor must not overwrite it
        // because the version still satisfies the range.
        let marker = session.path().join("node_modules/greet/marker.txt");
        std::fs::write(&marker, "kept").unwrap();
        vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap();
        assert!(marker.exists());
    }

    #[test]
    fn test_vendor_unknown_package_fails_clearly() {
        let (_reg, _cache, vendor) = simple_registry();
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("missing-pkg".to_string(), "*".to_string())]);
        let err = vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap_err();
        assert!(err.to_string().contains("missing-pkg"));
    }

    #[test]
    fn test_vendor_no_satisfying_version() {
        let (_reg, _cache, vendor) = simple_registry();
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("greet".to_string(), "^2.0.0".to_string())]);
        let err = vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("No version"));
    }

    #[test]
    fn test_vendor_transitive_deps_nested_layout() {
        // app@1.0.0 depends on lib@^1.0.0; expect app's copy of lib nested
        // (or found by walk-up — here root has no lib, so it must nest).
        let lib_tb = pkg_tarball("lib", "1.1.0", "module.exports = 41;");
        let app_tb = pkg_tarball("app", "1.0.0", "module.exports = require('lib') + 1;");

        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}", server.server_addr());
        let mut docs = HashMap::new();
        docs.insert(
            "app".to_string(),
            serde_json::json!({
                "dist-tags": {"latest": "1.0.0"},
                "versions": {"1.0.0": version_doc("app", "1.0.0", &url, &app_tb, &[("lib", "^1.0.0")], false)},
            })
            .to_string()
            .into_bytes(),
        );
        docs.insert(
            "lib".to_string(),
            serde_json::json!({
                "dist-tags": {"latest": "1.1.0"},
                "versions": {"1.1.0": version_doc("lib", "1.1.0", &url, &lib_tb, &[], false)},
            })
            .to_string()
            .into_bytes(),
        );
        docs.insert("tarballs/app-1.0.0.tgz".to_string(), app_tb);
        docs.insert("tarballs/lib-1.1.0.tgz".to_string(), lib_tb);

        let _reg = serve_docs(server, url.clone(), docs);

        let cache = tempfile::tempdir().unwrap();
        let vendor = Vendor::with_cache_dir(&url, cache.path().join("npm"));
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("app".to_string(), "latest".to_string())]);
        let lock = vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap();

        assert!(session
            .path()
            .join("node_modules/app/package.json")
            .exists());
        assert!(session
            .path()
            .join("node_modules/app/node_modules/lib/package.json")
            .exists());

        // The transitive package is locked at its nested path.
        assert_eq!(
            lock.keys().collect::<Vec<_>>(),
            vec!["node_modules/app", "node_modules/app/node_modules/lib"]
        );
        assert_eq!(lock["node_modules/app/node_modules/lib"].version, "1.1.0");
    }

    #[test]
    fn test_vendor_rejects_install_scripts() {
        let tb = pkg_tarball("native-ish", "2.0.0", "module.exports = 1;");
        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}", server.server_addr());
        let mut docs = HashMap::new();
        docs.insert(
            "native-ish".to_string(),
            serde_json::json!({
                "dist-tags": {"latest": "2.0.0"},
                "versions": {"2.0.0": version_doc("native-ish", "2.0.0", &url, &tb, &[], true)},
            })
            .to_string()
            .into_bytes(),
        );
        let _reg = serve_docs(server, url.clone(), docs);

        let cache = tempfile::tempdir().unwrap();
        let vendor = Vendor::with_cache_dir(&url, cache.path().join("npm"));
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("native-ish".to_string(), "*".to_string())]);
        let err = vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("install script"));
    }

    #[test]
    fn test_vendor_integrity_mismatch_rejected() {
        let tb = pkg_tarball("greet", "1.4.2", "module.exports = 1;");
        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}", server.server_addr());
        let mut meta_doc = version_doc("greet", "1.4.2", &url, &tb, &[], false);
        // Corrupt the advertised integrity.
        meta_doc["dist"]["integrity"] = serde_json::json!(sri_for(b"different bytes"));
        let mut docs = HashMap::new();
        docs.insert(
            "greet".to_string(),
            serde_json::json!({
                "dist-tags": {"latest": "1.4.2"},
                "versions": {"1.4.2": meta_doc},
            })
            .to_string()
            .into_bytes(),
        );
        docs.insert("tarballs/greet-1.4.2.tgz".to_string(), tb);
        let _reg = serve_docs(server, url.clone(), docs);

        let cache = tempfile::tempdir().unwrap();
        let vendor = Vendor::with_cache_dir(&url, cache.path().join("npm"));
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("greet".to_string(), "1.4.2".to_string())]);
        let err = vendor
            .vendor(&deps, session.path(), &ResourceLimits::default())
            .unwrap_err();
        assert_eq!(err.status_code(), 400);
        assert!(err.to_string().contains("Integrity"));
    }

    #[test]
    fn test_vendor_disk_limit_enforced() {
        let (_reg, _cache, vendor) = simple_registry();
        let session = tempfile::tempdir().unwrap();
        let deps = HashMap::from([("greet".to_string(), "*".to_string())]);
        let limits = ResourceLimits {
            max_disk_bytes: Some(10), // absurdly small
            ..ResourceLimits::default()
        };
        let err = vendor.vendor(&deps, session.path(), &limits).unwrap_err();
        assert!(err.to_string().contains("Disk usage limit"));
    }
}
