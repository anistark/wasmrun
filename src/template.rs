pub fn generate_html(filename: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
  <title>Chakra - Running {filename}</title>
</head>
<body style="background-color: #121212; color: white; font-family: monospace; text-align: center; margin-top: 50px;">
<h1>🧿 Chakra</h1>
<h2>Loaded {filename}</h2>
<script type="module">
async function init() {{
  const response = await fetch('{filename}');
  const importObject = {{
    env: {{
      console_log: () => {{
        console.log('📢 Hello from WASM!');
      }}
    }}
  }};
  const {{ instance }} = await WebAssembly.instantiateStreaming(response, importObject);
  console.log('🧿 WASM Module Loaded');
  if (instance.exports.main) {{
    instance.exports.main();
  }}
}}
init();
</script>
</body>
</html>
"#)
}
