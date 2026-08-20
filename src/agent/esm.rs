//! Agent mode: ESM → CommonJS lowering for vendored npm packages.
//!
//! The runtime's module wrapper is CommonJS-only and its resolver probes
//! `.js`/`.json` but never `.mjs`, so an ESM package cannot load at all.
//! Rewriting the sources reuses the swc stage the TypeScript path already
//! runs, rather than waiting on runtime-side ESM support.

use crate::agent::api::ApiError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Marks the temporary `.ts` copies handed to the transpiler, which accepts
/// `.ts`/`.tsx` inputs only. TypeScript is a superset, so the parse is the
/// one the file would get anyway.
const TMP_SUFFIX: &str = ".wasmrun-esm";

/// Entry-point conditions, in priority order. `import` is accepted last
/// because it names ESM, which is what this module lowers anyway; `browser`
/// and `types` are deliberately absent.
const EXPORT_CONDITIONS: [&str; 4] = ["require", "node", "default", "import"];

/// A vendored package whose sources need lowering. Paths are relative to the
/// session root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsmPackage {
    pub dir: String,
    pub name: String,
    pub version: String,
    pub files: Vec<String>,
    /// `main` to write when the runtime would not otherwise find an entry.
    pub main: Option<String>,
    /// `#alias` → file within the package, both as written.
    pub imports: Vec<(String, String)>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageJson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(rename = "type", default)]
    module_type: Option<String>,
    #[serde(default)]
    main: Option<String>,
    #[serde(default)]
    exports: Option<serde_json::Value>,
    #[serde(default)]
    imports: Option<serde_json::Value>,
}

/// Every package under `work_dir/node_modules` that publishes ESM, nested
/// ones included.
pub fn scan(work_dir: &Path) -> Vec<EsmPackage> {
    let mut out = Vec::new();
    scan_node_modules(work_dir, &work_dir.join("node_modules"), &mut out);
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out
}

fn scan_node_modules(root: &Path, node_modules: &Path, out: &mut Vec<EsmPackage>) {
    let Ok(entries) = std::fs::read_dir(node_modules) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        // A scoped directory (`@scope`) holds packages rather than being one.
        if name.starts_with('@') {
            scan_node_modules(root, &path, out);
            continue;
        }
        if let Some(pkg) = inspect_package(root, &path) {
            out.push(pkg);
        }
        scan_node_modules(root, &path.join("node_modules"), out);
    }
}

/// Describe one package directory, or `None` when it needs no lowering.
fn inspect_package(root: &Path, dir: &Path) -> Option<EsmPackage> {
    let manifest = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let pkg: PackageJson = serde_json::from_str(&manifest).ok()?;
    let is_module = pkg.module_type.as_deref() == Some("module");

    let mut sources = Vec::new();
    collect_sources(dir, is_module, &mut sources);
    if sources.is_empty() {
        return None;
    }

    // `x.mjs` lowers onto `x.js`, so drop it when that sibling exists: the
    // `.js` is the one the resolver loads.
    sources.retain(|p| match p.extension().and_then(|e| e.to_str()) {
        Some("mjs") => !p.with_extension("js").exists(),
        _ => true,
    });
    if sources.is_empty() {
        return None;
    }

    let rel_dir = rel_str(root, dir)?;
    let mut files: Vec<String> = sources.iter().filter_map(|p| rel_str(root, p)).collect();
    files.sort();

    let main = entry_fix(&pkg);
    let imports = subpath_imports(&pkg);
    Some(EsmPackage {
        dir: rel_dir,
        name: pkg.name,
        version: pkg.version,
        files,
        main,
        imports,
    })
}

/// The package's own `#alias` subpath imports, resolved to files. Wildcards
/// are skipped: materializing one means enumerating every file it matches.
fn subpath_imports(pkg: &PackageJson) -> Vec<(String, String)> {
    let Some(serde_json::Value::Object(map)) = &pkg.imports else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = map
        .iter()
        .filter(|(alias, _)| alias.starts_with('#') && !alias.contains('*'))
        .filter_map(|(alias, value)| {
            let target = resolve_conditions(value, 0)?;
            (!target.contains('*')).then(|| (alias.clone(), lowered_name(&target)))
        })
        .collect();
    out.sort();
    out
}

/// ESM files in `dir`: always `.mjs`, and `.js` too under `"type": "module"`.
fn collect_sources(dir: &Path, is_module: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() != "node_modules" {
                collect_sources(&path, is_module, out);
            }
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("mjs") => out.push(path),
            Some("js") if is_module => out.push(path),
            _ => {}
        }
    }
}

