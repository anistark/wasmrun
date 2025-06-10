use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tiny_http::Server;

use crate::compiler;
use crate::template::{generate_webapp_html, get_app_name};
use crate::watcher;

use super::handler;
use super::utils;

/// Run a Rust web application
pub fn run_webapp(path: &str, port: u16, watch_mode: bool) -> Result<(), String> {
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🚀 \x1b[1;36mChakra: Running Rust Web Application\x1b[0m\n");
    println!(
        "  📂 \x1b[1;34mProject Path:\x1b[0m \x1b[1;33m{}\x1b[0m",
        path
    );
    println!(
        "  🌐 \x1b[1;34mTarget Port:\x1b[0m \x1b[1;33m{}\x1b[0m",
        port
    );

    if watch_mode {
        println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mEnabled\x1b[0m");
    }

    // Check if server already running on this port
    if !utils::is_port_available(port) {
        if super::is_server_running() {
            match super::stop_existing_server() {
                Ok(_) => println!("  💀 \x1b[1;34mStopped existing server\x1b[0m"),
                Err(e) => eprintln!(
                    "  ⚠️ \x1b[1;33mWarning when stopping existing server: {}\x1b[0m",
                    e
                ),
            }
        } else {
            println!(
                "  ❌ \x1b[1;31mPort {} is already in use by another process\x1b[0m",
                port
            );
            println!("\x1b[1;34m╰\x1b[0m");
            return Err(format!("Port {} is already in use", port));
        }
    }
    let temp_dir = std::env::temp_dir().join("chakra_webapp_temp");
    let temp_output_dir = temp_dir.to_str().unwrap_or("/tmp").to_string();
    if !temp_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            println!(
                "  ❌ \x1b[1;31mFailed to create temporary directory: {}\x1b[0m",
                e
            );
            println!("\x1b[1;34m╰\x1b[0m");
            return Err(format!("Failed to create temporary directory: {}", e));
        }
    }

    println!(
        "  📁 \x1b[1;34mOutput Directory:\x1b[0m \x1b[1;33m{}\x1b[0m",
        temp_output_dir
    );
    println!("\x1b[1;34m╰\x1b[0m\n");
    println!("Building Rust web application...");
    let app_name = get_app_name(path);

    let js_entrypoint = match compiler::build_rust_web_application(path, &temp_output_dir) {
        Ok(js_file) => {
            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  ✅ \x1b[1;36mBuild Successful\x1b[0m\n");
            println!(
                "  📦 \x1b[1;34mJS Entry Point:\x1b[0m \x1b[1;32m{}\x1b[0m",
                js_file
            );
            println!("\x1b[1;34m╰\x1b[0m\n");
            js_file
        }
        Err(e) => {
            println!("\n\x1b[1;34m╭\x1b[0m");
            println!("  ❌ \x1b[1;31mBuild Failed\x1b[0m\n");
            println!("  \x1b[0;91m{}\x1b[0m", e);
            println!("\x1b[1;34m╰\x1b[0m\n");
            return Err(format!("Build failed: {}", e));
        }
    };
    let html = generate_webapp_html(&app_name, &js_entrypoint);
    run_webapp_server(path, &temp_output_dir, port, &html, watch_mode)?;
    Ok(())
}

/// Run the server for a web application
fn run_webapp_server(
    project_path: &str,
    output_dir: &str,
    port: u16,
    html: &str,
    watch_mode: bool,
) -> Result<(), String> {
    // Store PID
    let pid = std::process::id();
    fs::write(super::PID_FILE, pid.to_string())
        .map_err(|e| format!("Failed to write PID to {}: {}", super::PID_FILE, e))?;

    let url = format!("http://localhost:{}", port);

    // Display welcome message
    println!("\n\x1b[1;34m╭\x1b[0m");
    println!("  🌐 \x1b[1;36mWeb Application Server\x1b[0m\n");
    println!("  🚀 \x1b[1;34mServer URL:\x1b[0m \x1b[4;36m{}\x1b[0m", url);
    println!(
        "  🔌 \x1b[1;34mListening on port:\x1b[0m \x1b[1;33m{}\x1b[0m",
        port
    );

    if watch_mode {
        println!("  👀 \x1b[1;34mWatch Mode:\x1b[0m \x1b[1;32mActive\x1b[0m");
    }

    println!("  ℹ️ \x1b[1;34mServer PID:\x1b[0m \x1b[0;37m{}\x1b[0m", pid);
    println!("\n  \x1b[0;90mPress Ctrl+C to stop the server\x1b[0m");
    println!("\x1b[1;34m╰\x1b[0m\n");

    // Open browser
    if let Err(e) = webbrowser::open(&url) {
        println!("❗ Failed to open browser automatically: {e}");
    } else {
        println!("🌐 Opening browser...");
    }

    // Setup for watch mode if needed
    if watch_mode {
        match watcher::ProjectWatcher::new(project_path) {
            Ok(watcher) => {
                run_webapp_server_with_watch(watcher, project_path, output_dir, port, html)?;
            }
            Err(e) => {
                eprintln!("Failed to set up file watcher: {}", e);
                run_webapp_server_without_watch(html, output_dir, port)?;
            }
        }
    } else {
        // Run server without watching
        run_webapp_server_without_watch(html, output_dir, port)?;
    }

    // Clean up PID file
    if Path::new(super::PID_FILE).exists() {
        let _ = fs::remove_file(super::PID_FILE);
    }

    Ok(())
}

/// Run the webapp server with watch mode
fn run_webapp_server_with_watch(
    watcher: watcher::ProjectWatcher,
    project_path: &str,
    output_dir: &str,
    port: u16,
    html: &str,
) -> Result<(), String> {
    // Channels for communication
    let (tx, rx) = std::sync::mpsc::channel();
    let reload_flag = Arc::new(AtomicBool::new(false));

    // Need to clone values for server thread
    let html_content = html.to_string();
    let output_path = output_dir.to_string();
    let reload_flag_clone = Arc::clone(&reload_flag);
    let project_path = project_path.to_string();

    // Start server in a new thread
    let server_thread = std::thread::spawn(move || {
        // Create HTTP server
        if let Ok(server) = Server::http(format!("0.0.0.0:{}", port)) {
            // Track connected clients for live reload
            let mut clients_to_reload = Vec::new();

            for request in server.incoming_requests() {
                // Check for shutdown signal
                if rx.try_recv().is_ok() {
                    break;
                }

                handler::handle_webapp_request(
                    request,
                    &html_content,
                    &output_path,
                    &mut clients_to_reload,
                    &reload_flag_clone, // Pass the reload flag
                );
            }
        }
    });

    // Let the server start up
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Watch for file changes in the main thread
    println!("👀 Watching project directory for changes...");

    loop {
        // Wait for file changes
        if let Some(Ok(events)) = watcher.wait_for_change() {
            if watcher.should_recompile(&events) {
                println!("\n📝 File change detected. Recompiling...");

                // Recompile the project
                match compiler::build_rust_web_application(&project_path, output_dir) {
                    Ok(_) => {
                        println!("✅ Recompilation successful!");
                        println!("🔄 Triggering browser reload...");

                        // Set the reload flag
                        reload_flag.store(true, Ordering::SeqCst);
                    }
                    Err(e) => {
                        println!("❌ Recompilation failed: {}", e);
                    }
                }
            }
        }

        // Check for server exit
        if server_thread.is_finished() {
            println!("Server stopped. Exiting watch mode.");
            break;
        }

        // Sleep to avoid high CPU usage
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Signal the server to stop if still running
    let _ = tx.send(());

    // Wait for server thread to finish
    if let Err(e) = server_thread.join() {
        eprintln!("Error joining server thread: {:?}", e);
    }

    Ok(())
}

/// Run the webapp server without watch mode
fn run_webapp_server_without_watch(html: &str, output_dir: &str, port: u16) -> Result<(), String> {
    // Create HTTP server
    let server = Server::http(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // Track connected clients for live reload (not used in non-watch mode)
    let mut clients_to_reload = Vec::new();

    // Create a reload flag that will never be set in non-watch mode
    let reload_flag = Arc::new(AtomicBool::new(false));

    for request in server.incoming_requests() {
        handler::handle_webapp_request(
            request,
            html,
            output_dir,
            &mut clients_to_reload,
            &reload_flag,
        );
    }

    Ok(())
}

/// Helper function to recursively copy a directory
#[allow(dead_code)]
fn copy_dir_recursively(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    if !destination.exists() {
        fs::create_dir_all(destination)?;
    }

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursively(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }

    Ok(())
}
