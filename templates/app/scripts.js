// App template JavaScript for WASM applications

async function initializeWasm() {
    try {
        console.log("Loading WASM module: $FILENAME$");
        
        // For wasm-bindgen projects
        if (typeof init !== 'undefined') {
            console.log("Initializing wasm-bindgen module...");
            const wasmModule = await init();
            console.log("✅ WASM module loaded successfully!");
            updateStatus("✅ WASM Module loaded successfully!", "success");
            return wasmModule;
        }
        
        // For regular WASM modules
        const response = await fetch('$FILENAME$');
        const wasmModule = await WebAssembly.instantiateStreaming(response);
        console.log("✅ WASM module loaded successfully!");
        updateStatus("✅ WASM Module loaded successfully!", "success");
        return wasmModule;
        
    } catch (error) {
        console.error("❌ Error loading WASM module:", error);
        updateStatus("❌ Error loading WASM module", "error");
        showError(error.message);
        throw error;
    }
}

function updateStatus(message, type = "info") {
    const statusElement = document.getElementById('status');
    if (statusElement) {
        statusElement.innerHTML = message;
        statusElement.className = type;
    }
}

function showError(errorMessage) {
    const appContainer = document.getElementById('wasm-app');
    if (appContainer) {
        appContainer.innerHTML = `
            <div style="padding: 2rem; border: 2px solid #ff5555; border-radius: 8px; margin: 2rem;">
                <h2 style="color: #ff5555; margin-bottom: 1rem;">Error Loading WASM Module</h2>
                <p>There was an error loading the WASM module:</p>
                <pre style="background: #1a1a1a; padding: 1rem; overflow: auto; border-radius: 4px; margin: 1rem 0;">${errorMessage}</pre>
                <p>Check the browser console for more details.</p>
            </div>
        `;
    }
}

// Initialize when page loads
document.addEventListener('DOMContentLoaded', initializeWasm);