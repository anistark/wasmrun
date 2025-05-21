use std::fs;
use std::path::Path;

// For production, we'll use included templates
const HTML_TEMPLATE: &str = include_str!("index.html");
const CSS_TEMPLATE: &str = include_str!("style.css");
const JS_TEMPLATE: &str = include_str!("scripts.js");

/// Generate HTML with the app's JavaScript bundle
pub fn generate_webapp_html(app_name: &str, js_entrypoint: &str) -> String {
    // Determine if we're in development mode
    let in_dev_mode = Path::new("src/template/webapp/index.html").exists();

    if in_dev_mode {
        // If we're in development mode, load templates from disk
        let template_dir = Path::new("src/template/webapp");

        let html = fs::read_to_string(template_dir.join("index.html"))
            .unwrap_or_else(|_| "Failed to load index.html".to_string());

        let css = fs::read_to_string(template_dir.join("style.css"))
            .unwrap_or_else(|_| "/* Failed to load style.css */".to_string());

        let js = fs::read_to_string(template_dir.join("scripts.js"))
            .unwrap_or_else(|_| "// Failed to load scripts.js".to_string());

        return html
            .replace("$APP_NAME$", app_name)
            .replace(
                "<!-- @style-placeholder -->",
                &format!(
                    "<!-- Additional Chakra styles -->\n<style>\n{}\n</style>",
                    css
                ),
            )
            .replace(
                "<!-- @script-placeholder -->",
                &format!(
                    "<script type=\"module\">\n{}\n</script>",
                    js.replace("$JS_ENTRYPOINT$", js_entrypoint)
                ),
            );
    }

    // Use the included templates in production
    HTML_TEMPLATE
        .replace("$APP_NAME$", app_name)
        .replace(
            "<!-- @style-placeholder -->",
            &format!(
                "<!-- Additional Chakra styles -->\n<style>\n{}\n</style>",
                CSS_TEMPLATE
            ),
        )
        .replace(
            "<!-- @script-placeholder -->",
            &format!(
                "<script type=\"module\">\n{}\n</script>",
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
                    // Capitalize first letter and replace dashes with spaces
                    let formatted_name = cleaned_name
                        .split('-')
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(c) => c.to_uppercase().chain(chars).collect(),
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(" ");

                    return formatted_name;
                }
            }
        }
    }

    // Fallback: use directory name
    let path = Path::new(project_path);
    if let Some(dir_name) = path.file_name() {
        if let Some(name_str) = dir_name.to_str() {
            // Format directory name by replacing dashes with spaces
            let formatted_name = name_str
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(c) => c.to_uppercase().chain(chars).collect(),
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");

            return formatted_name;
        }
    }

    "Rust Web Application".to_string()
}
