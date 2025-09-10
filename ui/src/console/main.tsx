import { render } from 'preact'
import { Console } from './Console'
import { ThemeProvider } from '@/contexts/ThemeContext'
import '@/styles/globals.css'

render(
  <ThemeProvider>
    <Console />
  </ThemeProvider>,
  document.getElementById('root')!
)

// Export for global access if needed
;(window as any).WasmRunConsole = { Console }
