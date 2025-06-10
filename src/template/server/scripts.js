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

// Detect if a module requires WASI by examining imports
function detectWasiModule(wasmBytes) {
    // Skip analysis for large files (TODO: figure out browser restriction)
    if (wasmBytes.byteLength > 8 * 1024 * 1024) {
        log("WASM file is larger than 8MB. Skipping detailed analysis and assuming WASI support may be needed.", 'info');
        return true; // Assume it might be a WASI module
    }

    try {
        const module = new WebAssembly.Module(wasmBytes);
        const imports = WebAssembly.Module.imports(module);
        
        // Check if any import is from a WASI namespace
        return imports.some(imp => 
            imp.module === 'chakra_wasi_impl' || 
            imp.module === 'wasi_unstable' ||
            imp.module === 'wasi'
        );
    } catch (err) {
        log(`Error detecting WASI: ${err.message}`, 'error');
        // return false;
        return true; // Assume it might be a WASI module if we can't detect. TODO: Revisit this.
    }
}

// Set up virtual file system for WASI
function setupVirtualFileSystem(wasi) {
    // Create some demo files in the virtual filesystem
    wasi.createVirtualFile('/hello.txt', 'Hello from Chakra virtual filesystem!');
    wasi.createVirtualFile('/example.json', JSON.stringify({
        name: "Chakra",
        description: "WebAssembly runner with WASI support",
        version: "0.2.0"
    }, null, 2));
    
    // Create a readme file
    wasi.createVirtualFile('/README.md', `# Chakra WASI Virtual Filesystem

This is a virtual filesystem created by Chakra for WASI modules.
You can create, edit, and manipulate files directly in this interface.

## Usage
- Click on files to view and edit their content
- Use the "New File" and "New Directory" buttons to create new items
- Changes are saved in memory and will be lost when you refresh the page

Try running a WASI module that reads or writes files to see it interact with this filesystem!`);
    
    // Create an examples directory with a C example
    wasi.fs.mkdir('/examples');
    wasi.createVirtualFile('/examples/hello.c', `#include <stdio.h>

int main() {
    printf("Hello from WASI in Chakra!\\n");
    
    // Open and read a file
    FILE *f = fopen("/hello.txt", "r");
    if (f) {
        char buffer[100];
        if (fgets(buffer, sizeof(buffer), f)) {
            printf("File content: %s\\n", buffer);
        }
        fclose(f);
    }
    
    // Write to a new file
    FILE *fw = fopen("/output.txt", "w");
    if (fw) {
        fprintf(fw, "This file was created by a WASI program!\\n");
        fclose(fw);
    }
    
    return 0;
}`);
    
    log('Virtual filesystem initialized with sample files', 'info');
}

