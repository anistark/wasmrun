import { useState, useEffect } from 'preact/hooks'
import { BaseLayout } from '@/layouts/BaseLayout'
import { StatusBar } from '@/components/StatusBar'
import { ThemeToggle } from '@/components/ThemeToggle'
import { StatusMessage } from '@/types'
import { loadWasmModule } from '@/utils/wasm'

// These will be replaced by the Rust template processor
declare const TITLE: string
declare const FILENAME: string

export function App() {
  const [status, setStatus] = useState<StatusMessage>({
    message: 'Loading WASM module...',
    type: 'info',
  })

  const [wasmError, setWasmError] = useState<string | null>(null)

  useEffect(() => {
    initializeWasm()
  }, [])

  async function initializeWasm() {
    try {
      setStatus({
        message: `Loading WASM module: ${FILENAME}`,
        type: 'info',
      })

      await loadWasmModule(FILENAME)

      setStatus({
        message: '✅ WASM Module loaded successfully!',
        type: 'success',
      })
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error'
      console.error('❌ Error loading WASM module:', error)

      setStatus({
        message: '❌ Error loading WASM module',
        type: 'error',
      })

      setWasmError(errorMessage)
    }
  }

  return (
    <BaseLayout title={TITLE}>
      <StatusBar status={status} />

      <div class="flex-1 relative overflow-hidden">
        {wasmError ? (
          <div class="p-8 m-8 border-2 border-light-error dark:border-dark-error rounded-lg">
            <h2 class="text-light-error dark:text-dark-error text-2xl font-semibold mb-4">
              Error Loading WASM Module
            </h2>
            <p class="mb-4">There was an error loading the WASM module:</p>
            <pre class="bg-light-bg dark:bg-dark-bg p-4 rounded overflow-auto border border-light-surface3 dark:border-dark-surface3 text-sm font-mono">
              {wasmError}
            </pre>
            <p class="mt-4 text-light-textDim dark:text-dark-textDim">
              Check the browser console for more details.
            </p>
          </div>
        ) : (
          <div id="wasm-app" class="w-full h-full overflow-auto">
            {/* WASM app content will be injected here */}
          </div>
        )}
      </div>
    </BaseLayout>
  )
}
