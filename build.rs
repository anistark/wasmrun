use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/vite.config.ts");

    if env::var("SKIP_UI_BUILD").is_ok() {
        eprintln!("Skipping UI build (SKIP_UI_BUILD set)");
        return;
    }

    let ui_dir = Path::new("ui");
    let templates_dir = Path::new("templates");

    // Skip UI build if templates already exist and UI source is not available
    if !ui_dir.exists() {
        if templates_dir.exists() && templates_dir.join("app").exists() && templates_dir.join("console").exists() {
            eprintln!("UI source not found but templates exist, skipping UI build");
            return;
        } else {
            eprintln!("UI directory not found, skipping UI build");
            return;
        }
    }

    let node_modules = ui_dir.join("node_modules");
    if !node_modules.exists() {
        eprintln!("Installing UI dependencies...");
        let output = Command::new("pnpm")
            .arg("install")
            .current_dir(ui_dir)
            .output()
            .expect("Failed to install UI dependencies. Make sure pnpm is installed.");

        if !output.status.success() {
            panic!(
                "UI dependency installation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    eprintln!("Building console UI...");
    let console_output = Command::new("pnpm")
        .args(["vite", "build"])
        .env("VITE_TEMPLATE", "console")
        .current_dir(ui_dir)
        .output()
        .expect("Failed to build console UI. Make sure pnpm is installed.");

    if !console_output.status.success() {
        panic!(
            "Console UI build failed: {}\nStdout: {}",
            String::from_utf8_lossy(&console_output.stderr),
            String::from_utf8_lossy(&console_output.stdout)
        );
    }

    eprintln!("Building app UI...");
    let app_output = Command::new("pnpm")
        .args(["vite", "build"])
        .env("VITE_TEMPLATE", "app")
        .current_dir(ui_dir)
        .output()
        .expect("Failed to build app UI. Make sure pnpm is installed.");

    if !app_output.status.success() {
        panic!(
            "App UI build failed: {}\nStdout: {}",
            String::from_utf8_lossy(&app_output.stderr),
            String::from_utf8_lossy(&app_output.stdout)
        );
    }

    reorganize_build_output();
    eprintln!("UI build completed successfully!");
}

fn reorganize_build_output() {
    use std::fs;

    let temp_dir = Path::new("templates-temp");
    let target_dir = Path::new("templates");

    if !temp_dir.exists() {
        eprintln!("Temp build directory not found, skipping reorganization");
        return;
    }

    if target_dir.exists() {
        let _ = fs::remove_dir_all(target_dir);
    }
    let _ = fs::create_dir_all(target_dir);

    process_template_v2(&temp_dir.join("app"), target_dir, "app");
    process_template_v2(&temp_dir.join("console"), target_dir, "console");

    let assets_source = Path::new("assets");
    let assets_dest = target_dir.join("assets");
    if assets_source.exists() {
        let _ = copy_dir_recursive(assets_source, &assets_dest);
    }

    let _ = fs::remove_dir_all(temp_dir);
}

fn process_template_v2(template_build_dir: &Path, target_dir: &Path, template_name: &str) {
    use std::fs;

    let target_template_dir = target_dir.join(template_name);
    let _ = fs::create_dir_all(&target_template_dir);

    eprintln!(
        "Processing template '{template_name}' from {template_build_dir:?} to {target_template_dir:?}"
    );

    let html_src = template_build_dir.join(format!("src/{template_name}/index.html"));
    let html_dest = target_template_dir.join("index.html");
    if html_src.exists() {
        let _ = fs::copy(&html_src, &html_dest);
        fix_html_references(&html_dest);
    }

    let js_src = template_build_dir.join(format!("{template_name}.js"));
    if js_src.exists() {
        let js_dest = target_template_dir.join("scripts.js");
        let _ = fs::copy(&js_src, &js_dest);
    }

    let shared_dest = target_template_dir.join("shared.js");
    let _ = fs::write(&shared_dest, "// All code is bundled into scripts.js");

    let wasi_src = Path::new("ui/src/wasi/wasmrun_wasi_impl.js");
    let wasi_dest = target_template_dir.join("wasmrun_wasi_impl.js");
    if wasi_src.exists() {
        let _ = fs::copy(wasi_src, &wasi_dest);
    }

    let css_src = template_build_dir.join("index.css");
    if css_src.exists() {
        let css_dest = target_template_dir.join("style.css");
        let _ = fs::copy(&css_src, &css_dest);
    }
}

fn fix_html_references(html_path: &Path) {
    use regex::Regex;
    use std::fs;

    if let Ok(content) = fs::read_to_string(html_path) {
        let mut updated = content
            .replace("crossorigin ", "")
            .replace("type=\"module\" ", "");

        if let Ok(script_regex) = Regex::new(r#"<script[^>]*src="[^"]*\.(js|mjs)"[^>]*></script>"#)
        {
            updated = script_regex.replace_all(&updated, "").to_string();
        }

        if let Ok(link_js_regex) = Regex::new(r#"<link[^>]*rel="modulepreload"[^>]*>"#) {
            updated = link_js_regex.replace_all(&updated, "").to_string();
        }

        if let Ok(link_css_regex) =
            Regex::new(r#"<link[^>]*rel="stylesheet"[^>]*href="[^"]*\.css"[^>]*>"#)
        {
            updated = link_css_regex.replace_all(&updated, "").to_string();
        }

        if !updated.contains("<!-- @style-placeholder -->") {
            updated = updated.replace("</head>", "    <!-- @style-placeholder -->\n</head>");
        }

        if !updated.contains("<!-- @script-placeholder -->") {
            updated = updated.replace("</body>", "    <!-- @script-placeholder -->\n</body>");
        }

        let _ = fs::write(html_path, updated);
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    use std::fs;

    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}
