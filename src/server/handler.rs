use std::fs;
use std::path::Path;
use tiny_http::{Request, Response};

use super::api::{serve_asset, serve_file, serve_module_info, serve_version_info};
use super::utils::{content_type_header, determine_content_type};
use crate::template::{TemplateManager, TemplateType};

/// Handle an incoming HTTP request
#[allow(clippy::too_many_arguments)]
pub fn handle_request(
    request: Request,
    js_filename: Option<&str>,
    wasm_filename: &str,
    wasm_path: &str,
    project_path: Option<&str>,
    watch_mode: bool,
    clients_to_reload: &mut Vec<String>,
    template_manager: &TemplateManager,
    template_type: &TemplateType,
) {
    let url = request.url().to_string();
    let client_addr = match request.remote_addr() {
        Some(addr) => addr.to_string(),
        None => "unknown".to_string(),
    };

    println!("📝 Received request for: {url}");

    if url == "/" {
        // Serve the main HTML page
        let html = if watch_mode {
            template_manager.generate_html_with_watch_mode(template_type, wasm_filename, true)
        } else {
            template_manager.generate_html(template_type, wasm_filename)
        };

        let html = match html {
            Ok(html) => html,
            Err(e) => {
                eprintln!("❗ Error generating HTML: {e}");
                format!(
                    "<html><body><h1>Error</h1><p>Failed to generate HTML: {e}</p></body></html>"
                )
            }
        };

        let response = Response::from_string(html).with_header(content_type_header("text/html"));
        if let Err(e) = request.respond(response) {
            eprintln!("❗ Error sending HTML response: {e}");
        }

        if watch_mode && !clients_to_reload.contains(&client_addr) {
            clients_to_reload.push(client_addr);
        }
    } else if url == format!("/{wasm_filename}") {
        serve_file(request, wasm_path, "application/wasm");
    } else if let Some(js_file) = js_filename {
        if url == format!("/{js_file}") {
            let js_path = Path::new(wasm_path).parent().unwrap().join(js_file);
            serve_file(request, js_path.to_str().unwrap(), "application/javascript");
        }
    } else if url == "/reload" {
        if watch_mode {
            // TODO: check if there was an actual file change
            println!("🔄 Handling reload request in watch mode");

            let response =
                Response::from_string("no-reload").with_header(content_type_header("text/plain"));

            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending reload response: {e}");
            }
        } else {
            let response = Response::from_string("not-watching")
                .with_header(content_type_header("text/plain"));

            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending reload response: {e}");
            }
        }
    } else if url == "/api/module-info" {
        serve_module_info(request, wasm_path, project_path);
    } else if url == "/api/version" {
        serve_version_info(request);
    } else if url.starts_with("/assets/") {
        serve_asset(request, &url);
    } else {
        let base_dir = Path::new(wasm_path).parent().unwrap();
        let requested_file = base_dir.join(url.trim_start_matches('/'));

        if requested_file.exists() && requested_file.is_file() {
            let content_type = determine_content_type(&requested_file);
            serve_file(request, requested_file.to_str().unwrap(), content_type);
        } else {
            if url.ends_with("_bg.wasm") {
                if let Ok(entries) = fs::read_dir(base_dir) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if let Some(name) = entry_path.file_name() {
                            if name.to_string_lossy().ends_with("_bg.wasm") && entry_path.is_file()
                            {
                                serve_file(
                                    request,
                                    entry_path.to_str().unwrap(),
                                    "application/wasm",
                                );
                                return;
                            }
                        }
                    }
                }
            }

            // Check for common file patterns (js, css, etc.)
            for ext in &["js", "css", "json", "wasm"] {
                if url.ends_with(&format!(".{ext}")) {
                    let filename = url.split('/').next_back().unwrap_or("");
                    if let Ok(entries) = fs::read_dir(base_dir) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path
                                .file_name()
                                .is_some_and(|name| name.to_string_lossy() == filename)
                            {
                                let content_type = match *ext {
                                    "js" => "application/javascript",
                                    "css" => "text/css",
                                    "json" => "application/json",
                                    "wasm" => "application/wasm",
                                    _ => "application/octet-stream",
                                };
                                serve_file(request, entry_path.to_str().unwrap(), content_type);
                                return;
                            }
                        }
                    }
                }
            }

            // 404 for all other requests
            let response = Response::from_string("404 Not Found")
                .with_status_code(404)
                .with_header(content_type_header("text/plain"));
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending 404 response: {e}");
            }
        }
    }
}
