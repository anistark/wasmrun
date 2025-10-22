use crate::error::{Result, WasmrunError};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum TemplateType {
    Console,
    App,
}

impl TemplateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TemplateType::Console => "console",
            TemplateType::App => "app",
        }
    }
}

#[derive(Debug)]
pub struct Template {
    pub html: String,
    pub css: String,
    pub js: String,
    pub wasi_js: Option<String>,
}

pub struct TemplateManager {
    templates: HashMap<TemplateType, Template>,
    template_dir: PathBuf,
}

impl TemplateManager {
    pub fn new() -> Result<Self> {
        // First, try to find templates relative to the current executable
        // This works for: cargo run (debug), cargo install, and system installations
        let template_dir = match Self::find_templates_dir() {
            Some(dir) => dir,
            None => PathBuf::from("templates"), // Fallback for development scenarios
        };

        let mut manager = Self {
            templates: HashMap::new(),
            template_dir,
        };
        manager.load_templates()?;
        Ok(manager)
    }

    // Find templates directory by checking multiple locations
    fn find_templates_dir() -> Option<PathBuf> {
        // 1. Check relative to current directory first (for development/cargo run scenarios)
        let cwd_templates = PathBuf::from("templates");
        if cwd_templates.exists() && Self::is_valid_templates_dir(&cwd_templates) {
            return Some(cwd_templates.canonicalize().ok()?);
        }

        // 2. Check relative to executable (for installed versions)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Check both alongside executable and in parent directories
                let mut check_path = exe_dir.to_path_buf();
                for _ in 0..3 {
                    let templates_path = check_path.join("templates");
                    if templates_path.exists() && Self::is_valid_templates_dir(&templates_path) {
                        return Some(templates_path);
                    }
                    if !check_path.pop() {
                        break;
                    }
                }
            }
        }

        // 3. Search up from current directory (fallback for nested execution)
        if let Ok(mut current) = std::env::current_dir() {
            for _ in 0..10 {
                let templates_path = current.join("templates");
                if templates_path.exists() && Self::is_valid_templates_dir(&templates_path) {
                    return Some(templates_path);
                }
                if !current.pop() {
                    break;
                }
            }
        }

        None
    }

    // Validate that a directory contains the required template subdirectories
    fn is_valid_templates_dir(path: &Path) -> bool {
        path.join("console").exists() && path.join("app").exists()
    }

    #[allow(dead_code)]
    pub fn with_template_dir<P: AsRef<Path>>(template_dir: P) -> Result<Self> {
        let mut manager = Self {
            templates: HashMap::new(),
            template_dir: template_dir.as_ref().to_path_buf(),
        };
        manager.load_templates()?;
        Ok(manager)
    }

    fn load_templates(&mut self) -> Result<()> {
        // Load console template
        let console_template = self.load_template(&TemplateType::Console)?;
        self.templates
            .insert(TemplateType::Console, console_template);

        // Load app template
        let app_template = self.load_template(&TemplateType::App)?;
        self.templates.insert(TemplateType::App, app_template);

        Ok(())
    }

    fn load_template(&self, template_type: &TemplateType) -> Result<Template> {
        let template_path = self.template_dir.join(template_type.as_str());

        if !template_path.exists() {
            return Err(WasmrunError::from(format!(
                "Template directory not found: {}",
                template_path.display()
            )));
        }

        let html = self.read_template_file(&template_path, "index.html")?;
        let css = self.read_template_file(&template_path, "style.css")?;
        let js = self.read_template_file(&template_path, "scripts.js")?;

        // Load WASI JS
        let wasi_js = match template_type {
            // TODO: Replace wasmrun_wasi_impl.js with some library or CDN link in the future
            TemplateType::Console => {
                if template_path.join("wasmrun_wasi_impl.js").exists() {
                    Some(self.read_template_file(&template_path, "wasmrun_wasi_impl.js")?)
                } else {
                    None
                }
            }
            TemplateType::App => {
                if template_path.join("wasmrun_wasi_impl.js").exists() {
                    Some(self.read_template_file(&template_path, "wasmrun_wasi_impl.js")?)
                } else {
                    None
                }
            }
        };

        Ok(Template {
            html,
            css,
            js,
            wasi_js,
        })
    }

    fn read_template_file(&self, template_path: &Path, filename: &str) -> Result<String> {
        let file_path = template_path.join(filename);
        fs::read_to_string(&file_path).map_err(|e| {
            WasmrunError::from(format!(
                "Failed to read template file {}: {}",
                file_path.display(),
                e
            ))
        })
    }

    pub fn generate_html(&self, template_type: &TemplateType, filename: &str) -> Result<String> {
        let template = self.templates.get(template_type).ok_or_else(|| {
            WasmrunError::from(format!("Template not found: {}", template_type.as_str()))
        })?;

        self.render_template(template, filename, false)
    }

    pub fn generate_html_with_watch_mode(
        &self,
        template_type: &TemplateType,
        filename: &str,
        watch_mode: bool,
    ) -> Result<String> {
        let template = self.templates.get(template_type).ok_or_else(|| {
            WasmrunError::from(format!("Template not found: {}", template_type.as_str()))
        })?;

        self.render_template(template, filename, watch_mode)
    }

    fn render_template(
        &self,
        template: &Template,
        filename: &str,
        watch_mode: bool,
    ) -> Result<String> {
        let watch_meta = if watch_mode {
            r#"<meta name="wasmrun-watch" content="true">"#
        } else {
            ""
        };

        let title = self.generate_title(filename);

        let mut html = template
            .html
            .replace("$FILENAME$", filename)
            .replace("$TITLE$", &title)
            .replace(
                "<!-- @style-placeholder -->",
                &format!("<style>\n{}\n</style>", template.css),
            );

        // Build script content
        let mut script_content = String::new();
        script_content.push_str(watch_meta);

        if let Some(wasi_js) = &template.wasi_js {
            script_content.push_str(&format!(
                "\n<script>\n// Wasmrun WASI implementation\n{wasi_js}\n</script>"
            ));
        }

        script_content.push_str(&format!(
            "\n<script>\n// Main script\n{}\n</script>",
            template.js.replace("$FILENAME$", filename)
        ));

        html = html.replace("<!-- @script-placeholder -->", &script_content);

        Ok(html)
    }

    fn generate_title(&self, filename: &str) -> String {
        let file_stem = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);
        format!("Wasmrun - {file_stem}")
    }

    #[allow(dead_code)]
    pub fn list_available_templates(&self) -> Vec<&TemplateType> {
        self.templates.keys().collect()
    }

    #[allow(dead_code)]
    pub fn has_template(&self, template_type: &TemplateType) -> bool {
        self.templates.contains_key(template_type)
    }
}

impl Default for TemplateManager {
    fn default() -> Self {
        Self::new().expect("Failed to load templates. Make sure the 'templates/' directory exists with console/ and app/ subdirectories.")
    }
}
