// Chakra Dev Tools
(function() {
    // Setup toggles
    const barToggle = document.getElementById('bar-toggle');
    const consoleToggle = document.getElementById('console-toggle');
    const devBar = document.getElementById('chakra-dev-bar');
    const devConsole = document.getElementById('dev-console');
    
    // Toggle dev bar
    barToggle.addEventListener('click', () => {
        devBar.classList.toggle('collapsed');
        barToggle.innerHTML = devBar.classList.contains('collapsed') 
            ? '<span>⬇️</span> Show' 
            : '<span>⬆️</span> Hide';
    });
    
    // Toggle console
    consoleToggle.addEventListener('click', () => {
        devConsole.classList.toggle('visible');
    });
    
    // Console logging override
    const originalConsoleLog = console.log;
    const originalConsoleError = console.error;
    const originalConsoleWarn = console.warn;
    
    function addLogToConsole(message, type = 'info') {
        const logEntry = document.createElement('div');
        logEntry.className = type;
        logEntry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
        devConsole.appendChild(logEntry);
        devConsole.scrollTop = devConsole.scrollHeight;
    }
    
    console.log = function(...args) {
        originalConsoleLog.apply(console, args);
        addLogToConsole(args.join(' '), 'info');
    };
    
    console.error = function(...args) {
        originalConsoleError.apply(console, args);
        addLogToConsole(args.join(' '), 'error');
    };
    
    console.warn = function(...args) {
        originalConsoleWarn.apply(console, args);
        addLogToConsole(args.join(' '), 'warning');
    };
    
    // Status update function
    window.updateChakraStatus = function(message, type = 'info') {
        const statusEl = document.getElementById('chakra-status');
        statusEl.className = `chakra-status ${type}`;
        statusEl.innerHTML = `<span>${message}</span>`;
    };
    
    // Resource loading timer
    const startTime = performance.now();
    window.addEventListener('load', () => {
        const loadTime = Math.round(performance.now() - startTime);
        const statusEl = document.getElementById('chakra-status');
        
        if (!statusEl.classList.contains('error')) {
            statusEl.innerHTML += `<span class="resource-timer">(${loadTime}ms)</span>`;
        }
    });

    // Live reload support
    setupLiveReload();
})();

// Live reload polling
function setupLiveReload() {
    let reloadTimer = null;
    
    function checkForReload() {
        if (reloadTimer) clearTimeout(reloadTimer);
        
        fetch('/reload-check?t=' + Date.now(), {
            cache: 'no-store'
        })
        .then(response => {
            if (response.headers.get('X-Reload-Needed') === 'true') {
                console.log("⚡ Change detected - reloading page now...");
                window.location.reload();
                return;
            }
            
            // Schedule next check
            reloadTimer = setTimeout(checkForReload, 1000);
        })
        .catch(err => {
            console.log("⚠️ Reload check error, retrying in 2s...", err);
            reloadTimer = setTimeout(checkForReload, 2000);
        });
    }
    
    // Start checking for reloads
    checkForReload();
    
    // Handle visibility changes
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') {
            console.log("Tab is visible again - resuming reload checks");
            checkForReload();
        } else {
            if (reloadTimer) clearTimeout(reloadTimer);
        }
    });
}

// App Initialization
document.addEventListener('DOMContentLoaded', async function() {
    try {
        console.log("Initializing Rust web application...");
        
        // Import the application
        // First approach - Add a leading slash to ensure absolute path
        const modulePath = '/$JS_ENTRYPOINT$';
        console.log(`Attempting to import module from: ${modulePath}`);
        
        let module;
        try {
            module = await import(modulePath);
        } catch (importError) {
            console.warn(`Failed to import using absolute path: ${importError.message}`);
            
            // Second approach - Try without leading slash
            const relativePath = '$JS_ENTRYPOINT$';
            console.log(`Trying alternate import path: ${relativePath}`);
            module = await import(relativePath);
        }
        
        // Get the initialization function - accommodate different export styles
        const init = module.default || module.init || module;
        
        if (typeof init !== 'function') {
            throw new Error(`Could not find initialization function in module. Exports available: ${Object.keys(module).join(', ')}`);
        }
        
        console.log("Found initialization function, starting app...");
        
        // Initialize the application - try with target first
        try {
            await init({
                target: document.getElementById('app')
            });
            console.log("App initialized with target option");
        } catch (targetError) {
            console.warn(`Initialization with target failed: ${targetError.message}. Trying without options...`);
            
            // If initialization with options fails, try without options
            await init();
            console.log("App initialized without options");
        }
        
        // Update status on success
        window.updateChakraStatus('Application loaded successfully ✅', 'success');
        console.log("Rust web application initialized successfully!");
    } catch (error) {
        // Handle errors during initialization
        window.updateChakraStatus(`Error loading application ❌`, 'error');
        console.error("Failed to initialize application:", error);
        
        // Show error in app container with more detailed information
        document.getElementById('app').innerHTML = `
            <div style="padding: 2rem; color: #ff5555; max-width: 800px; margin: 2rem auto; border: 1px solid #ff5555; border-radius: 8px;">
                <h2>Application Failed to Load</h2>
                <p>There was an error initializing the Rust application:</p>
                <pre style="background: rgba(0,0,0,0.1); padding: 1rem; border-radius: 4px; overflow: auto; white-space: pre-wrap;">${error.message}</pre>
                
                <h3 style="margin-top: 1.5rem;">Debugging Information</h3>
                <p><strong>Module Path:</strong> $JS_ENTRYPOINT$</p>
                <p><strong>Error Type:</strong> ${error.name}</p>
                ${error.stack ? `<p><strong>Stack Trace:</strong></p><pre style="background: rgba(0,0,0,0.1); padding: 1rem; border-radius: 4px; overflow: auto; font-size: 0.8rem;">${error.stack}</pre>` : ''}
                
                <h3 style="margin-top: 1.5rem;">Possible Solutions</h3>
                <ul>
                    <li>Verify the JavaScript file name matches what was generated</li>
                    <li>Check if the JavaScript file is in the correct location</li>
                    <li>Try rebuilding the application with <code>wasm-pack build --target web</code></li>
                    <li>If using Trunk, check that the output files are properly generated</li>
                </ul>
                
                <p style="margin-top: 1rem;">Check the browser console for more details (press F12).</p>
            </div>
        `;
    }
});
