//! [Server Mode] The app port: a web app's own page, its static files and its build output
//!
//! A path resolves against the build output under `/pkg/` first (the wasm-pack layout a page
//! imports from), then the project directory, then the build output at the root. So a stale
//! `pkg/` left in the project never shadows the build this session just made.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tiny_http::{Header, Method, Request, Response};

use super::dev::{format_bytes, Artifacts, DevState, LogLevel, LogSource};
use super::utils::determine_content_type;

const CLIENT_PATH: &str = "/__wasmrun/client.js";
const BUILD_PATH: &str = "/__wasmrun/build";

/// Injected into every page the app port serves. Forwards the page's console to the control
/// center when framed by it, and reloads the page when a build lands
const CLIENT_JS: &str = r#"(function () {
  var allowed = UI_ORIGINS;
  var parentOrigin = null;
  if (window.parent !== window) {
    var ancestors = location.ancestorOrigins;
    var candidate = ancestors && ancestors.length ? ancestors[0] : null;
    if (!candidate && document.referrer) {
      try { candidate = new URL(document.referrer).origin; } catch (e) {}
    }
    if (allowed.indexOf(candidate) !== -1) parentOrigin = candidate;
  }
  function send(msg) {
    if (!parentOrigin) return;
    msg.source = 'wasmrun-app';
    try { window.parent.postMessage(msg, parentOrigin); } catch (e) {}
  }
  function fmt(value) {
    if (value instanceof Error) return value.stack || String(value);
    if (typeof value === 'string') return value;
    try { return JSON.stringify(value); } catch (e) { return String(value); }
  }
  ['log', 'info', 'warn', 'error', 'debug'].forEach(function (level) {
    var original = console[level];
    console[level] = function () {
      send({ type: 'console', level: level, message: Array.prototype.map.call(arguments, fmt).join(' ') });
      return original.apply(console, arguments);
    };
  });
  window.addEventListener('error', function (e) {
    send({ type: 'console', level: 'error', message: (e.error && e.error.stack) || e.message });
  });
  window.addEventListener('unhandledrejection', function (e) {
    send({ type: 'console', level: 'error', message: 'Unhandled rejection: ' + fmt(e.reason) });
  });
  window.addEventListener('load', function () {
    var nav = performance.getEntriesByType('navigation')[0];
    send({ type: 'loaded', ms: nav ? Math.round(nav.loadEventStart) : null });
  });
  if (!WATCH) return;
  var generation = null;
  function poll() {
    fetch('/__wasmrun/build', { cache: 'no-store' })
      .then(function (r) { return r.json(); })
      .then(function (build) {
        if (generation !== null && build.generation !== generation) {
          location.reload();
          return;
        }
        generation = build.generation;
        setTimeout(poll, 1000);
      })
      .catch(function () { setTimeout(poll, 2000); });
  }
  poll();
})();
"#;

enum Resolved {
    File(PathBuf),
    DefaultPage,
}

pub fn handle_request(request: Request, state: &DevState) {
    let started = Instant::now();
    let method = request.method().clone();
    let raw_url = request.url().to_string();
    let path = url_path(&raw_url);

    if path.starts_with("/__wasmrun/") {
        respond_internal(request, state, &path);
        return;
    }

    let (status, bytes) = if !matches!(method, Method::Get | Method::Head) {
        respond(request, 405, "text/plain", b"Method Not Allowed".to_vec())
    } else {
        let artifacts = state.artifacts();
        match resolve(&path, state.project_path.as_deref(), &artifacts) {
            Some(Resolved::File(file)) => match fs::read(&file) {
                Ok(body) => {
                    let content_type = determine_content_type(&file);
                    let body = if content_type == "text/html" {
                        inject_client(&body)
                    } else {
                        body
                    };
                    respond(request, 200, content_type, body)
                }
                Err(e) => respond(request, 500, "text/plain", e.to_string().into_bytes()),
            },
            Some(Resolved::DefaultPage) => {
                let page = default_page(&state.project_name, &artifacts);
                respond(request, 200, "text/html", inject_client(page.as_bytes()))
            }
            None => respond(request, 404, "text/plain", b"404 Not Found".to_vec()),
        }
    };

    state.record_request(status, bytes);
    let level = match status {
        200..=399 => LogLevel::Info,
        404 => LogLevel::Warning,
        _ => LogLevel::Error,
    };
    state.log(
        LogSource::Http,
        level,
        format!(
            "{method} {path} {status} {} {} ms",
            format_bytes(bytes),
            started.elapsed().as_millis()
        ),
    );
}

