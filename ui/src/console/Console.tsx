import { useState, useEffect, useCallback } from 'preact/hooks'
import { ConsoleLayout } from '@/layouts/ConsoleLayout'
import { StatusBar } from '@/components/StatusBar'
import { LogContainer } from '@/components/LogContainer'
import { FunctionPlayground } from '@/components/FunctionPlayground'
import { ModuleInfo } from '@/components/ModuleInfo'
import { Tabs } from '@/components/Tabs'
import { StatusMessage, LogEntry, ExportedFunction, WasmModuleInfo, TabItem } from '@/types'
import { log, loadWasmModule, analyzeWasmModule } from '@/utils/wasm'

// These will be replaced by the Rust template processor
declare const FILENAME: string

export function Console() {
  const [status, setStatus] = useState<StatusMessage>({
    message: '⏳ Loading WASM module...',
    type: 'info',
  })

  const [logs, setLogs] = useState<LogEntry[]>([])
  const [moduleInfo, setModuleInfo] = useState<WasmModuleInfo | null>(null)
  const [exportedFunctions, setExportedFunctions] = useState<ExportedFunction[]>([])
  const [wasmInstance] = useState<WebAssembly.Instance | null>(null)

  const addLog = useCallback((message: string, type: LogEntry['type'] = 'info') => {
    const logEntry = log(message, type)
    setLogs(prev => [...prev, logEntry])
  }, [])

  const initializeWasm = useCallback(async () => {
    try {
      addLog(`Loading WASM module: ${FILENAME}`)

      const module = await loadWasmModule(FILENAME)
      const analysis = analyzeWasmModule(module)

      // Create basic module info
      const moduleInfo: WasmModuleInfo = {
        name: FILENAME,
        size: 0, // Will be updated when we get the actual bytes
        imports: analysis.imports || [],
        exports: analysis.exports || [],
        isWasi: analysis.isWasi || false,
      }

      setModuleInfo(moduleInfo)

      // Extract callable functions
      const functions: ExportedFunction[] = (analysis.exports || [])
        .filter(name => name !== 'memory' && !name.startsWith('__'))
        .map(name => ({
          name,
          signature: `${name}() -> unknown`,
          parameters: [], // TODO: Extract actual parameters if available
          description: `Exported function: ${name}`,
        }))

      setExportedFunctions(functions)

      setStatus({
        message: '✅ WASM Module loaded successfully!',
        type: 'success',
      })

      addLog('✅ WASM module loaded successfully!', 'success')
      addLog(`Found ${functions.length} exported functions`, 'info')
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error'
      console.error('❌ Error loading WASM module:', error)

      setStatus({
        message: '❌ Error loading WASM module',
        type: 'error',
      })

      addLog(`❌ Error loading WASM module: ${errorMessage}`, 'error')
    }
  }, [addLog])

  useEffect(() => {
    initializeWasm()
  }, [initializeWasm])

  const handleFunctionCall = async (functionName: string, args: any[]) => {
    if (!wasmInstance) {
      throw new Error('WASM module not loaded')
    }

    addLog(`Calling function: ${functionName}(${args.join(', ')})`, 'info')

    try {
      const func = (wasmInstance.exports as any)[functionName]
      if (typeof func !== 'function') {
        throw new Error(`Function ${functionName} not found in exports`)
      }

      const result = func(...args)
      addLog(`✅ Function returned: ${result}`, 'success')
      return result
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error'
      addLog(`❌ Function call failed: ${errorMessage}`, 'error')
      throw error
    }
  }

  const tabs: TabItem[] = [
    {
      id: 'console',
      label: 'Console',
      content: <LogContainer logs={logs} />,
    },
    {
      id: 'playground',
      label: 'Playground',
      content: (
        <FunctionPlayground functions={exportedFunctions} onFunctionCall={handleFunctionCall} />
      ),
    },
    {
      id: 'info',
      label: 'Module Info',
      content: <ModuleInfo moduleInfo={moduleInfo} />,
    },
  ]

  return (
    <ConsoleLayout title="Wasmrun">
      <div class="px-8 py-4">
        <h2 class="text-2xl font-medium text-dark-textMuted mb-4">Running: {FILENAME}</h2>
        <StatusBar status={status} />
      </div>

      <div class="flex-1 px-8 pb-8">
        <Tabs tabs={tabs} defaultTab="console" />
      </div>
    </ConsoleLayout>
  )
}