/// A `main` to write when the runtime would otherwise not resolve the
/// package: the resolver reads `main` then `index.js` and knows nothing about
/// `exports`, which is where ESM-only packages put their entry. Additive, so a
/// resolver that later learns `exports` keeps preferring the map.
fn entry_fix(pkg: &PackageJson) -> Option<String> {
    if let Some(main) = &pkg.main {
        let lowered = lowered_name(main);
        if lowered != *main {
            return Some(lowered);
        }
        return None;
    }
    let from_exports = pkg.exports.as_ref().and_then(resolve_root_export);
    match from_exports {
        // `index.js` is the resolver's own fallback; no `main` needed.
        Some(target) if strip_dot_slash(&target) == "index.js" => None,
        Some(target) => Some(lowered_name(&target)),
        None => None,
    }
}

/// The file the `"."` entry of an `exports` map points at.
fn resolve_root_export(exports: &serde_json::Value) -> Option<String> {
    let root = match exports {
        // Sugar: `"exports": "./index.js"` means the root entry directly.
        serde_json::Value::String(s) => return Some(s.clone()),
        serde_json::Value::Object(map) => {
            // An object is either subpaths (keys start with `.`) or bare
            // conditions for the root entry.
            if map.keys().any(|k| k.starts_with('.')) {
                map.get(".")?
            } else {
                exports
            }
        }
        _ => return None,
    };
    resolve_conditions(root, 0)
}

fn resolve_conditions(value: &serde_json::Value, depth: usize) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => EXPORT_CONDITIONS
            .iter()
            .find_map(|c| map.get(*c).and_then(|v| resolve_conditions(v, depth + 1))),
        // First usable branch of an alternatives array, as node does.
        serde_json::Value::Array(items) => {
            items.iter().find_map(|v| resolve_conditions(v, depth + 1))
        }
        _ => None,
    }
}

/// The name a source ends up with after lowering.
fn lowered_name(path: &str) -> String {
    match path.strip_suffix(".mjs") {
        Some(stem) => format!("{stem}.js"),
        None => path.to_string(),
    }
}