fn respond_internal(request: Request, state: &DevState, path: &str) {
    match path {
        CLIENT_PATH => {
            let origins = ui_origins(&state.ui_url);
            let js = CLIENT_JS
                .replace(
                    "UI_ORIGINS",
                    &serde_json::to_string(&origins).unwrap_or_default(),
                )
                .replace("WATCH", if state.watch { "true" } else { "false" });
            respond(request, 200, "application/javascript", js.into_bytes());
        }
        BUILD_PATH => {
            let build = state.build_info();
            let body = serde_json::json!({
                "generation": build.generation,
                "status": build.status,
            });
            respond(
                request,
                200,
                "application/json",
                body.to_string().into_bytes(),
            );
        }
        _ => {
            respond(request, 404, "text/plain", b"404 Not Found".to_vec());
        }
    }
}

/// The control center is reached as `127.0.0.1` or `localhost`, and `postMessage` needs the exact one
fn ui_origins(ui_url: &str) -> Vec<String> {
    let alternate = if ui_url.contains("127.0.0.1") {
        ui_url.replace("127.0.0.1", "localhost")
    } else {
        ui_url.replace("localhost", "127.0.0.1")
    };
    vec![ui_url.to_string(), alternate]
}

fn respond(request: Request, status: u16, content_type: &str, body: Vec<u8>) -> (u16, u64) {
    let len = body.len() as u64;
    let response = Response::from_data(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "no-store"));
    if let Err(e) = request.respond(response) {
        eprintln!("❗ Error sending app response: {e}");
    }
    (status, len)
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header is valid")
}

fn url_path(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or("/");
    percent_decode(path)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The path's segments, or `None` if any of them could leave the root or names a dotfile
fn safe_segments(path: &str) -> Option<Vec<&str>> {
    let mut segments = Vec::new();
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        if segment.starts_with('.') || segment.contains('\\') || segment.contains('\0') {
            return None;
        }
        segments.push(segment);
    }
    Some(segments)
}

