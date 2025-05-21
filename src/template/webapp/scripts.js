// Modified section to add debugging for CSS loading issues
async function initializeApp() {
    try {
        console.log("Chakra: Loading app from $JS_ENTRYPOINT$");
        
        // Debug CSS loading - log all stylesheets that are loaded in the document
        console.log("Current stylesheets loaded:", Array.from(document.styleSheets).map(sheet => {
            try {
                return sheet.href || 'inline stylesheet';
            } catch (e) {
                return 'Cross-origin stylesheet';
            }
        }));
        
        // Get references to elements
        const appElement = document.getElementById('app');
        const loadingElement = document.getElementById('chakra-loading');
        
        let module;
        try {
            // Try with absolute path first
            module = await import('/$JS_ENTRYPOINT$');
        } catch (error) {
            console.warn("Failed to load module with absolute path, trying relative path...");
            try {
                module = await import('./$JS_ENTRYPOINT$');
            } catch (secondError) {
                // Try one more time without the leading dot
                console.warn("Failed with relative path, trying plain import...");
                module = await import('$JS_ENTRYPOINT$');
            }
        }
        
        // Get the initialization function (different frameworks export differently)
        const init = module.default || module.init || module;
        
        if (typeof init !== 'function') {
            throw new Error(`Could not find initialization function in the module. Available exports: ${Object.keys(module).join(', ')}`);
        }
        
        console.log("Chakra: Initializing web application...");
        
        // Different initialization strategies
        let initialized = false;
        
        // Strategy 1: Try with target option
        if (!initialized) {
            try {
                await init({ target: appElement });
                console.log("Chakra: Application initialized with target option");
                initialized = true;
            } catch (error) {
                console.warn("Target initialization failed, trying alternative methods...", error);
            }
        }
        
        // Strategy 2: Try with root element instead of target (some frameworks use this)
        if (!initialized) {
            try {
                await init({ root: appElement });
                console.log("Chakra: Application initialized with root option");
                initialized = true;
            } catch (error) {
                console.warn("Root initialization failed, trying next method...", error);
            }
        }
        
        // Strategy 3: Try with DOM element directly
        if (!initialized) {
            try {
                await init(appElement);
                console.log("Chakra: Application initialized with direct element");
                initialized = true;
            } catch (error) {
                console.warn("Direct element initialization failed, trying next method...", error);
            }
        }
        
        // Strategy 4: Try with no arguments
        if (!initialized) {
            try {
                await init();
                console.log("Chakra: Application initialized without arguments");
                initialized = true;
            } catch (error) {
                console.warn("No-argument initialization failed", error);
                throw new Error("All initialization methods failed, application cannot be loaded");
            }
        }
        
        // Correct any container issues that might have occurred
        // Some frameworks might create their own containers with classes like 'app'
        setTimeout(() => {
            // Look for a div with class="app" that might have been created outside our container
            const appClassElements = document.querySelectorAll('.app');
            const appContainerExists = document.querySelector('#app');
            
            if (appClassElements.length > 0 && appContainerExists) {
                // Check if the .app element is not inside #app
                appClassElements.forEach(element => {
                    const isOutside = !appContainerExists.contains(element);
                    if (isOutside) {
                        console.log("Chakra: Found app element outside container, moving it inside");
                        // If the framework created a container outside our app div, move it inside
                        appContainerExists.innerHTML = '';
                        appContainerExists.appendChild(element);
                    }
                });
            }
            
            // Debug CSS loading - log all stylesheets again after initialization
            console.log("Stylesheets after app initialization:", Array.from(document.styleSheets).map(sheet => {
                try {
                    return sheet.href || 'inline stylesheet';
                } catch (e) {
                    return 'Cross-origin stylesheet';
                }
            }));
            
            // Check if there are links for CSS that might need to be loaded
            const cssLinks = document.querySelectorAll('link[rel="stylesheet"]');
            console.log(`Found ${cssLinks.length} CSS links in the document`);
            cssLinks.forEach(link => {
                console.log(`CSS link: ${link.href}`);
                
                // Force reload CSS that might not have been loaded correctly
                if (link.href) {
                    const originalHref = link.href;
                    link.href = '';
                    setTimeout(() => {
                        link.href = originalHref;
                    }, 10);
                }
            });
            
            // For frameworks that create style elements dynamically
            const styles = document.querySelectorAll('style');
            console.log(`Found ${styles.length} style elements in the document`);
        }, 100);
        
        // Remove loading indicator
        if (loadingElement) {
            // First fade it out with a CSS transition
            loadingElement.classList.add('hidden');
            // Then remove it completely after animation completes
            setTimeout(() => {
                if (loadingElement.parentNode) {
                    loadingElement.parentNode.removeChild(loadingElement);
                }
            }, 300);
        }
    } catch (error) {
        console.error("Failed to initialize application:", error);
        
        // Remove loading indicator
        const loadingElement = document.getElementById('chakra-loading');
        if (loadingElement && loadingElement.parentNode) {
            loadingElement.parentNode.removeChild(loadingElement);
        }
        
        // Show error message
        document.getElementById('app').innerHTML = `
            <div class="app-error">
                <h2>Application Failed to Load</h2>
                <p>There was an error initializing the Rust application:</p>
                <pre>${error.message}</pre>
                
                <h3>Debugging Information</h3>
                <p><strong>Module Path:</strong> $JS_ENTRYPOINT$</p>
                
                <h3>Possible Solutions</h3>
                <ul>
                    <li>Check the browser console for detailed error messages (F12)</li>
                    <li>Verify the JavaScript file was correctly generated</li>
                    <li>Make sure all required dependencies are available</li>
                    <li>For Rust wasm-bindgen projects, try rebuilding with <code>wasm-pack build --target web</code></li>
                </ul>
            </div>
        `;
    }
}

// Start the application
initializeApp();

// Simple live reload
let checkTimer;
function checkForReload() {
    fetch('/reload-check?t=' + Date.now(), { cache: 'no-store' })
        .then(response => {
            if (response.headers.get('X-Reload-Needed') === 'true') {
                console.log("Chakra: Change detected - reloading page");
                window.location.reload();
                return;
            }
            checkTimer = setTimeout(checkForReload, 1000);
        })
        .catch(() => {
            checkTimer = setTimeout(checkForReload, 2000);
        });
}

// Start reload checking
checkForReload();

// Handle visibility changes
document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
        checkForReload();
    } else {
        clearTimeout(checkTimer);
    }
});
