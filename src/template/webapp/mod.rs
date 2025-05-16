use std::path::Path;

// HTML template for web applications
const WEBAPP_HTML: &str = include_str!("index.html");

/// Generate HTML for a web application
pub fn generate_webapp_html(app_name: &str, js_entrypoint: &str) -> String {
    WEBAPP_HTML
        .replace("$APP_NAME$", app_name)
        .replace("$JS_ENTRYPOINT$", js_entrypoint)
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
