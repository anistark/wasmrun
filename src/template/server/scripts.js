// Log messages
function log(message, type = 'info') {
    const logContainer = document.getElementById('log-container');
    const logEntry = document.createElement('div');
    logEntry.className = type;
    logEntry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
    logContainer.appendChild(logEntry);
    logContainer.scrollTop = logContainer.scrollHeight;
}

// Update status message
function updateStatus(message, isError = false) {
    const statusEl = document.getElementById('status');
    statusEl.textContent = message;
    statusEl.className = isError ? 'error' : 'success';
}

// Load WASM with retries
async function loadWasmWithRetries(retries = 5) {
    let attempt = 0;
    
    while (attempt < retries) {
        try {
            log(`Attempt ${attempt + 1}: Fetching WASM file '$FILENAME$'...`);
            
            const response = await fetch('/$FILENAME$');
            
            if (!response.ok) {
                throw new Error(`HTTP error! Status: ${response.status}`);
            }
            
            log(`WASM file fetched successfully, analyzing...`, 'success');
            const wasmBytes = await response.clone().arrayBuffer();
            log(`WASM module size: ${wasmBytes.byteLength} bytes`, 'info');
            
            try {
                const importObject = {
                    env: {
                        memory: new WebAssembly.Memory({ initial: 256 }),
                        console_log: (...args) => {
                            log(`📢 WASM function called console_log`, 'info');
                        }
                    }
                };
                log(`Attempting to instantiate WASM module...`, 'info');
                const { instance, module } = await WebAssembly.instantiateStreaming(
                    response,
                    importObject
                );
                
                log('✅ Chakra WASM Module loaded successfully!', 'success');
                updateStatus('✅ WASM Module loaded successfully!');
                
                // Call main function if it exists
                if (typeof instance.exports.main === 'function') {
                    log('Calling main() function...', 'info');
                    const result = instance.exports.main();
                    log(`main() returned: ${result}`, 'success');
                } else {
                    // List available exports
                    const exports = Object.keys(instance.exports);
                    log(`No main() function found. Available exports: ${exports.join(', ')}`, 'info');
                    
                    // Try to find a likely entry point
                    const likelyEntryPoints = ['_start', 'start', 'init', 'run', 'execute'];
                    for (const entryPoint of likelyEntryPoints) {
                        if (typeof instance.exports[entryPoint] === 'function') {
                            log(`Calling ${entryPoint}() function...`, 'info');
                            try {
                                const result = instance.exports[entryPoint]();
                                log(`${entryPoint}() returned: ${result}`, 'success');
                                break;
                            } catch (e) {
                                log(`Error calling ${entryPoint}(): ${e.message}`, 'error');
                            }
                        }
                    }
                }
                window.wasmInstance = instance;
                log('WASM instance exported as "window.wasmInstance" for console access', 'info');
                
                return;
            } catch (err) {
                log(`Error during instantiation: ${err.message}`, 'error');
                if (err.message.includes('function import requires a callable') || 
                    err.message.includes('Import #0')) {
                    updateStatus(`⚠️ This appears to be a wasm-bindgen module`, false);
                    log(`This WASM file appears to be compiled with wasm-bindgen`, 'info');
                    log(`These modules require their JavaScript glue code to run`, 'info');
                    log(`Try running the original JavaScript file that loads this WASM`, 'info');
                    const infoBox = document.createElement('div');
                    infoBox.className = 'info-box';
                    infoBox.innerHTML = `
                        <h3>⚠️ Advanced WASM Module Detected</h3>
                        <p>This WASM file appears to require JavaScript bindings that Chakra cannot automatically provide.</p>
                        <p>This is common for modules compiled with wasm-bindgen or similar tools.</p>
                        <h4>Suggestions:</h4>
                        <ul>
                            <li>Use the JavaScript file that was generated alongside this WASM file</li>
                            <li>For Rust wasm-bindgen projects, use <code>wasm-pack</code> to build and run</li>
                            <li>Simple C/C++ WASM files without JS bindings work best with Chakra</li>
                        </ul>
                    `;
                    document.body.appendChild(infoBox);
                    
                    return;
                }
                
                throw err;
            }
            
        } catch (err) {
            log(`Error: ${err.message}`, 'error');
            attempt++;
            
            if (attempt >= retries) {
                updateStatus(`❌ Failed to load WASM after ${retries} attempts`, true);
                log('All retry attempts failed', 'error');
            } else {
                log(`Retrying in 2 seconds... (${attempt}/${retries})`, 'info');
                await new Promise(resolve => setTimeout(resolve, 2000));
            }
        }
    }
}

loadWasmWithRetries();
