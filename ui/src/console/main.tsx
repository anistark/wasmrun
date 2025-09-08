import { render } from 'preact'
import { Console } from './Console'
import '@/styles/globals.css'

render(<Console />, document.getElementById('root')!)

// Export for global access if needed
;(window as any).WasmRunConsole = { Console }
