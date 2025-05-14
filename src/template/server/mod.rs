use std::fs;
use std::path::Path;

const INDEX_HTML: &str = include_str!("index.html");
const STYLE_CSS: &str = include_str!("style.css");
const SCRIPTS_JS: &str = include_str!("scripts.js");
const WASI_JS: &str = include_str!("chakra_wasi_impl.js");

/// Generate the complete HTML
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
                "<script>\n// Chakra WASI implementation\n{}\n</script>\n<script>\n// Main script\n{}\n</script>",
                WASI_JS,
                process_scripts(filename)
            ),
        )
}

/// Process the JavaScript template, replacing any placeholders
fn process_scripts(filename: &str) -> String {
    SCRIPTS_JS.replace("$FILENAME$", filename)
}

// TODO: Alternative implementation that loads templates at runtime (for development)
// To see changes without recompiling
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

    let wasi_js = fs::read_to_string(template_dir.join("chakra_wasi_impl.js"))
        .unwrap_or_else(|_| "// Failed to load chakra_wasi_impl.js".to_string());

    html.replace("$FILENAME$", filename)
        .replace(
            "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n    </style>", css),
        )
        .replace(
            "<!-- @script-placeholder -->",
            &format!(
                "<script>\n// WASI implementation\n{}\n\n// Main script\n{}\n    </script>",
                wasi_js,
                js.replace("$FILENAME$", filename)
            ),
        )
}

/// Generate HTML for wasm-bindgen projects
pub fn generate_html_wasm_bindgen(js_filename: &str, _wasm_filename: &str) -> String {
    let js_only_filename = Path::new(js_filename)
        .file_name()
        .unwrap()
        .to_string_lossy();

    INDEX_HTML
        .replace("$FILENAME$", &js_only_filename)
        .replace(
            "<!-- @style-placeholder -->",
            &format!("<style>\n{}\n    </style>", STYLE_CSS),
        )
        .replace(
            "<!-- @script-placeholder -->",
            &format!(
                "<script>\n// Chakra WASI implementation - available but not used by wasm-bindgen\n{}\n</script>\n\
                <script>\n// Configure console logging for wasm-bindgen\nconst originalConsoleLog = console.log;\nconst originalConsoleError = console.error;\n\
                console.log = function(...args) {{\n  originalConsoleLog.apply(console, args);\n\
                  const logContainer = document.getElementById('log-container');\n\
                  if (logContainer) {{\n\
                    const logEntry = document.createElement('div');\n\
                    logEntry.className = 'info';\n\
                    logEntry.textContent = `[${{new Date().toLocaleTimeString()}}] ${{args.join(' ')}}`;\n\
                    logContainer.appendChild(logEntry);\n\
                    logContainer.scrollTop = logContainer.scrollHeight;\n\
                  }}\n}};\n\
                console.error = function(...args) {{\n  originalConsoleError.apply(console, args);\n\
                  const logContainer = document.getElementById('log-container');\n\
                  if (logContainer) {{\n\
                    const logEntry = document.createElement('div');\n\
                    logEntry.className = 'error';\n\
                    logEntry.textContent = `[${{new Date().toLocaleTimeString()}}] ❌ ${{args.join(' ')}}`;\n\
                    logContainer.appendChild(logEntry);\n\
                    logContainer.scrollTop = logContainer.scrollHeight;\n\
                  }}\n}};\n\
                </script>\n\
                <script type=\"module\">\n// Log initial message\nconsole.log(\"Loading wasm-bindgen module: {}\");\n\
                // Import the JS bindings\nimport init from \"./{}\";\n\
                async function runWasmBindgen() {{\n  try {{\n\
                    // Initialize the module\n    console.log(\"Initializing wasm-bindgen module...\");\n\
                    await init();\n    console.log(\"✅ wasm-bindgen module initialized successfully!\");\n\
                    document.getElementById('status').textContent = \"✅ WASM Module loaded successfully!\";\n\
                    document.getElementById('status').className = \"success\";\n\
                  }} catch (e) {{\n    console.error(\"Error initializing wasm-bindgen module:\", e);\n\
                    document.getElementById('status').textContent = \"❌ Error initializing WASM module\";\n\
                    document.getElementById('status').className = \"error\";\n  }}\n}}\n\
                // Run when the DOM is ready\ndocument.addEventListener('DOMContentLoaded', function() {{\n\
                  console.log(\"DOM loaded, initializing wasm-bindgen...\");\n  runWasmBindgen();\n\
                  // Set up tabs\n  const tabButtons = document.querySelectorAll('.tab-button');\n\
                  if (tabButtons) {{\n    tabButtons.forEach(button => {{\n\
                      button.addEventListener('click', () => {{\n\
                        document.querySelectorAll('.tab-button').forEach(btn => btn.classList.remove('active'));\n\
                        document.querySelectorAll('.tab-pane').forEach(pane => pane.classList.remove('active'));\n\
                        button.classList.add('active');\n\
                        const tabId = button.getAttribute('data-tab');\n\
                        document.getElementById(`${{tabId}}-tab`).classList.add('active');\n\
                      }});\n    }});\n  }}\n}});\n\
                </script>",
                WASI_JS,
                js_only_filename,
                js_only_filename
            ),
        )
}
