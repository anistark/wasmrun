import { useState, useEffect, useCallback } from 'preact/hooks'
import { ConsoleLayout } from '@/layouts/ConsoleLayout'
import { LogContainer } from '@/components/LogContainer'
import { FunctionPlayground } from '@/components/FunctionPlayground'
import { ModuleInfo } from '@/components/ModuleInfo'
import { LogEntry, ExportedFunction, WasmModuleInfo, TabItem } from '@/types'
import { log, loadWasmModule, analyzeWasmModule } from '@/utils/wasm'

// These will be replaced by the Rust template processor
declare const FILENAME: string

export function Console() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [moduleInfo, setModuleInfo] = useState<WasmModuleInfo | null>(null)
  const [exportedFunctions, setExportedFunctions] = useState<ExportedFunction[]>([])
  const [wasmInstance] = useState<WebAssembly.Instance | null>(null)
  const [activeTab, setActiveTab] = useState('console')

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

      addLog('✅ WASM module loaded successfully!', 'success')
      addLog(`Found ${functions.length} exported functions`, 'info')
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error'
      console.error('❌ Error loading WASM module:', error)

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

  const renderActiveTabContent = () => {
    switch (activeTab) {
      case 'console':
        return <LogContainer logs={logs} />
      case 'playground':
        return (
          <FunctionPlayground functions={exportedFunctions} onFunctionCall={handleFunctionCall} />
        )
      case 'info':
        return <ModuleInfo moduleInfo={moduleInfo} />
      default:
        return <LogContainer logs={logs} />
    }
  }

  return (
    <ConsoleLayout title="Wasmrun" filename={FILENAME} tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab}>
      <div class="flex-1">{renderActiveTabContent()}</div>
    </ConsoleLayout>
  )
}