/// A file under `root`, following an `index.html` for a directory, that does not escape `root`
fn file_under(root: &Path, segments: &[&str]) -> Option<PathBuf> {
    let mut candidate = root.to_path_buf();
    candidate.extend(segments);
    if candidate.is_dir() {
        candidate.push("index.html");
    }
    if !candidate.is_file() {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let resolved = candidate.canonicalize().ok()?;
    resolved.starts_with(&root).then_some(resolved)
}

fn resolve(path: &str, project: Option<&Path>, artifacts: &Artifacts) -> Option<Resolved> {
    let segments = safe_segments(path)?;
    let build_dir = artifacts.dir();

    if segments.first() == Some(&"pkg") {
        if let Some(file) = file_under(&build_dir, &segments[1..]) {
            return Some(Resolved::File(file));
        }
    }
    if let Some(file) = project.and_then(|root| file_under(root, &segments)) {
        return Some(Resolved::File(file));
    }
    if let Some(file) = file_under(&build_dir, &segments) {
        // The build directory has no page of its own worth serving at the root
        if !segments.is_empty() {
            return Some(Resolved::File(file));
        }
    }
    let is_root_page = segments.is_empty() || segments == ["index.html"];
    (is_root_page && artifacts.js_path.is_some()).then_some(Resolved::DefaultPage)
}

/// Put the dev client first in `<head>`, so it wraps the console before any of the page's scripts run
fn inject_client(html: &[u8]) -> Vec<u8> {
    let tag = format!(r#"<script src="{CLIENT_PATH}"></script>"#);
    let text = String::from_utf8_lossy(html);
    let lower = text.to_ascii_lowercase();
    let insert_at = lower
        .find("<head")
        .and_then(|start| lower[start..].find('>').map(|end| start + end + 1))
        .or_else(|| {
            lower
                .find("<html")
                .and_then(|start| lower[start..].find('>').map(|end| start + end + 1))
        })
        .unwrap_or(0);
    let mut out = String::with_capacity(text.len() + tag.len());
    out.push_str(&text[..insert_at]);
    out.push_str(&tag);
    out.push_str(&text[insert_at..]);
    out.into_bytes()
}

/// For a build that emitted JS glue but a project with no page: load the glue and show what it exports
fn default_page(project_name: &str, artifacts: &Artifacts) -> String {
    let js = artifacts.js_filename().unwrap_or_default();
    let glue = artifacts
        .js_path
        .as_deref()
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_default();
    let is_bindgen = glue.contains("__wbg_init") || glue.contains("__wbindgen");
    let title = html_escape(project_name);

    let loader = if is_bindgen {
        format!(
            r#"<script type="module">
  import init, * as wasm from './pkg/{js}';
  await init();
  window.wasm = wasm;
  const names = Object.keys(wasm).filter((name) => name !== 'default' && name !== 'initSync' && !name.startsWith('__'));
  document.getElementById('exports').innerHTML =
    names.map((name) => `<li><code>${{name}}</code></li>`).join('') || '<li>none</li>';
  console.log(`${{names.length}} exports on window.wasm`);
</script>"#
        )
    } else {
        format!(r#"<script src="./pkg/{js}"></script>"#)
    };
    let note = if is_bindgen {
        "A wasm-bindgen module with no <code>index.html</code> of its own. Its exports are on <code>window.wasm</code>; call them from the browser console."
    } else {
        "The build emitted a JS loader and the project has no <code>index.html</code> of its own, so the loader runs here. Its output is in the Console panel."
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
  body {{ font-family: system-ui, sans-serif; max-width: 42rem; margin: 3rem auto; padding: 0 1rem; line-height: 1.5; color: #1e293b; }}
  code {{ background: #f1f5f9; padding: 0.1rem 0.3rem; border-radius: 0.25rem; }}
</style>
</head>
<body>
<h1>{title}</h1>
<p>{note}</p>
<h2>Exports</h2>
<ul id="exports"><li>Loading <code>{js}</code>...</li></ul>
{loader}
</body>
</html>
"#
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct Fixture {
        _dir: tempfile::TempDir,
        project: PathBuf,
        artifacts: Artifacts,
    }

    fn fixture(with_index: bool) -> Fixture {
        let dir = tempdir().unwrap();
        let project = dir.path().join("project");
        let build = dir.path().join("build");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::create_dir_all(project.join("pkg")).unwrap();
        fs::create_dir_all(build.join("snippets")).unwrap();
        if with_index {
            fs::write(
                project.join("index.html"),
                "<html><head></head><body></body></html>",
            )
            .unwrap();
        }
        fs::write(project.join("assets/app.css"), "body{}").unwrap();
        fs::write(project.join(".env"), "SECRET=1").unwrap();
        fs::write(project.join("pkg/app.js"), "stale").unwrap();
        fs::write(project.join("pkg/extra.js"), "only in project").unwrap();
        fs::write(
            build.join("app.js"),
            "export default function __wbg_init() {}",
        )
        .unwrap();
        fs::write(build.join("app_bg.wasm"), b"\0asm").unwrap();
        fs::write(build.join("snippets/s.js"), "snippet").unwrap();
        fs::write(dir.path().join("outside.txt"), "outside").unwrap();
        Fixture {
            artifacts: Artifacts {
                wasm_path: build.join("app_bg.wasm"),
                js_path: Some(build.join("app.js")),
            },
            project,
            _dir: dir,
        }
    }

    fn resolved_file(path: &str, f: &Fixture) -> Option<String> {
        match resolve(path, Some(&f.project), &f.artifacts) {
            Some(Resolved::File(p)) => Some(fs::read_to_string(p).unwrap()),
            Some(Resolved::DefaultPage) => Some("<default>".into()),
            None => None,
        }
    }

    #[test]
    fn test_build_output_wins_under_pkg() {
        let f = fixture(true);
        assert_eq!(
            resolved_file("/pkg/app.js", &f).unwrap(),
            "export default function __wbg_init() {}"
        );
        assert_eq!(resolved_file("/pkg/snippets/s.js", &f).unwrap(), "snippet");
        assert_eq!(
            resolved_file("/pkg/extra.js", &f).unwrap(),
            "only in project"
        );
    }

    #[test]
    fn test_project_files_and_root_fallback_to_build() {
        let f = fixture(true);
        assert!(resolved_file("/", &f).unwrap().contains("<html>"));
        assert_eq!(resolved_file("/assets/app.css", &f).unwrap(), "body{}");
        assert_eq!(resolved_file("/app_bg.wasm", &f).unwrap(), "\0asm");
        assert!(resolved_file("/missing.js", &f).is_none());
    }

    #[test]
    fn test_default_page_only_without_index() {
        let f = fixture(false);
        assert_eq!(resolved_file("/", &f).unwrap(), "<default>");
        assert_eq!(resolved_file("/index.html", &f).unwrap(), "<default>");

        let mut plain = fixture(false);
        plain.artifacts.js_path = None;
        assert!(resolved_file("/", &plain).is_none());
    }

    #[test]
    fn test_traversal_and_dotfiles_are_refused() {
        let f = fixture(true);
        assert!(resolved_file("/../outside.txt", &f).is_none());
        assert!(resolved_file("/pkg/../../outside.txt", &f).is_none());
        assert!(resolved_file("/.env", &f).is_none());
        assert!(resolved_file(&url_path("/%2e%2e/outside.txt"), &f).is_none());
        assert!(resolved_file(&url_path("/assets/..%2F..%2Foutside.txt"), &f).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_out_of_project_is_refused() {
        let f = fixture(true);
        std::os::unix::fs::symlink(
            f.project.parent().unwrap().join("outside.txt"),
            f.project.join("link.txt"),
        )
        .unwrap();
        assert!(resolved_file("/link.txt", &f).is_none());
    }

    #[test]
    fn test_url_path_strips_query_and_decodes() {
        assert_eq!(url_path("/a%20b.js?v=1#x"), "/a b.js");
        assert_eq!(url_path("/trailing%"), "/trailing%");
        assert_eq!(url_path("/bad%zz"), "/bad%zz");
        assert_eq!(url_path("/%é"), "/%é");
    }

    #[test]
    fn test_inject_client_goes_first_in_head() {
        let html =
            inject_client(b"<!DOCTYPE html><html lang=\"en\"><HEAD><title>x</title></HEAD></html>");
        let html = String::from_utf8(html).unwrap();
        assert!(html.contains(r#"<HEAD><script src="/__wasmrun/client.js"></script><title>"#));

        let bare = String::from_utf8(inject_client(b"<p>hi</p>")).unwrap();
        assert!(bare.starts_with(r#"<script src="/__wasmrun/client.js"></script><p>"#));
    }

    #[test]
    fn test_default_page_kinds() {
        let f = fixture(false);
        let bindgen = default_page("demo<x>", &f.artifacts);
        assert!(bindgen.contains("import init, * as wasm from './pkg/app.js'"));
        assert!(bindgen.contains("demo&lt;x&gt;"));

        fs::write(f.artifacts.js_path.as_ref().unwrap(), "var Module = {};").unwrap();
        let emscripten = default_page("demo", &f.artifacts);
        assert!(emscripten.contains(r#"<script src="./pkg/app.js"></script>"#));
    }

    #[test]
    fn test_ui_origins_cover_both_loopback_names() {
        assert_eq!(
            ui_origins("http://127.0.0.1:8420"),
            vec!["http://127.0.0.1:8420", "http://localhost:8420"]
        );
    }
}