fn strip_dot_slash(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

fn rel_str(root: &Path, path: &Path) -> Option<String> {
    Some(path.strip_prefix(root).ok()?.to_string_lossy().into_owned())
}

/// The temporary `.ts` path a source is transpiled through.
pub fn tmp_input(file: &str) -> String {
    let stem = file
        .strip_suffix(".mjs")
        .or_else(|| file.strip_suffix(".js"))
        .unwrap_or(file);
    format!("{stem}{TMP_SUFFIX}.ts")
}

/// The `.js` the transpiler emits for [`tmp_input`].
pub fn tmp_output(file: &str) -> String {
    let stem = file
        .strip_suffix(".mjs")
        .or_else(|| file.strip_suffix(".js"))
        .unwrap_or(file);
    format!("{stem}{TMP_SUFFIX}.js")
}

/// Copy each source to its temporary `.ts` path, returning the transpiler
/// inputs.
pub fn stage_inputs(
    work_dir: &Path,
    files: &[String],
) -> std::result::Result<Vec<String>, ApiError> {
    let mut inputs = Vec::with_capacity(files.len());
    for file in files {
        let input = tmp_input(file);
        std::fs::copy(work_dir.join(file), work_dir.join(&input)).map_err(|e| {
            ApiError::Internal(format!("Failed to stage '{file}' for ESM lowering: {e}"))
        })?;
        inputs.push(input);
    }
    Ok(inputs)
}

/// Move each transpiled result over its source and clean up the staging
/// files. A lowered `.mjs` is removed: the resolver cannot load it anyway.
pub fn commit_outputs(work_dir: &Path, files: &[String]) -> std::result::Result<(), ApiError> {
    for file in files {
        let emitted = work_dir.join(tmp_output(file));
        let target = work_dir.join(lowered_name(file));
        std::fs::rename(&emitted, &target)
            .map_err(|e| ApiError::Internal(format!("Failed to install lowered '{file}': {e}")))?;
        let _ = std::fs::remove_file(work_dir.join(tmp_input(file)));
        if file.ends_with(".mjs") {
            let _ = std::fs::remove_file(work_dir.join(file));
        }
    }
    Ok(())
}

/// Remove staging files left by a failed lowering, so the program never sees
/// them.
pub fn clean_staging(work_dir: &Path, files: &[String]) {
    for file in files {
        let _ = std::fs::remove_file(work_dir.join(tmp_input(file)));
        let _ = std::fs::remove_file(work_dir.join(tmp_output(file)));
    }
}

/// Bring a lowered package's `package.json` in line with what it now holds,
/// preserving every other field: `"type": "module"` is dropped (true
/// afterwards, and what stops [`scan`] re-lowering it next run), `main` is
/// written when the entry needed one, and `exports`/`imports` targets are
/// pointed at the files lowering left behind.
pub fn finalize_manifest(work_dir: &Path, pkg: &EsmPackage) -> std::result::Result<(), ApiError> {
    let pkg_dir = work_dir.join(&pkg.dir);
    let path = pkg_dir.join("package.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| ApiError::Internal(format!("Failed to read {}: {e}", path.display())))?;
    let mut json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::Internal(format!("Invalid package.json in '{}': {e}", pkg.dir)))?;
    let serde_json::Value::Object(map) = &mut json else {
        return Ok(());
    };
    if map.get("type").and_then(|t| t.as_str()) == Some("module") {
        map.remove("type");
    }
    if let Some(main) = &pkg.main {
        map.insert("main".into(), serde_json::Value::String(main.clone()));
    }
    for field in ["exports", "imports"] {
        if let Some(value) = map.get_mut(field) {
            relink_lowered_targets(value, &pkg_dir);
        }
    }
    let rendered = serde_json::to_string_pretty(&json)
        .map_err(|e| ApiError::Internal(format!("Failed to render package.json: {e}")))?;
    std::fs::write(&path, rendered)
        .map_err(|e| ApiError::Internal(format!("Failed to write {}: {e}", path.display())))
}

/// Point every `.mjs` target in an `exports`/`imports` map at the `.js` that
/// lowering produced, at any depth: condition objects, subpath maps and
/// alternatives arrays all hold targets as plain strings.
///
/// The runtime's resolver consults `exports` before `main` and probes only
/// `.js`/`.json`, so a map left pointing at a `.mjs` sends it at a file
/// lowering renamed away, and `main` never gets its turn. Only targets whose
/// lowered sibling is actually on disk are rewritten, so an entry this pass
/// never touched stays exactly as the package wrote it.
fn relink_lowered_targets(value: &mut serde_json::Value, pkg_dir: &Path) {
    match value {
        serde_json::Value::String(target) => {
            let lowered = lowered_name(target);
            if lowered != *target && pkg_dir.join(strip_dot_slash(&lowered)).exists() {
                *target = lowered;
            }
        }
        serde_json::Value::Object(map) => {
            for nested in map.values_mut() {
                relink_lowered_targets(nested, pkg_dir);
            }
        }
        serde_json::Value::Array(items) => {
            for nested in items.iter_mut() {
                relink_lowered_targets(nested, pkg_dir);
            }
        }
        _ => {}
    }
}

/// Attribute a transpiler error to the package owning the offending file.
/// swc reports the path it was given, so the owner is recoverable from the
/// message rather than by transpiling each package separately.
pub fn blame(packages: &[EsmPackage], error: &str) -> String {
    let owner = packages.iter().find(|p| error.contains(&p.dir));
    match owner {
        Some(p) if !p.name.is_empty() => format!(
            "Failed to convert ES module package '{}@{}' to CommonJS: {}",
            p.name, p.version, error
        ),
        _ => format!("Failed to convert an ES module dependency to CommonJS: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a package into `root/node_modules/<name>`.
    fn write_pkg(root: &Path, name: &str, manifest: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = root.join("node_modules").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), manifest).unwrap();
        for (path, content) in files {
            let full = dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content).unwrap();
        }
        dir
    }

    #[test]
    fn test_scan_finds_type_module_package() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "esm-pkg",
            r#"{"name":"esm-pkg","version":"1.0.0","type":"module"}"#,
            &[
                ("index.js", "export const x = 1;"),
                ("lib/util.js", "export const y = 2;"),
            ],
        );

        let found = scan(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "esm-pkg");
        assert_eq!(found[0].version, "1.0.0");
        assert_eq!(
            found[0].files,
            vec![
                "node_modules/esm-pkg/index.js",
                "node_modules/esm-pkg/lib/util.js"
            ]
        );
        // index.js is the resolver's own fallback, so no main is needed.
        assert_eq!(found[0].main, None);
    }

    #[test]
    fn test_scan_skips_commonjs_package() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "cjs-pkg",
            r#"{"name":"cjs-pkg","version":"2.0.0","main":"index.js"}"#,
            &[("index.js", "module.exports = 1;")],
        );
        assert!(scan(root.path()).is_empty());
    }

    #[test]
    fn test_scan_finds_mjs_in_commonjs_package() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "dual",
            r#"{"name":"dual","version":"1.0.0","main":"index.cjs"}"#,
            &[
                ("index.cjs", "module.exports = 1;"),
                ("extra.mjs", "export default 2;"),
            ],
        );
        let found = scan(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].files, vec!["node_modules/dual/extra.mjs"]);
    }

    #[test]
    fn test_scan_keeps_js_over_sibling_mjs() {
        // A dual-published file pair: the resolver loads the `.js`, so the
        // `.mjs` must not lower on top of it.
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "dual",
            r#"{"name":"dual","version":"1.0.0","main":"index.js"}"#,
            &[
                ("index.js", "module.exports = 1;"),
                ("index.mjs", "export default 1;"),
            ],
        );
        assert!(scan(root.path()).is_empty());
    }

    #[test]
    fn test_scan_walks_scoped_and_nested_packages() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "@scope/inner",
            r#"{"name":"@scope/inner","version":"1.0.0","type":"module"}"#,
            &[("index.js", "export const a = 1;")],
        );
        let outer = write_pkg(
            root.path(),
            "outer",
            r#"{"name":"outer","version":"1.0.0"}"#,
            &[("index.js", "module.exports = 1;")],
        );
        std::fs::create_dir_all(outer.join("node_modules/deep")).unwrap();
        std::fs::write(
            outer.join("node_modules/deep/package.json"),
            r#"{"name":"deep","version":"3.0.0","type":"module"}"#,
        )
        .unwrap();
        std::fs::write(
            outer.join("node_modules/deep/index.js"),
            "export const b = 2;",
        )
        .unwrap();

        let found = scan(root.path());
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["@scope/inner", "deep"]);
    }

    #[test]
    fn test_scan_ignores_nested_node_modules_as_own_files() {
        let root = tempfile::tempdir().unwrap();
        let outer = write_pkg(
            root.path(),
            "outer",
            r#"{"name":"outer","version":"1.0.0","type":"module"}"#,
            &[("index.js", "export const a = 1;")],
        );
        std::fs::create_dir_all(outer.join("node_modules/deep")).unwrap();
        std::fs::write(
            outer.join("node_modules/deep/package.json"),
            r#"{"name":"deep","version":"3.0.0","type":"module"}"#,
        )
        .unwrap();
        std::fs::write(
            outer.join("node_modules/deep/index.js"),
            "export const b = 2;",
        )
        .unwrap();

        let found = scan(root.path());
        let outer_pkg = found.iter().find(|p| p.name == "outer").unwrap();
        assert_eq!(outer_pkg.files, vec!["node_modules/outer/index.js"]);
    }

    #[test]
    fn test_entry_fix_from_exports_map() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "mapped",
            r#"{"name":"mapped","version":"5.0.0","type":"module",
                "exports":{".":{"types":"./x.d.ts","browser":"./b.js","default":"./dist/main.js"}}}"#,
            &[("dist/main.js", "export const x = 1;")],
        );
        let found = scan(root.path());
        assert_eq!(found[0].main.as_deref(), Some("./dist/main.js"));
    }

    #[test]
    fn test_entry_fix_prefers_require_condition() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "mapped",
            r#"{"name":"mapped","version":"5.0.0","type":"module",
                "exports":{".":{"import":"./esm/main.js","require":"./cjs/main.js"}}}"#,
            &[
                ("esm/main.js", "export const x = 1;"),
                ("cjs/main.js", "module.exports = 1;"),
            ],
        );
        let found = scan(root.path());
        assert_eq!(found[0].main.as_deref(), Some("./cjs/main.js"));
    }

    #[test]
    fn test_entry_fix_rewrites_mjs_main() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "mjs-main",
            r#"{"name":"mjs-main","version":"1.0.0","main":"./dist/index.mjs"}"#,
            &[("dist/index.mjs", "export default 1;")],
        );
        let found = scan(root.path());
        assert_eq!(found[0].main.as_deref(), Some("./dist/index.js"));
    }

    #[test]
    fn test_entry_fix_absent_when_main_already_usable() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "has-main",
            r#"{"name":"has-main","version":"1.0.0","type":"module","main":"./dist/index.js"}"#,
            &[("dist/index.js", "export const x = 1;")],
        );
        assert_eq!(scan(root.path())[0].main, None);
    }

    #[test]
    fn test_exports_string_sugar() {
        let exports = serde_json::json!("./index.js");
        assert_eq!(resolve_root_export(&exports).as_deref(), Some("./index.js"));
    }

    #[test]
    fn test_exports_bare_conditions_without_subpaths() {
        let exports = serde_json::json!({"node": "./n.js", "default": "./d.js"});
        assert_eq!(resolve_root_export(&exports).as_deref(), Some("./n.js"));
    }

    #[test]
    fn test_exports_array_alternatives() {
        let exports = serde_json::json!({".": [{"unknown": "./u.js"}, "./fallback.js"]});
        assert_eq!(
            resolve_root_export(&exports).as_deref(),
            Some("./fallback.js")
        );
    }

    #[test]
    fn test_exports_subpath_only_map_has_no_root() {
        let exports = serde_json::json!({"./sub": "./sub.js"});
        assert_eq!(resolve_root_export(&exports), None);
    }

    #[test]
    fn test_tmp_paths_round_trip() {
        assert_eq!(
            tmp_input("node_modules/a/x.js"),
            "node_modules/a/x.wasmrun-esm.ts"
        );
        assert_eq!(
            tmp_output("node_modules/a/x.js"),
            "node_modules/a/x.wasmrun-esm.js"
        );
        assert_eq!(tmp_input("a/x.mjs"), "a/x.wasmrun-esm.ts");
        assert_eq!(lowered_name("a/x.mjs"), "a/x.js");
        assert_eq!(lowered_name("a/x.js"), "a/x.js");
    }

    #[test]
    fn test_stage_and_commit_replace_source() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("node_modules/p");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.mjs"), "export default 1;").unwrap();

        let files = vec!["node_modules/p/index.mjs".to_string()];
        let inputs = stage_inputs(root.path(), &files).unwrap();
        assert_eq!(inputs, vec!["node_modules/p/index.wasmrun-esm.ts"]);
        assert!(dir.join("index.wasmrun-esm.ts").exists());

        // Stand in for the transpiler's emit.
        std::fs::write(dir.join("index.wasmrun-esm.js"), "module.exports = 1;").unwrap();
        commit_outputs(root.path(), &files).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.join("index.js")).unwrap(),
            "module.exports = 1;"
        );
        assert!(
            !dir.join("index.mjs").exists(),
            "lowered .mjs should be removed"
        );
        assert!(!dir.join("index.wasmrun-esm.ts").exists());
    }

    #[test]
    fn test_clean_staging_removes_both_sides() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("node_modules/p");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.wasmrun-esm.ts"), "x").unwrap();
        std::fs::write(dir.join("index.wasmrun-esm.js"), "x").unwrap();

        clean_staging(root.path(), &["node_modules/p/index.js".to_string()]);
        assert!(!dir.join("index.wasmrun-esm.ts").exists());
        assert!(!dir.join("index.wasmrun-esm.js").exists());
    }

    #[test]
    fn test_finalize_manifest_writes_main_and_preserves_other_fields() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "p",
            r#"{"name":"p","version":"1.0.0","type":"module","sideEffects":false}"#,
            &[("index.js", "export const x = 1;")],
        );
        let pkg = EsmPackage {
            dir: "node_modules/p".into(),
            name: "p".into(),
            version: "1.0.0".into(),
            files: vec![],
            main: Some("./dist/main.js".into()),
            imports: vec![],
        };
        finalize_manifest(root.path(), &pkg).unwrap();

        let text =
            std::fs::read_to_string(root.path().join("node_modules/p/package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["main"], "./dist/main.js");
        assert_eq!(json["name"], "p");
        assert_eq!(json["sideEffects"], false);
    }

    #[test]
    fn test_finalize_manifest_drops_type_module_so_rescan_is_a_no_op() {
        let root = tempfile::tempdir().unwrap();
        write_pkg(
            root.path(),
            "p",
            r#"{"name":"p","version":"1.0.0","type":"module"}"#,
            &[("index.js", "export const x = 1;")],
        );
        let pkg = scan(root.path()).remove(0);
        finalize_manifest(root.path(), &pkg).unwrap();

        let text =
            std::fs::read_to_string(root.path().join("node_modules/p/package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(json.get("type").is_none(), "type: module should be dropped");
        assert!(
            scan(root.path()).is_empty(),
            "a lowered package must not be picked up for lowering again"
        );
    }

    #[test]
    fn test_blame_names_the_owning_package() {
        let packages = vec![EsmPackage {
            dir: "node_modules/broken".into(),
            name: "broken".into(),
            version: "2.1.0".into(),
            files: vec!["node_modules/broken/index.js".into()],
            main: None,
            imports: vec![],
        }];
        let msg = blame(
            &packages,
            "node_modules/broken/index.js:3:9: Expression expected",
        );
        assert!(msg.contains("'broken@2.1.0'"), "{msg}");

        let unknown = blame(&packages, "some other failure");
        assert!(unknown.contains("an ES module dependency"), "{unknown}");
    }
}
