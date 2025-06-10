use std::fs;
use std::path::Path;

const HTML_TEMPLATE: &str = include_str!("index.html");
const CSS_TEMPLATE: &str = include_str!("style.css");
const JS_TEMPLATE: &str = include_str!("scripts.js");

/// Generate HTML with the app's JavaScript bundle
pub fn generate_webapp_html(app_name: &str, js_entrypoint: &str) -> String {
    let in_dev_mode = Path::new("src/template/webapp/index.html").exists();

    if in_dev_mode {
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

pub fn get_app_name(project_path: &str) -> String {
    if let Ok(cargo_toml) = std::fs::read_to_string(Path::new(project_path).join("Cargo.toml")) {
        if let Some(name_line) = cargo_toml
            .lines()
            .find(|line| line.trim().starts_with("name"))
        {
            if let Some(name) = name_line.split('=').nth(1) {
                let cleaned_name = name.trim().trim_matches('"').trim_matches('\'');
                if !cleaned_name.is_empty() {
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

    let path = Path::new(project_path);
    if let Some(dir_name) = path.file_name() {
        if let Some(name_str) = dir_name.to_str() {
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
