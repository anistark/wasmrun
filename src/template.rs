pub fn generate_html(filename: &str) -> String {
  format!(
      r#"<!DOCTYPE html>
<html>
<head>
<title>◉ Chakra - Running {filename}</title>
<link rel="icon" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'%3E%3Ccircle cx='50' cy='50' r='40' fill='%23805AD5'/%3E%3Ccircle cx='50' cy='50' r='25' fill='%236B46C1'/%3E%3Ccircle cx='50' cy='50' r='10' fill='%234A1D96'/%3E%3C/svg%3E" type="image/svg+xml">
<style>
body {{
  background-color: #121212;
  color: white;
  font-family: monospace;
  text-align: center;
  margin-top: 50px;
}}
.log-container {{
  background-color: #1e1e1e;
  border: 1px solid #444;
  border-radius: 5px;
  width: 80%;
  max-width: 800px;
  height: 300px;
  margin: 20px auto;
  padding: 10px;
  overflow-y: auto;
  text-align: left;
}}
.error {{
  color: #ff5555;
}}
.success {{
  color: #50fa7b;
}}
.info-box {{
  background-color: #2d3748;
  border-left: 4px solid #4299e1;
  padding: 16px;
  margin: 20px auto;
  max-width: 800px;
  border-radius: 4px;
  text-align: left;
}}

.info-box h3 {{
  margin-top: 0;
  color: #4299e1;
}}

.info-box ul {{
  margin-left: 20px;
  padding-left: 0;
}}

.info-box code {{
  background-color: #1a202c;
  padding: 2px 5px;
  border-radius: 3px;
}}
</style>
</head>
<body>
<h1>Welcome to Chakra</h1>
<h2>Loaded: {filename}</h2>

<div id="status" class="info">⏳ Loading WASM module...</div>
<div id="log-container" class="log-container"></div>

<script type="module">
// Helper function to log messages
function log(message, type = 'info') {{
  const logContainer = document.getElementById('log-container');
  const logEntry = document.createElement('div');
  logEntry.className = type;
  logEntry.textContent = `[${{new Date().toLocaleTimeString()}}] ${{message}}`;
  logContainer.appendChild(logEntry);
  logContainer.scrollTop = logContainer.scrollHeight;
}}

// Update status message
function updateStatus(message, isError = false) {{
  const statusEl = document.getElementById('status');
  statusEl.textContent = message;
  statusEl.className = isError ? 'error' : 'success';
}}

// Function to load WASM with retries
async function loadWasmWithRetries(retries = 5) {{
  let attempt = 0;
  
  while (attempt < retries) {{
      try {{
          log(`Attempt ${{attempt + 1}}: Fetching WASM file '{filename}'...`);
          
          const response = await fetch('/{filename}');
          
          if (!response.ok) {{
              throw new Error(`HTTP error! Status: ${{response.status}}`);
          }}
          
          log(`WASM file fetched successfully, instantiating...`, 'success');
          
          // Create import object with environment functions
          const importObject = {{
              env: {{
                  console_log: () => {{
                      // This is a placeholder for actual memory access
                      log('📢 WASM function called console_log', 'info');
                  }}
              }},
              wbg: {{
                  // This is a placeholder for wasm-bindgen functions
                  // Commonly needed functions for wasm-bindgen compiled files
                  __wbindgen_throw: (ptr, len) => {{
                      log('Error in WASM module', 'error');
                  }},
                  __wbindgen_string_new: (ptr, len) => {{
                      log('WASM created a new string', 'info');
                      return 0; // Return a placeholder value
                  }},
                  __wbg_log_1fc5c6edb3d7ddb3: (arg) => {{
                      log(`WASM log: ${{arg}}`, 'info');
                  }}
              }}
          }};
          
          // Instantiate the WebAssembly module
          const {{ instance, module }} = await WebAssembly.instantiateStreaming(
              response, 
              importObject
          );
          
          log('🧿 WASM Module loaded successfully!', 'success');
          updateStatus('✅ WASM Module loaded successfully!');
          
          // Call main function if it exists
          if (typeof instance.exports.main === 'function') {{
              log('Calling main() function...', 'info');
              const result = instance.exports.main();
              log(`main() returned: ${{result}}`, 'success');
          }} else {{
              log('No main() function found in the WASM module', 'info');
          }}
          
          return; // Successfully loaded and instantiated
      }} catch (err) {{
          log(`Error: ${{err.message}}`, 'error');
          attempt++;
          
          if (attempt >= retries) {{
              updateStatus(`❌ Failed to load WASM after ${{retries}} attempts`, true);
              log('All retry attempts failed', 'error');
          }} else {{
              log(`Retrying in 2 seconds... (${{attempt}}/${{retries}})`, 'info');
              await new Promise(resolve => setTimeout(resolve, 2000));
          }}
      }}
  }}
}}

// Start loading the WASM with retry logic
loadWasmWithRetries();
</script>
</body>
</html>
"#
  )
}