// Load WASM with retries
async function loadWasmWithRetries(retries = 5) {
    let attempt = 0;

    if (!window.WASI || !window.WASI.WASIImplementation) {
        log("WASI implementation not found! Make sure chakra_wasi_impl.js is properly loaded.", 'error');
        updateStatus("❌ WASI implementation not loaded", true);
        return;
    }

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
            
            // Check if this is a WASI module
            const isWasiModule = detectWasiModule(wasmBytes);
            if (isWasiModule) {
                log(`Detected WASI module, initializing WASI runtime...`, 'info');
            } else {
                log(`Standard WebAssembly module detected`, 'info');
            }
            
            try {
                let importObject = {
                    env: {
                        memory: new WebAssembly.Memory({ initial: 256 }),
                        console_log: (...args) => {
                            log(`📢 WASM function called console_log: ${args.join(', ')}`, 'info');
                        }
                    }
                };
                
                // Add WASI support if needed
                let wasi = null;
                if (isWasiModule) {
                    wasi = new window.WASI.WASIImplementation({
                        args: ['$FILENAME$'],
                        env: {
                            'CHAKRA': '1',
                            'HOME': '/',
                            'PATH': '/bin:/usr/bin'
                        },
                        preopens: {
                            '/': '/',
                            '/tmp': '/tmp'
                        },
                        stdout: (text) => {
                            log(`📢 WASM stdout: ${text}`, 'info');
                        },
                        stderr: (text) => {
                            log(`❌ WASM stderr: ${text}`, 'error');
                        }
                    });
                    
                    setupVirtualFileSystem(wasi);
                    
                    const wasiImports = wasi.getImportObject();
                    
                    importObject = {
                        ...importObject,
                        ...wasiImports
                    };
                    
                    log('WASI runtime initialized with virtual filesystem', 'success');
                }
                
                log(`Attempting to instantiate WASM module...`, 'info');
                const { instance, module } = await WebAssembly.instantiateStreaming(
                    response,
                    importObject
                );
                
                log('✅ WASM Module loaded successfully!', 'success');
                updateStatus('✅ WASM Module loaded successfully!');
                
                updateModuleInfo(module, isWasiModule);
                
                // For WASI modules, initialize and run
                if (isWasiModule && wasi) {
                    try {
                        // Initialize the WASI instance
                        wasi.initialize(instance);
                        log('WASI instance initialized', 'success');
                        
                        // Available exports
                        const exports = Object.keys(instance.exports);
                        // log(`Available exports: ${exports.join(', ')}`, 'info');
                        
                        if (typeof instance.exports._start === 'function') {
                            log('Calling WASI _start() function...', 'info');
                            try {
                                instance.exports._start();
                                log('WASI _start() completed successfully', 'success');
                            } catch (e) {
                                if (e.message.includes('unreachable') || e.message.includes('exit')) {
                                    log('WASI program completed execution (via exit)', 'success');
                                } else {
                                    log(`Error in WASI execution: ${e.message}`, 'error');
                                }
                            }
                        } else if (typeof instance.exports.main === 'function') {
                            log('Calling main() function...', 'info');
                            const result = instance.exports.main();
                            log(`main() returned: ${result}`, 'success');
                        } else {
                            log('No _start or main function found in WASI module', 'info');
                        }
                        
                        try {
                            const files = wasi.fs.readdir('/');
                            log(`Files in root directory: ${files.join(', ')}`, 'info');
                        } catch (e) {
                            log(`Error reading filesystem: ${e.message}`, 'error');
                        }
                        
                        createWasiUI(wasi, instance);
                    } catch (wasiError) {
                        log(`WASI error: ${wasiError.message}`, 'error');
                    }
                } else {
                    if (typeof instance.exports.main === 'function') {
                        log('Calling main() function...', 'info');
                        const result = instance.exports.main();
                        log(`main() returned: ${result}`, 'success');
                    } else {
                        const exports = Object.keys(instance.exports);
                        log(`No main() function found. Available exports: ${exports.join(', ')}`, 'info');
                        
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
                }
                
                window.wasmInstance = instance;
                if (wasi) {
                    window.wasi = wasi;
                }
                log('WASM instance exported as "window.wasmInstance" for console access', 'info');
                
                return;
            } catch (err) {
                log(`Error during instantiation: ${err.message}`, 'error');

                const validWasm = await analyzeWasmBinary(wasmBytes);
                if (!validWasm) {
                    updateStatus(`⚠️ WASM format issues detected, see console for details`, true);
                }

                // Handle specific WASI-related errors
                if (err.message.includes('chakra_wasi_impl') || 
                    err.message.includes('WASI') ||
                    err.message.includes('import')) {
                    
                    log('WASI module requires specific imports that might be missing', 'error');
                    updateStatus('⚠️ WASI module failed to initialize', true);
                    
                    const infoBox = document.createElement('div');
                    infoBox.className = 'info-box';
                    infoBox.innerHTML = `
                        <h3>⚠️ WASI Module Initialization Error</h3>
                        <p>This WebAssembly module uses WASI functionality but encountered an error during initialization.</p>
                        <p>Error details: ${err.message}</p>
                        <h4>Possible causes:</h4>
                        <ul>
                            <li>Module requires specific WASI functions that aren't implemented yet</li>
                            <li>Module expects a specific filesystem layout not provided in the virtual filesystem</li>
                            <li>Module uses advanced WASI features beyond the current implementation</li>
                        </ul>
                    `;
                    document.body.appendChild(infoBox);
                    return;
                } else if (err.message.includes('function import requires a callable') || 
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

// Update the Module Info tab with details about the Wasm module
function updateModuleInfo(module, isWasi) {
    const moduleDetails = document.getElementById('module-details');
    if (!moduleDetails) return;
    
    // Get module information
    const imports = WebAssembly.Module.imports(module);
    const exports = WebAssembly.Module.exports(module);
    
    // Create imports table
    let importsList = '';
    const importsByModule = {};
    imports.forEach(imp => {
        if (!importsByModule[imp.module]) {
            importsByModule[imp.module] = [];
        }
        importsByModule[imp.module].push(imp);
    });
    
    for (const [module, moduleImports] of Object.entries(importsByModule)) {
        importsList += `<strong>${module}</strong>: `;
        importsList += moduleImports.map(imp => `${imp.name} (${imp.kind})`).join(', ');
        importsList += '<br>';
    }
    
    // Create exports list
    const exportsByKind = {
        function: [],
        global: [],
        memory: [],
        table: []
    };
    
    exports.forEach(exp => {
        if (exportsByKind[exp.kind]) {
            exportsByKind[exp.kind].push(exp.name);
        }
    });
    
    let exportsList = '';
    for (const [kind, items] of Object.entries(exportsByKind)) {
        if (items.length > 0) {
            exportsList += `<strong>${kind}s</strong>: `;
            exportsList += items.join(', ');
            exportsList += '<br>';
        }
    }
    
    moduleDetails.innerHTML = `
        <div class="module-section">
            <h4>Module Type</h4>
            <p>${isWasi ? 'WASI Module (WebAssembly System Interface)' : 'Standard WebAssembly Module'}</p>
        </div>
        
        <div class="module-section">
            <h4>Imports (${imports.length})</h4>
            <p>${imports.length > 0 ? importsList : 'No imports'}</p>
        </div>
        
        <div class="module-section">
            <h4>Exports (${exports.length})</h4>
            <p>${exports.length > 0 ? exportsList : 'No exports'}</p>
        </div>
        
        <div class="module-section">
            <h4>Entry Point</h4>
            <p>${getEntryPoint(exports)}</p>
        </div>
    `;
}

// Get module entry point
function getEntryPoint(exports) {
    const exportNames = exports.map(exp => exp.name);
    
    if (exportNames.includes('_start')) {
        return '<code>_start</code> (WASI standard entry point)';
    } else if (exportNames.includes('main')) {
        return '<code>main</code>';
    } else if (exportNames.includes('start')) {
        return '<code>start</code>';
    } else if (exportNames.includes('run')) {
        return '<code>run</code>';
    } else if (exportNames.includes('init')) {
        return '<code>init</code>';
    } else {
        return 'No standard entry point detected';
    }
}

// Create UI elements for interacting with the WASI filesystem
function createWasiUI(wasi, instance) {
    const fsTab = document.getElementById('filesystem-tab');
    if (!fsTab) return;
    
    fsTab.innerHTML = '';
    
    // File Explorer UI
    const explorer = document.createElement('div');
    explorer.className = 'wasi-explorer';
    explorer.innerHTML = `
        <div class="wasi-header">
            <h3>WASI Virtual Filesystem</h3>
            <button id="wasi-new-file" class="wasi-button">New File</button>
            <button id="wasi-new-dir" class="wasi-button">New Directory</button>
        </div>
        <div class="wasi-content">
            <div class="wasi-sidebar">
                <h4>Files</h4>
                <div id="wasi-file-tree" class="wasi-file-tree"></div>
            </div>
            <div class="wasi-main">
                <div class="wasi-file-header">
                    <h4 id="wasi-file-name">Select a file</h4>
                    <div class="wasi-file-actions">
                        <button id="wasi-save-file" class="wasi-button" disabled>Save</button>
                        <button id="wasi-run-file" class="wasi-button" disabled>Run</button>
                    </div>
                </div>
                <textarea id="wasi-file-editor" class="wasi-file-editor" placeholder="Select a file to view its contents"></textarea>
            </div>
        </div>
    `;
    
    fsTab.appendChild(explorer);
    
    updateFileTree(wasi);
    
    document.getElementById('wasi-new-file').addEventListener('click', () => {
        const filename = prompt('Enter file name:');
        if (!filename) return;
        
        const path = '/' + filename;
        wasi.createVirtualFile(path, '');
        log(`Created file: ${path}`, 'success');
        updateFileTree(wasi);
    });
    
    document.getElementById('wasi-new-dir').addEventListener('click', () => {
        const dirname = prompt('Enter directory name:');
        if (!dirname) return;
        
        const path = '/' + dirname;
        wasi.fs.mkdir(path);
        log(`Created directory: ${path}`, 'success');
        updateFileTree(wasi);
    });
    
    document.getElementById('wasi-save-file').addEventListener('click', () => {
        const filename = document.getElementById('wasi-file-name').textContent;
        const content = document.getElementById('wasi-file-editor').value;
        
        wasi.createVirtualFile(filename, content);
        log(`Saved file: ${filename}`, 'success');
    });
    
    document.getElementById('wasi-run-file').addEventListener('click', () => {
        const filename = document.getElementById('wasi-file-name').textContent;
        log(`Running file ${filename} is not implemented yet`, 'info');
        // TODO: [TOL] Future implementation could compile and run script files
    });
}

// Update the file tree display
function updateFileTree(wasi) {
    const tree = document.getElementById('wasi-file-tree');
    if (!tree) return;
    
    tree.innerHTML = '';
    
    function addDirectoryToTree(path, parentElement) {
        try {
            const files = wasi.fs.readdir(path);
            if (!files) return;
            
            for (const file of files) {
                if (file === '.' || file === '..') continue;
                
                const filePath = path === '/' ? '/' + file : path + '/' + file;
                const fileInfo = wasi.fs.files.get(filePath);
                
                const fileElement = document.createElement('div');
                fileElement.className = fileInfo.type === 3 ? 'wasi-file-tree-dir' : 'wasi-file-tree-file';
                fileElement.innerHTML = `
                    <span class="wasi-file-name">${file}</span>
                `;
                
                if (fileInfo.type === 3) {
                    fileElement.addEventListener('click', (e) => {
                        e.stopPropagation();
                        fileElement.classList.toggle('expanded');
                        
                        const childContainer = fileElement.querySelector('.wasi-file-children');
                        if (!childContainer) {
                            const children = document.createElement('div');
                            children.className = 'wasi-file-children';
                            fileElement.appendChild(children);
                            addDirectoryToTree(filePath, children);
                        }
                    });
                } else {
                    fileElement.addEventListener('click', (e) => {
                        e.stopPropagation();
                        openFile(wasi, filePath);
                    });
                }
                
                parentElement.appendChild(fileElement);
            }
        } catch (e) {
            console.error(`Error reading directory ${path}:`, e);
        }
    }
    
    addDirectoryToTree('/', tree);
}

// Open a file in the editor
function openFile(wasi, path) {
    const editor = document.getElementById('wasi-file-editor');
    const fileNameEl = document.getElementById('wasi-file-name');
    const saveButton = document.getElementById('wasi-save-file');
    const runButton = document.getElementById('wasi-run-file');
    
    if (!editor || !fileNameEl) return;
    
    try {
        const content = wasi.readVirtualFile(path);
        editor.value = content || '';
        fileNameEl.textContent = path;
        saveButton.disabled = false;
        runButton.disabled = false;
    } catch (e) {
        log(`Error opening file ${path}: ${e.message}`, 'error');
    }
}

function listRequiredImports(module) {
    try {
        const imports = WebAssembly.Module.imports(module);
        log(`Module requires ${imports.length} imports:`, 'info');
        const wasiImports = imports.filter(imp => imp.module === 'wasi_snapshot_preview1');
        log(`WASI imports needed: ${wasiImports.map(imp => imp.name).join(', ')}`, 'info');
    } catch (e) {
        log(`Cannot analyze imports: ${e.message}`, 'info');
    }
}

// Activate a tab when clicked
document.addEventListener('DOMContentLoaded', function() {
    const tabButtons = document.querySelectorAll('.tab-button');
    
    if (tabButtons) {
        tabButtons.forEach(button => {
            button.addEventListener('click', () => {
                // Deactivate all tabs
                document.querySelectorAll('.tab-button').forEach(btn => btn.classList.remove('active'));
                document.querySelectorAll('.tab-pane').forEach(pane => pane.classList.remove('active'));
                
                // Activate the clicked tab
                button.classList.add('active');
                const tabId = button.getAttribute('data-tab');
                document.getElementById(`${tabId}-tab`).classList.add('active');
            });
        });
    }
});

async function analyzeWasmBinary(wasmBytes) {
    log("Starting detailed WASM binary analysis...", "info");
    
    // Check magic bytes - WASM files start with \0asm
    if (wasmBytes.byteLength < 8) {
        log("Error: File too small to be a valid WASM module", "error");
        return false;
    }
    
    const magicBytes = new Uint8Array(wasmBytes.slice(0, 4));
    const magicString = String.fromCharCode(...magicBytes);
    log(`Magic bytes: ${Array.from(magicBytes).map(b => b.toString(16).padStart(2, '0')).join(' ')}`, "info");
    log(`Magic string: "${magicString}"`, "info");
    
    if (magicString !== "\0asm") {
        log("Error: Invalid magic bytes, not a WASM file", "error");
        return false;
    }
    
    // Check version
    const version = new DataView(wasmBytes.slice(4, 8)).getUint32(0, true);
    log(`WASM version: ${version}`, "info");
    
    if (version !== 1) {
        log(`Warning: Unexpected WASM version. Expected 1, got ${version}`, "error");
    }
    
    try {
        let offset = 8;
        const view = new DataView(wasmBytes);
        
        log("Analyzing WASM sections:", "info");
        let sectionCount = 0;
        
        // Known section types
        const sectionTypes = [
            "Custom", "Type", "Import", "Function", "Table", 
            "Memory", "Global", "Export", "Start", "Element", 
            "Code", "Data", "DataCount"
        ];
        
        while (offset < wasmBytes.byteLength) {
            sectionCount++;
            
            // Read section ID (1 byte)
            if (offset >= wasmBytes.byteLength) break;
            const sectionId = view.getUint8(offset++);
            
            // Read section size (LEB128 variable-length encoding)
            let sectionSize = 0;
            let shift = 0;
            let byte;
            
            do {
                if (offset >= wasmBytes.byteLength) {
                    log(`Error: Unexpected end of file when reading section ${sectionCount} size`, "error");
                    return false;
                }
                
                byte = view.getUint8(offset++);
                sectionSize |= (byte & 0x7f) << shift;
                shift += 7;
            } while (byte & 0x80);
            
            const sectionName = sectionId < sectionTypes.length ? sectionTypes[sectionId] : `Unknown(${sectionId})`;
            log(`Section ${sectionCount}: ${sectionName} (ID: ${sectionId}, Size: ${sectionSize} bytes, Offset: 0x${(offset - 1).toString(16)})`, "info");
            
            if (offset - 1 <= 130 && offset - 1 + sectionSize >= 126) {
                log(`⚠️ The error might be in this section! Check if the Memory section is out of order.`, "error");
            }
            
            offset += sectionSize;
        }
        
        log(`Total sections found: ${sectionCount}`, "info");
        
        try {
            const module = new WebAssembly.Module(wasmBytes);
            const imports = WebAssembly.Module.imports(module);
            const exports = WebAssembly.Module.exports(module);
            
            log(`Module successfully parsed! (${imports.length} imports, ${exports.length} exports)`, "success");
            return true;
        } catch (e) {
            log(`Module parsing failed in verify stage: ${e.message}`, "error");
            return false;
        }
    } catch (e) {
        log(`Error during section analysis: ${e.message}`, "error");
        log(`Error occurred at around byte offset: ${e.offset || "unknown"}`, "error");
        return false;
    }
}

function setupLiveReload() {
    // Only set up live reload if we detect we're in a watch mode environment
    // Check if the page was loaded with watch mode indicators
    const urlParams = new URLSearchParams(window.location.search);
    const isWatchMode = urlParams.get('watch') === 'true' || 
                        document.querySelector('meta[name="chakra-watch"]') !== null;
    
    if (!isWatchMode) {
        log("Live reload not enabled (not in watch mode)", "info");
        return;
    }
    
    let reloadInterval = null;
    let lastReloadTime = 0;
    let consecutiveErrors = 0;
    
    // Poll the server for reload events
    function startPolling() {
        if (reloadInterval) return;
        
        log("Starting live reload polling...", "info");
        
        reloadInterval = setInterval(() => {
            fetch('/reload', { 
                cache: 'no-store',
                headers: {
                    'X-Requested-With': 'XMLHttpRequest'
                }
            })
            .then(response => {
                consecutiveErrors = 0; // Reset error count on successful request
                
                // Check for the reload header
                if (response.headers.get('X-Reload') === 'true') {
                    // Prevent multiple reloads in quick succession
                    const now = Date.now();
                    if (now - lastReloadTime < 2000) return;
                    
                    lastReloadTime = now;
                    log("🔄 Change detected - reloading WASM module...", "info");
                    
                    // No polling during reload.
                    // TODO: Consider a more graceful reload mechanism like reloading only the WASM module without a full page reload
                    clearInterval(reloadInterval);
                    reloadInterval = null;
                    
                    setTimeout(() => {
                        window.location.reload();
                    }, 500);
                } else {
                    // No reload needed, continue polling
                    // log("No changes detected", "info"); // Too verbose
                }
            })
            .catch(e => {
                consecutiveErrors++;
                
                if (consecutiveErrors > 5) {
                    log("Live reload: Server appears to be down, stopping polling", "info");
                    clearInterval(reloadInterval);
                    reloadInterval = null;
                } else {
                    // Ignore occasional errors - server might be restarting
                    // log(`Live reload polling error: ${e.message}`, "info");
                }
            });
        }, 2000); // Poll every 2 seconds instead of every second to reduce load
    }
    
    // Start polling
    startPolling();
    
    // Listen for visibility changes
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') {
            if (consecutiveErrors <= 5) {
                log("Tab is visible again - checking for updates...", "info");
                startPolling();
            }
        } else {
            clearInterval(reloadInterval);
            reloadInterval = null;
        }
    });
    
    // Handle window unload
    window.addEventListener('beforeunload', () => {
        clearInterval(reloadInterval);
    });
}

// Start loading the WASM file
loadWasmWithRetries();

// Initialize live reload (but only if in watch mode)
setupLiveReload();
