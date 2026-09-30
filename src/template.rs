//! [Server Mode] HTML templates for the UI port, embedded at build time
//!
//! `build.rs` builds the UI into `templates/`, and the pages are compiled into the binary from
//! there, so an installed `wasmrun` needs nothing on disk beside it.

use std::path::Path;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum TemplateType {
    Console,
    App,
}

struct Template {
    html: &'static str,
    css: &'static str,
    js: &'static str,
}

const CONSOLE: Template = Template {
    html: include_str!("../templates/console/index.html"),
    css: include_str!("../templates/console/style.css"),
    js: include_str!("../templates/console/scripts.js"),
};

const APP: Template = Template {
    html: include_str!("../templates/app/index.html"),
    css: include_str!("../templates/app/style.css"),
    js: include_str!("../templates/app/scripts.js"),
};

#[derive(Default)]
pub struct TemplateManager;

impl TemplateManager {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_html_with_watch_mode(
        &self,
        template_type: &TemplateType,
        filename: &str,
        watch_mode: bool,
    ) -> String {
        let template = match template_type {
            TemplateType::Console => &CONSOLE,
            TemplateType::App => &APP,
        };
        render(template, filename, watch_mode)
    }
}

fn render(template: &Template, filename: &str, watch_mode: bool) -> String {
    let watch_meta = if watch_mode {
        r#"<meta name="wasmrun-watch" content="true">"#
    } else {
        ""
    };

    let script_content = format!(
        "{watch_meta}\n<script>\n// Main script\n{}\n</script>",
        template.js.replace("$FILENAME$", filename)
    );

    template
        .html
        .replace("$FILENAME$", filename)
        .replace("$TITLE$", &title(filename))
        .replace(
            "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n</style>", template.css),
        )
        .replace("<!-- @script-placeholder -->", &script_content)
}

fn title(filename: &str) -> String {
    let file_stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    format!("Wasmrun - {file_stem}")
}
