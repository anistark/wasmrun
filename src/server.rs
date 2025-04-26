use tiny_http::{Server, Response, Request};
use std::fs;
use std::path::Path;

use crate::template::generate_html;
use crate::utils::content_type_header;

pub fn run_server(path: &str, port: u16) {
    let wasm_filename = Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let server = Server::http(format!("0.0.0.0:{port}")).unwrap();
    println!("🚀 Chakra server running at http://localhost:{port}");

    if let Err(e) = webbrowser::open(&format!("http://localhost:{port}")) {
        println!("❗ Failed to open browser automatically: {e}");
    }

    for request in server.incoming_requests() {
        handle_request(request, &wasm_filename, path);
    }
}

fn handle_request(request: Request, wasm_filename: &str, wasm_path: &str) {
    let url = request.url();

    if url == "/" {
        let html = generate_html(wasm_filename);
        let response = Response::from_string(html)
            .with_header(content_type_header("text/html"));
        request.respond(response).unwrap();
    } else if url.trim_start_matches("/") == wasm_filename {
        let wasm_bytes = fs::read(wasm_path).unwrap();
        let response = Response::from_data(wasm_bytes)
            .with_header(content_type_header("application/wasm"));
        request.respond(response).unwrap();
    } else {
        let response = Response::from_string("404 Not Found")
            .with_status_code(404)
            .with_header(content_type_header("text/plain"));
        request.respond(response).unwrap();
    }
}
