use std::fs;
use std::path::Path;

// For development, we can load templates at runtime
pub fn generate_webapp_html_dev(app_name: &str, js_entrypoint: &str) -> String {
    let template_dir = Path::new("src/template/webapp");

    // Load templates from files at runtime
    let html = fs::read_to_string(template_dir.join("index.html"))
        .unwrap_or_else(|_| "Failed to load index.html".to_string());

    let css = fs::read_to_string(template_dir.join("style.css"))
        .unwrap_or_else(|_| "/* Failed to load style.css */".to_string());

    let js = fs::read_to_string(template_dir.join("scripts.js"))
        .unwrap_or_else(|_| "// Failed to load scripts.js".to_string());

    html.replace("$APP_NAME$", app_name)
        .replace(
            "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n</style>", css),
        )
        .replace(
            "<!-- @script-placeholder -->",
            &format!(
                "<script>\n{}\n</script>",
                js.replace("$JS_ENTRYPOINT$", js_entrypoint)
            ),
        )
}

// For production, we'll use included templates
const HTML_TEMPLATE: &str = include_str!("index.html");
const CSS_TEMPLATE: &str = include_str!("style.css");
const JS_TEMPLATE: &str = include_str!("scripts.js");

pub fn generate_webapp_html(app_name: &str, js_entrypoint: &str) -> String {
    // Determine if we're in development mode
    let in_dev_mode = Path::new("src/template/webapp/index.html").exists();

    if in_dev_mode {
        return generate_webapp_html_dev(app_name, js_entrypoint);
    }

    // Otherwise use the included templates
    HTML_TEMPLATE
        .replace("$APP_NAME$", app_name)
        .replace(
            "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n</style>", CSS_TEMPLATE),
        )
        .replace(
            "<!-- @script-placeholder -->",
            &format!(
                "<script>\n{}\n</script>",
                JS_TEMPLATE.replace("$JS_ENTRYPOINT$", js_entrypoint)
            ),
        )
}

/// Get the app name from a project path
pub fn get_app_name(project_path: &str) -> String {
    // Extract app name from Cargo.toml if possible
    if let Ok(cargo_toml) = std::fs::read_to_string(Path::new(project_path).join("Cargo.toml")) {
        if let Some(name_line) = cargo_toml
            .lines()
            .find(|line| line.trim().starts_with("name"))
        {
            if let Some(name) = name_line.split('=').nth(1) {
                let cleaned_name = name.trim().trim_matches('"').trim_matches('\'');
                if !cleaned_name.is_empty() {
                    return format!("{} Web App", cleaned_name);
                }
            }
        }
    }

    // Fallback: use directory name
    let path = Path::new(project_path);
    if let Some(dir_name) = path.file_name() {
        if let Some(name_str) = dir_name.to_str() {
            return format!("{} Web App", name_str);
        }
    }

    // Final fallback
    "Rust Web Application".to_string()
}
