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

    // Improved HTML template for wasm-bindgen based applications
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Chakra - Running {}</title>
    <link rel="icon" href="/assets/logo.png" type="image/png">
    <style>
    * {{
        box-sizing: border-box;
        margin: 0;
        padding: 0;
    }}

    body {{
        background-color: #121212;
        color: #FFFFFF;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
        line-height: 1.6;
        display: flex;
        flex-direction: column;
        min-height: 100vh;
    }}

    header {{
        display: flex;
        align-items: center;
        padding: 1rem 2rem;
        background-color: #1e1e2e;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    }}

    header h1 {{
        margin-left: 1rem;
        font-size: 1.8rem;
        font-weight: 600;
    }}

    .logo {{
        display: flex;
        align-items: center;
        justify-content: center;
    }}

    .logo img {{
        width: 40px;
        height: 40px;
    }}

    main {{
        flex: 1;
        display: flex;
        flex-direction: column;
    }}
    
    .status-bar {{
        background-color: #252b37;
        padding: 0.5rem 1rem;
        display: flex;
        justify-content: space-between;
        align-items: center;
        border-bottom: 1px solid #313244;
    }}
    
    .success {{
        color: #50fa7b;
    }}
    
    .error {{
        color: #ff5555;
    }}
    
    .info {{
        color: #8be9fd;
    }}
    
    .app-container {{
        flex: 1;
        padding: 0;
        position: relative;
        overflow: hidden;
    }}
    
    #wasm-bindgen-app {{
        width: 100%;
        height: 100%;
        overflow: auto;
    }}
    
    #console-tab {{
        display: none;
        position: absolute;
        bottom: 0;
        left: 0;
        width: 100%;
        height: 200px;
        background-color: rgba(0, 0, 0, 0.8);
        border-top: 1px solid #313244;
        overflow: auto;
        font-family: monospace;
        font-size: 0.9rem;
        padding: 0.5rem;
        z-index: 100;
    }}
    
    #console-toggle {{
        position: absolute;
        bottom: 1rem;
        right: 1rem;
        padding: 0.5rem;
        background-color: #1e1e2e;
        border: 1px solid #313244;
        border-radius: 4px;
        color: #cdd6f4;
        cursor: pointer;
        z-index: 101;
    }}
    
    #console-toggle:hover {{
        background-color: #313244;
    }}
    
    footer {{
        background-color: #1e1e2e;
        padding: 1rem 2rem;
        text-align: center;
        font-size: 0.9rem;
    }}
    
    /* Animation for the loading indicator */
    @keyframes spin {{
        0% {{ transform: rotate(0deg); }}
        100% {{ transform: rotate(360deg); }}
    }}
    
    .loading-indicator {{
        display: inline-block;
        width: 20px;
        height: 20px;
        border: 2px solid rgba(255, 255, 255, 0.3);
        border-radius: 50%;
        border-top-color: #fff;
        animation: spin 1s ease-in-out infinite;
        margin-right: 8px;
        vertical-align: middle;
    }}
    </style>
</head>
<body>
    <header>
        <div class="logo">
            <img src="/assets/logo.png" alt="Chakra Logo" width="40" height="40">
        </div>
        <h1>Chakra - Wasm-Bindgen App</h1>
    </header>
    
    <main>
        <div class="status-bar">
            <div id="status">
                <span class="loading-indicator"></span>
                Loading wasm-bindgen module...
            </div>
            <div>
                <button id="console-toggle">Toggle Console</button>
            </div>
        </div>
        
        <div class="app-container">
            <div id="wasm-bindgen-app"></div>
            <div id="console-tab"></div>
        </div>
    </main>
    
    <footer>
        Powered by Chakra with wasm-bindgen support
    </footer>
    
    <script>
    // Console logging setup
    const originalConsoleLog = console.log;
    const originalConsoleError = console.error;
    const originalConsoleWarn = console.warn;
    const originalConsoleInfo = console.info;

    // Function to add log to the console tab
    function addLogToConsole(message, type = 'info') {{
        const consoleTab = document.getElementById('console-tab');
        if (consoleTab) {{
            const logEntry = document.createElement('div');
            logEntry.className = type;
            logEntry.textContent = `[${{new Date().toLocaleTimeString()}}] ${{message}}`;
            consoleTab.appendChild(logEntry);
            consoleTab.scrollTop = consoleTab.scrollHeight;
        }}
    }}

    // Override console functions
    console.log = function(...args) {{
        originalConsoleLog.apply(console, args);
        addLogToConsole(args.join(' '), 'info');
    }};

    console.error = function(...args) {{
        originalConsoleError.apply(console, args);
        addLogToConsole(args.join(' '), 'error');
    }};

    console.warn = function(...args) {{
        originalConsoleWarn.apply(console, args);
        addLogToConsole(args.join(' '), 'warn');
    }};

    console.info = function(...args) {{
        originalConsoleInfo.apply(console, args);
        addLogToConsole(args.join(' '), 'info');
    }};

    // Console toggle functionality
    document.getElementById('console-toggle').addEventListener('click', function() {{
        const consoleTab = document.getElementById('console-tab');
        if (consoleTab.style.display === 'block') {{
            consoleTab.style.display = 'none';
        }} else {{
            consoleTab.style.display = 'block';
        }}
    }});
    </script>
    
    <script type="module">
    // Import the wasm-bindgen JS module
    import init from './{}';
    
    async function runWasmBindgen() {{
        try {{
            console.log("Initializing wasm-bindgen module...");
            // Check if the init function takes an argument (some frameworks require this)
            const initFn = init;
            const argCount = initFn.length;
            
            let result;
            if (argCount > 0) {{
                // The init function expects options or arguments, provide a target element
                result = await initFn({{
                    root: document.getElementById('wasm-bindgen-app')
                }});
            }} else {{
                // Standard init with no arguments
                result = await initFn();
            }}
            
            console.log("✅ wasm-bindgen module initialized successfully!");
            document.getElementById('status').innerHTML = "✅ WASM Module loaded successfully!";
            document.getElementById('status').className = "success";
            
            // If the module returned something, log it
            if (result) {{
                console.log("Module initialization returned:", result);
            }}
        }} catch (e) {{
            console.error("❌ Error initializing wasm-bindgen module:", e);
            document.getElementById('status').textContent = "❌ Error initializing WASM module";
            document.getElementById('status').className = "error";
            
            // Show a more detailed error
            document.getElementById('wasm-bindgen-app').innerHTML = `
                <div style="padding: 2rem; border: 2px solid #ff5555; border-radius: 8px; margin: 2rem;">
                    <h2 style="color: #ff5555; margin-bottom: 1rem;">Error Loading WASM Module</h2>
                    <p>There was an error initializing the wasm-bindgen module:</p>
                    <pre style="background: #1a1a1a; padding: 1rem; overflow: auto; border-radius: 4px;">${{e.message}}</pre>
                    <p style="margin-top: 1rem;">Check the browser console for more details.</p>
                </div>
            `;
        }}
    }}
    
    // Run the initialization when the page loads
    runWasmBindgen();
    </script>
</body>
</html>"#,
        js_only_filename, js_only_filename
    )
}
