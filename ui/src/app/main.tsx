import { render } from 'preact'
import { App } from './App'
import '@/styles/globals.css'

render(<App />, document.getElementById('root')!)

// Export for global access if needed
;(window as any).WasmRunApp = { App }
