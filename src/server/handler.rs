use std::fs;
use std::path::Path;
// use std::sync::atomic::{AtomicBool, Ordering};
// use std::sync::Arc;
use tiny_http::{Request, Response};

use super::utils::{check_assets_directory, content_type_header, determine_content_type};
use crate::template::{TemplateManager, TemplateType};
use crate::commands::verify_wasm;
use crate::plugin::manager::PluginManager;

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

/// Serve a file
pub fn serve_file(request: Request, file_path: &str, content_type: &str) {
    match fs::read(file_path) {
        Ok(file_bytes) => {
            println!(
                "🔄 Serving file: {} ({} bytes, content-type: {})",
                file_path,
                file_bytes.len(),
                content_type
            );
            let response =
                Response::from_data(file_bytes).with_header(content_type_header(content_type));
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending file response: {e}");
            }
        }
        Err(e) => {
            eprintln!("❗ Error reading file {file_path}: {e}");
            let response = Response::from_string(format!("Error: {e}"))
                .with_status_code(500)
                .with_header(content_type_header("text/plain"));
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending error response: {e}");
            }
        }
    }
}

/// Serve a static asset file
pub fn serve_asset(request: Request, url: &str) {
    let asset_filename = url.strip_prefix("/assets/").unwrap_or("");
    let asset_path = format!("./assets/{asset_filename}");

    let content_type = if url.ends_with(".png") {
        "image/png"
    } else if url.ends_with(".jpg") || url.ends_with(".jpeg") {
        "image/jpeg"
    } else if url.ends_with(".svg") {
        "image/svg+xml"
    } else if url.ends_with(".gif") {
        "image/gif"
    } else if url.ends_with(".css") {
        "text/css"
    } else if url.ends_with(".js") {
        "application/javascript"
    } else {
        "application/octet-stream"
    };

    match fs::read(&asset_path) {
        Ok(asset_bytes) => {
            println!(
                "🖼️ Successfully serving asset: {} ({} bytes)",
                asset_path,
                asset_bytes.len()
            );
            let response =
                Response::from_data(asset_bytes).with_header(content_type_header(content_type));
            if let Err(e) = request.respond(response) {
                eprintln!("‼️ Error sending asset response: {e}");
            }
        }
        Err(e) => {
            eprintln!("‼️ Error reading asset file {asset_path}: {e} (does the file exist?)");

            check_assets_directory();

            let response = Response::from_string(format!("Asset not found: {e}"))
                .with_status_code(404)
                .with_header(content_type_header("text/plain"));
            if let Err(e) = request.respond(response) {
                eprintln!("‼️ Error sending asset error response: {e}");
            }
        }
    }
}

/// Serve WASM module information as JSON
pub fn serve_module_info(request: Request, wasm_path: &str, project_path: Option<&str>) {
    match verify_wasm(wasm_path) {
        Ok(verification_result) => {
            // Get plugin information for the project
            let plugin_info = if let Ok(plugin_manager) = PluginManager::new() {
                if let Some(project_path) = project_path {
                    println!("🔍 Looking for plugin for project: {}", project_path);
                    
                    // Find the plugin used for this project
                    if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
                        let info = plugin.info();
                        println!("✅ Found plugin: {} v{}", info.name, info.version);
                        Some(serde_json::json!({
                            "name": info.name,
                            "version": info.version,
                            "description": info.description,
                            "type": if plugin_manager.get_external_plugins().contains_key(&info.name) { "external" } else { "builtin" }
                        }))
                    } else {
                        println!("❌ No plugin found for project: {}", project_path);
                        None
                    }
                } else {
                    println!("❌ No project path provided, unable to detect plugin");
                    None
                }
            } else {
                println!("❌ Failed to create plugin manager");
                None
            };
            
            // Convert VerificationResult to JSON
            let mut json_response = serde_json::json!({
                "valid_magic": verification_result.valid_magic,
                "file_size": verification_result.file_size,
                "section_count": verification_result.section_count,
                "sections": verification_result.sections.iter().map(|s| serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "size": s.size
                })).collect::<Vec<_>>(),
                "has_export_section": verification_result.has_export_section,
                "export_names": verification_result.export_names,
                "has_start_section": verification_result.has_start_section,
                "start_function_index": verification_result.start_function_index,
                "has_memory_section": verification_result.has_memory_section,
                "memory_limits": verification_result.memory_limits,
                "has_table_section": verification_result.has_table_section,
                "function_count": verification_result.function_count
            });
            
            // Add plugin info if available
            if let Some(plugin) = plugin_info {
                json_response["plugin"] = plugin;
            }

            println!("📊 Serving module info for: {}", wasm_path);
            
            let response = Response::from_string(json_response.to_string())
                .with_header(content_type_header("application/json"))
                .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*").unwrap());
            
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending module info response: {e}");
            }
        }
        Err(error) => {
            eprintln!("❗ Error analyzing WASM module {}: {}", wasm_path, error);
            
            let error_response = serde_json::json!({
                "error": error,
                "valid_magic": false
            });

            let response = Response::from_string(error_response.to_string())
                .with_status_code(500)
                .with_header(content_type_header("application/json"))
                .with_header(tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*").unwrap());
            
            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending error response: {e}");
            }
        }
    }
}
