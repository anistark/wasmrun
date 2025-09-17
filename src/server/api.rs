use std::fs;
use tiny_http::{Request, Response};

use super::utils::{check_assets_directory, content_type_header};
use crate::commands::verify_wasm;
use crate::plugin::manager::PluginManager;

/// Serve WASM module information as JSON
pub fn serve_module_info(request: Request, wasm_path: &str, project_path: Option<&str>) {
    match verify_wasm(wasm_path) {
        Ok(verification_result) => {
            // Get plugin information for the project
            let plugin_info = if let Ok(plugin_manager) = PluginManager::new() {
                if let Some(project_path) = project_path {
                    println!("🔍 Looking for plugin for project: {project_path}");

                    // Find the plugin used for this project
                    if let Some(plugin) = plugin_manager.find_plugin_for_project(project_path) {
                        let info = plugin.info();
                        println!("✅ Found plugin: {} v{}", info.name, info.version);
                        Some(serde_json::json!({
                            "name": info.name,
                            "version": info.version,
                            "description": info.description,
                            "author": info.author,
                            "type": if plugin_manager.get_external_plugins().contains_key(&info.name) { "external" } else { "builtin" },
                            "source": info.source.as_ref().map(|s| match s {
                                crate::plugin::PluginSource::CratesIo { name, version: _ } =>
                                    serde_json::json!({
                                        "type": "crates.io",
                                        "url": format!("https://crates.io/crates/{}", name)
                                    }),
                                crate::plugin::PluginSource::Local { path } =>
                                    serde_json::json!({
                                        "type": "local",
                                        "path": path.to_string_lossy()
                                    }),
                                crate::plugin::PluginSource::Git { url, branch } =>
                                    serde_json::json!({
                                        "type": "git",
                                        "url": url,
                                        "branch": branch
                                    })
                            }),
                            "capabilities": {
                                "compile_wasm": info.capabilities.compile_wasm,
                                "compile_webapp": info.capabilities.compile_webapp,
                                "live_reload": info.capabilities.live_reload,
                                "optimization": info.capabilities.optimization,
                                "custom_targets": info.capabilities.custom_targets
                            }
                        }))
                    } else {
                        println!("❌ No plugin found for project: {project_path}");
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

            println!("📊 Serving module info for: {wasm_path}");

            let response = Response::from_string(json_response.to_string())
                .with_header(content_type_header("application/json"))
                .with_header(
                    tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*")
                        .unwrap(),
                );

            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending module info response: {e}");
            }
        }
        Err(error) => {
            eprintln!("❗ Error analyzing WASM module {wasm_path}: {error}");

            let error_response = serde_json::json!({
                "error": error,
                "valid_magic": false
            });

            let response = Response::from_string(error_response.to_string())
                .with_status_code(500)
                .with_header(content_type_header("application/json"))
                .with_header(
                    tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*")
                        .unwrap(),
                );

            if let Err(e) = request.respond(response) {
                eprintln!("❗ Error sending error response: {e}");
            }
        }
    }
}

/// Serve version information as JSON
pub fn serve_version_info(request: Request) {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    let version_response = serde_json::json!({
        "name": name,
        "version": version
    });

    println!("📊 Serving version info: {name} v{version}");

    let response = Response::from_string(version_response.to_string())
        .with_header(content_type_header("application/json"))
        .with_header(
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*").unwrap(),
        );

    if let Err(e) = request.respond(response) {
        eprintln!("❗ Error sending version info response: {e}");
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
