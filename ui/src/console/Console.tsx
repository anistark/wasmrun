import { useState, useEffect, useCallback } from 'preact/hooks'
import { ConsoleLayout } from '@/layouts/ConsoleLayout'
import { LogContainer } from '@/components/LogContainer'
import { FunctionPlayground } from '@/components/FunctionPlayground'
import { ModuleInfo } from '@/components/ModuleInfo'
import { LogEntry, ExportedFunction, WasmModuleInfo, TabItem } from '@/types'
import { log, loadWasmModule, analyzeWasmModule, fetchModuleInspection } from '@/utils/wasm'

// These will be replaced by the Rust template processor
declare const FILENAME: string

export function Console() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [moduleInfo, setModuleInfo] = useState<WasmModuleInfo | null>(null)
  const [exportedFunctions, setExportedFunctions] = useState<ExportedFunction[]>([])
  const [wasmInstance, setWasmInstance] = useState<WebAssembly.Instance | null>(null)
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

      // Instantiate the WASM module to create a runnable instance
      // For wasm-bindgen modules, we need to provide the proper imports
      let instance: WebAssembly.Instance

      try {
        // Try to instantiate without imports first
        instance = new WebAssembly.Instance(module, {})
      } catch {
        // If that fails, try with basic imports for wasm-bindgen
        const imports = {
          wbg: {
            __wbg_log_8b68cfc62b396cc3: (arg0: number, arg1: number) => {
              // This would extract string from memory - simplified for now
              console.log(`WASM log: ${arg0}, ${arg1}`)
            },
            __wbindgen_init_externref_table: () => {
              // Initialize external reference table
            },
          },
        }
        instance = new WebAssembly.Instance(module, imports)
      }

      setWasmInstance(instance)

      // Fetch real inspection data from backend
      addLog('Analyzing WASM module structure...')
      const inspection = await fetchModuleInspection()

      // Create module info with real inspection data
      const moduleInfo: WasmModuleInfo = {
        name: FILENAME,
        size: inspection?.file_size || 0,
        imports: analysis.imports || [],
        exports: analysis.exports || [],
        isWasi: analysis.isWasi || false,
        inspection,
      }

      setModuleInfo(moduleInfo)

      if (inspection) {
        addLog(
          `Module analysis complete: ${inspection.section_count} sections, ${inspection.function_count} functions`,
          'success'
        )
      } else {
        addLog('Module analysis failed - using basic info only', 'warning')
      }

      // Extract callable functions with proper parameter definitions
      const functions: ExportedFunction[] = (analysis.exports || [])
        .filter(name => name !== 'memory' && !name.startsWith('__'))
        .map(name => {
          // Define parameters for known functions
          let parameters: any[] = []
          let signature = `${name}() -> unknown`

          switch (name) {
            case 'greet':
              parameters = [{ name: 'name', type: 'string', value: 'World' }]
              signature = 'greet(name: string) -> void'
              break
            case 'fibonacci':
              parameters = [{ name: 'n', type: 'i32', value: '10' }]
              signature = 'fibonacci(n: u32) -> u32'
              break
            case 'sum_array':
              parameters = [{ name: 'numbers', type: 'array', value: '[1, 2, 3, 4, 5]' }]
              signature = 'sum_array(numbers: i32[]) -> i32'
              break
            default:
              // For unknown functions, try to guess if they might have parameters
              parameters = []
              break
          }

          return {
            name,
            signature,
            parameters,
            description: `Exported function: ${name}`,
          }
        })

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
    <ConsoleLayout filename={FILENAME} tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab}>
      <div class="flex-1">{renderActiveTabContent()}</div>
    </ConsoleLayout>
  )
}
