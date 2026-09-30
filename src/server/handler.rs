//! [Server Mode] The UI port: the console for a module, or the control center around a web app

use std::path::Path;
use tiny_http::{Request, Response};

use super::api::{serve_asset, serve_file, serve_module_info, serve_version_info};
use super::dev::DevState;
use super::utils::{content_type_header, determine_content_type};
use crate::template::{TemplateManager, TemplateType};

pub fn handle_request(
    request: Request,
    state: &DevState,
    templates: &TemplateManager,
    template_type: &TemplateType,
) {
    let url = request.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let artifacts = state.artifacts();
    let wasm_filename = artifacts.wasm_filename();

    match path {
        "/" => {
            let html =
                templates.generate_html_with_watch_mode(template_type, &wasm_filename, state.watch);
            respond(request, Response::from_string(html), "text/html");
        }
        "/api/dev" => respond_json(request, &state.snapshot()),
        "/api/logs" => {
            let since = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("since="))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let (entries, next) = state.logs_since(since);
            respond_json(
                request,
                &serde_json::json!({ "entries": entries, "next": next }),
            );
        }
        "/api/module-info" => serve_module_info(
            request,
            &artifacts.wasm_path.to_string_lossy(),
            state
                .project_path
                .as_deref()
                .map(Path::to_string_lossy)
                .as_deref(),
        ),
        "/api/version" => serve_version_info(request),
        p if p.starts_with("/assets/") => serve_asset(request, p),
        p if p.trim_start_matches('/') == wasm_filename => serve_file(
            request,
            &artifacts.wasm_path.to_string_lossy(),
            "application/wasm",
        ),
        p => {
            let name = p.trim_start_matches('/');
            let dir = artifacts.dir();
            let file = dir.join(name);
            let inside = !name.is_empty()
                && !name.split('/').any(|s| s.starts_with('.'))
                && file.is_file()
                && file
                    .canonicalize()
                    .ok()
                    .zip(dir.canonicalize().ok())
                    .is_some_and(|(f, d)| f.starts_with(d));
            if inside {
                serve_file(
                    request,
                    &file.to_string_lossy(),
                    determine_content_type(&file),
                );
            } else {
                let response = Response::from_string("404 Not Found").with_status_code(404);
                respond(request, response, "text/plain");
            }
        }
    }
}

fn respond_json(request: Request, value: &serde_json::Value) {
    respond(
        request,
        Response::from_string(value.to_string()),
        "application/json",
    );
}

fn respond<R: std::io::Read>(request: Request, response: Response<R>, content_type: &str) {
    let response = response
        .with_header(content_type_header(content_type))
        .with_header(
            tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..])
                .expect("static header is valid"),
        );
    if let Err(e) = request.respond(response) {
        eprintln!("❗ Error sending response: {e}");
    }
}
