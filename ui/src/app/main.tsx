import { render } from 'preact'
import { App } from './App'
import { ThemeProvider } from '@/contexts/ThemeContext'
import '@/styles/globals.css'

render(
  <ThemeProvider>
    <App />
  </ThemeProvider>, 
  document.getElementById('root')!
)

// Export for global access if needed
;(window as any).WasmRunApp = { App }
