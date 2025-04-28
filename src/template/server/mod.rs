use std::fs;
use std::path::Path;

// Include the template files at compile time
const INDEX_HTML: &str = include_str!("index.html");
const STYLE_CSS: &str = include_str!("style.css");
const SCRIPTS_JS: &str = include_str!("scripts.js");

/// Generate the complete HTML by combining all template parts and replacing placeholders
pub fn generate_html(filename: &str) -> String {
    INDEX_HTML
        .replace("$FILENAME$", filename)
        .replace(
        "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n    </style>", STYLE_CSS),
        )
        .replace(
        "<!-- @script-placeholder -->",
          &format!(
               "<script type=\"module\">\n{}\n    </script>",
               process_scripts(filename)
            ),
        )
}

/// Process the JavaScript template, replacing any placeholders
fn process_scripts(filename: &str) -> String {
    SCRIPTS_JS.replace("$FILENAME$", filename)
}

// Alternative implementation that loads templates at runtime (for development)
// Only enable this when developing to see changes without recompiling
#[allow(dead_code)]
pub fn generate_html_dev(filename: &str) -> String {
    let template_dir = Path::new("src/template/server");

    // Load templates from files at runtime
    let html = fs::read_to_string(template_dir.join("index.html"))
        .unwrap_or_else(|_| "Failed to load index.html".to_string());

    let css = fs::read_to_string(template_dir.join("style.css"))
        .unwrap_or_else(|_| "/* Failed to load style.css */".to_string());

    let js = fs::read_to_string(template_dir.join("scripts.js"))
        .unwrap_or_else(|_| "// Failed to load scripts.js".to_string());

    html
        .replace("$FILENAME$", filename)
        .replace(
            "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n    </style>", css),
        )
        .replace(
            "<!-- @script-placeholder -->",
            &format!(
                "<script type=\"module\">\n{}\n    </script>",
                js.replace("$FILENAME$", filename)
            ),
        )
}
