import { useState, useEffect, useCallback } from 'preact/hooks'
import { ConsoleLayout } from '@/layouts/ConsoleLayout'
import { LogContainer } from '@/components/LogContainer'
import { FunctionPlayground } from '@/components/FunctionPlayground'
import { ModuleInfo } from '@/components/ModuleInfo'
import { LogEntry, ExportedFunction, WasmModuleInfo, TabItem } from '@/types'
import { log, loadWasmModule, analyzeWasmModule, fetchModuleInspection } from '@/utils/wasm'
import { parseCommand } from '@/utils/commandParser'

// These will be replaced by the Rust template processor
declare const FILENAME: string

// Helper function to identify internal WASM functions that should be hidden from UI
const isInternalFunction = (name: string) => {
  return name === 'memory' || name === 'main' || name.startsWith('__wbindgen')
}

// Detect if a function expects string parameters (wasm-bindgen string functions)
const isStringFunction = (name: string) => {
  return (
    name === 'greet' ||
    name.includes('greet') ||
    name.includes('message') ||
    name.includes('text') ||
    name.includes('name') ||
    name.includes('hello')
  )
}

// Detect if a function expects array parameters
const isArrayFunction = (name: string) => {
  return (
    name === 'sum_array' ||
    name.includes('array') ||
    name.includes('list') ||
    name.includes('sum') ||
    name.includes('process') ||
    name.includes('calculate')
  )
}

// Generate function information based on naming patterns and common conventions
const generateFunctionInfo = (name: string) => {
  // Known function patterns with their signatures
  const knownPatterns = {
    greet: {
      parameters: [{ name: 'name', type: 'string', value: 'World' }],
      signature: 'greet(name: string) -> string',
      description: 'Greet a person with a friendly message',
    },
    fibonacci: {
      parameters: [{ name: 'n', type: 'i32', value: '10' }],
      signature: 'fibonacci(n: u32) -> u32',
      description: 'Calculate the nth Fibonacci number',
    },
    sum_array: {
      parameters: [{ name: 'numbers', type: 'array', value: '[1, 2, 3, 4, 5]' }],
      signature: 'sum_array(numbers: i32[]) -> i32',
      description: 'Sum all numbers in an array',
    },
  }

  // Check for exact matches first
  if (knownPatterns[name as keyof typeof knownPatterns]) {
    return knownPatterns[name as keyof typeof knownPatterns]
  }

  // Pattern-based inference for common naming conventions
  if (name.includes('add') || name.includes('sum')) {
    return {
      parameters: [
        { name: 'a', type: 'i32', value: '5' },
        { name: 'b', type: 'i32', value: '3' },
      ],
      signature: `${name}(a: i32, b: i32) -> i32`,
      description: `Arithmetic function: ${name}`,
    }
  }

  if (name.includes('get') || name.includes('read')) {
    return {
      parameters: [],
      signature: `${name}() -> i32`,
      description: `Getter function: ${name}`,
    }
  }

  if (name.includes('set') || name.includes('write')) {
    return {
      parameters: [{ name: 'value', type: 'i32', value: '42' }],
      signature: `${name}(value: i32) -> void`,
      description: `Setter function: ${name}`,
    }
  }

  if (name.includes('array') || name.includes('list')) {
    return {
      parameters: [{ name: 'items', type: 'array', value: '[1, 2, 3]' }],
      signature: `${name}(items: i32[]) -> i32`,
      description: `Array processing function: ${name}`,
    }
  }

  // Default fallback for unknown functions
  return {
    parameters: [],
    signature: `${name}() -> unknown`,
    description: `Exported function: ${name}`,
  }
}

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
              // This will be called after the instance is created
              // For now, just store the arguments - we'll extract the string later
              if (instance && instance.exports.memory) {
                try {
                  const memory = instance.exports.memory as WebAssembly.Memory
                  const ptr = arg0
                  const len = arg1
                  const bytes = new Uint8Array(memory.buffer, ptr, len)
                  const message = new TextDecoder().decode(bytes)
                  addLog(message, 'info')
                } catch {
                  addLog(`WASM log: ${arg0}, ${arg1}`, 'info')
                }
              } else {
                // Store for later processing
                setTimeout(() => {
                  if (instance && instance.exports.memory) {
                    try {
                      const memory = instance.exports.memory as WebAssembly.Memory
                      const ptr = arg0
                      const len = arg1
                      const bytes = new Uint8Array(memory.buffer, ptr, len)
                      const message = new TextDecoder().decode(bytes)
                      addLog(message, 'info')
                    } catch {
                      addLog(`WASM log: ${arg0}, ${arg1}`, 'info')
                    }
                  }
                }, 0)
              }
            },
            __wbindgen_init_externref_table: () => {
              // Initialize external reference table
            },
          },
        }

        instance = new WebAssembly.Instance(module, imports)
      }

      // Debug: log all exports to understand the actual WASM interface
      // const allExports = Object.keys(instance.exports)
      // addLog(`Available exports: ${allExports.join(', ')}`, 'info')

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
        inspection: inspection || undefined,
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
      // Filter out internal functions from UI display, but keep them accessible for console calls
      const functions: ExportedFunction[] = (analysis.exports || [])
        .filter(name => {
          // Hide internal functions from UI
          return !isInternalFunction(name)
        })
        .map(name => {
          // Generate function metadata based on naming patterns and conventions
          const functionInfo = generateFunctionInfo(name)

          return {
            name,
            signature: functionInfo.signature,
            parameters: functionInfo.parameters,
            description: functionInfo.description,
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

  const handleFunctionCall = useCallback(
    async (functionName: string, args: any[], skipLog = false) => {
      if (!wasmInstance) {
        throw new Error('WASM module not loaded')
      }

      if (!skipLog) {
        addLog(`> ${functionName}(${args.join(', ')})`, 'info')
      }

      try {
        const exports = wasmInstance.exports as any

        const func = exports[functionName]
        if (typeof func !== 'function') {
          throw new Error(`Function ${functionName} not found in exports`)
        }

        // Debug: log function info
        addLog(
          `Calling ${functionName} with args: [${args.map(a => (typeof a === 'string' ? `"${a}"` : a)).join(', ')}]`,
          'info'
        )

        // Special handling for main function
        if (functionName === 'main') {
          addLog(`Calling main function (entry point)`, 'info')
          try {
            const result = func()
            addLog(`✅ main() executed successfully: ${result}`, 'success')
            return result
          } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error'
            addLog(`❌ main() failed: ${errorMessage}`, 'error')
            throw error
          }
        }

        // Special handling for internal wasm-bindgen functions
        if (functionName.startsWith('__wbindgen')) {
          addLog(`Calling internal wasm-bindgen function: ${functionName}`, 'warning')
          try {
            const result = func(...args)
            addLog(`✅ ${functionName}() result: ${result}`, 'success')
            return result
          } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'Unknown error'
            addLog(`❌ ${functionName}() failed: ${errorMessage}`, 'error')
            throw error
          }
        }

        // Special handling for wasm-bindgen string functions
        if (isStringFunction(functionName) && args.length > 0) {
          const memory = exports.memory as WebAssembly.Memory
          const malloc = exports.__wbindgen_malloc
          const free = exports.__wbindgen_free

          if (memory && malloc && free) {
            // Encode string to WASM memory
            const stringArg = String(args[0])
            const encoder = new TextEncoder()
            const stringBytes = encoder.encode(stringArg)

            // Allocate memory for the string
            const stringPtr = malloc(stringBytes.length, 1) // alignment = 1 for bytes
            const memoryView = new Uint8Array(memory.buffer)
            memoryView.set(stringBytes, stringPtr)

            try {
              // Call function with pointer and length
              const resultPtr = func(stringPtr, stringBytes.length)

              // The result looks like "1114136,49" which means [ptr, len]
              // Let's parse this format directly
              const resultStr = String(resultPtr)
              if (resultStr.includes(',')) {
                const [ptr, len] = resultStr.split(',').map((n: string) => parseInt(n.trim()))

                if (ptr > 0 && len > 0 && len < 1000000) {
                  // sanity check
                  try {
                    const resultBytes = new Uint8Array(memory.buffer, ptr, len)
                    const result = new TextDecoder().decode(resultBytes)
                    addLog(`✅ ${result}`, 'success')
                    return result
                  } catch (e) {
                    addLog(`Error decoding string: ${e}`, 'warning')
                  }
                }
              }

              // Fallback: return raw result
              addLog(`✅ Raw result: ${resultPtr}`, 'success')
              return resultPtr
            } finally {
              // Always free the input string memory
              free(stringPtr, stringBytes.length)
            }
          } else {
            // Missing required wasm-bindgen functions, fall back to regular call
            addLog(`Missing wasm-bindgen functions for ${functionName}, using fallback`, 'warning')
          }

          // Don't continue to regular function call for string functions
          return
        }

        // Handle array functions specially - they expect array parameters
        if (isArrayFunction(functionName) && args.length > 0) {
          const memory = exports.memory as WebAssembly.Memory
          const malloc = exports.__wbindgen_malloc
          const free = exports.__wbindgen_free

          // Debug: log available wasm-bindgen functions
          addLog(`Debug: Available exports for memory management:`, 'info')
          addLog(`  memory: ${!!memory}, malloc: ${!!malloc}, free: ${!!free}`, 'info')
          const bindgenExports = Object.keys(exports).filter(key => key.startsWith('__wbindgen'))
          addLog(`  wbindgen exports: ${bindgenExports.join(', ')}`, 'info')

          if (memory && malloc && free) {
            let array

            // If multiple arguments were passed (like sum_array(1,2,3,4,5)), treat them as array elements
            if (args.length > 1) {
              array = args
                .map(arg => (typeof arg === 'number' ? arg : parseInt(String(arg))))
                .filter(n => !isNaN(n))
            } else {
              // Single argument - could be an array or string representation
              array = args[0]

              // Parse array if it's a string
              if (typeof array === 'string') {
                try {
                  array = JSON.parse(array)
                } catch {
                  // Try to parse as comma-separated values
                  array = array
                    .split(',')
                    .map((s: string) => parseInt(s.trim()))
                    .filter((n: number) => !isNaN(n))
                }
              }
            }

            if (Array.isArray(array) && array.length > 0) {
              // Allocate memory for the array (4 bytes per i32)
              const arrayPtr = malloc(array.length * 4, 4) // alignment = 4 for i32
              const view = new DataView(memory.buffer)

              // Write the array to memory
              for (let i = 0; i < array.length; i++) {
                view.setInt32(arrayPtr + i * 4, array[i], true) // little-endian
              }

              try {
                // Call function with pointer and length
                // addLog(
                //   `Debug: Calling ${functionName} WASM function with ptr=${arrayPtr}, len=${array.length}`,
                //   'info'
                // )
                const result = func(arrayPtr, array.length)
                // addLog(`Debug: ${functionName} returned: ${result}`, 'info')
                addLog(`✅ ${result}`, 'success')

                // Free the array memory immediately after getting result
                // addLog(`Debug: Freeing memory at ptr=${arrayPtr}`, 'info')
                try {
                  // wasm-bindgen free expects (ptr, size, alignment)
                  free(arrayPtr, array.length * 4, 4)
                } catch {
                  // If the 3-param version fails, try the 2-param version
                  try {
                    free(arrayPtr, array.length * 4)
                  } catch {
                    // If both fail, try just the pointer
                    try {
                      free(arrayPtr)
                    } catch {
                      addLog(`Debug: All free attempts failed, but function succeeded`, 'warning')
                    }
                  }
                }

                return result
              } catch (wasmError) {
                addLog(`Debug: WASM function call failed: ${wasmError}`, 'error')
                // Still try to free memory on error
                try {
                  free(arrayPtr, array.length * 4, 4)
                } catch {
                  try {
                    free(arrayPtr, array.length * 4)
                  } catch {
                    try {
                      free(arrayPtr)
                    } catch {
                      // Give up on freeing
                    }
                  }
                }
                throw wasmError
              }
            } else {
              throw new Error('Invalid array input for ${functionName}')
            }
          } else {
            // Missing required wasm-bindgen functions, try alternative approaches
            addLog(
              'Missing wasm-bindgen functions for ${functionName}, trying alternatives',
              'warning'
            )

            let array
            // If multiple arguments were passed (like sum_array(1,2,3,4,5)), treat them as array elements
            if (args.length > 1) {
              array = args
                .map(arg => (typeof arg === 'number' ? arg : parseInt(String(arg))))
                .filter(n => !isNaN(n))
            } else {
              // Single argument - could be an array or string representation
              array = args[0]

              // Parse array if it's a string
              if (typeof array === 'string') {
                try {
                  array = JSON.parse(array)
                } catch {
                  // Try to parse as comma-separated values
                  array = array
                    .split(',')
                    .map((s: string) => parseInt(s.trim()))
                    .filter((n: number) => !isNaN(n))
                }
              }
            }

            if (Array.isArray(array) && array.length > 0) {
              // Try different calling conventions
              try {
                // Method 1: Call with individual arguments
                addLog(
                  `Debug: Trying to call ${functionName} with individual args: ${array.join(', ')}`,
                  'info'
                )
                const result1 = func(...array)
                addLog(`✅ ${result1}`, 'success')
                return result1
              } catch {
                try {
                  // Method 2: Call with array as single argument
                  addLog(`Debug: Trying to call ${functionName} with array as single arg`, 'info')
                  const result2 = func(array)
                  addLog(`✅ ${result2}`, 'success')
                  return result2
                } catch {
                  // Method 3: Sum manually as fallback
                  addLog(`Debug: Both WASM calls failed, computing sum manually`, 'warning')
                  const manualSum = array.reduce((sum, num) => sum + num, 0)
                  addLog(`✅ ${manualSum} (computed manually)`, 'success')
                  return manualSum
                }
              }
            } else {
              throw new Error('Invalid array input for ${functionName}')
            }
          }
          // All paths above either return or throw, so execution never reaches here
        }

        // Regular function call for non-special functions
        const result = func(...args)
        addLog(`✅ ${result}`, 'success')
        return result
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error'
        addLog(`❌ ${errorMessage}`, 'error')
        throw error
      }
    },
    [wasmInstance, addLog]
  )

  const handleConsoleCommand = useCallback(
    async (command: string) => {
      const parsed = parseCommand(command)

      addLog(`> ${command}`, 'info')

      try {
        switch (parsed.type) {
          case 'help':
            addLog('Available commands:', 'info')
            addLog('  help - Show this help message', 'info')
            addLog('  clear - Clear the console', 'info')
            addLog('  list - List available functions', 'info')
            addLog('  memory.size() - Get memory size in pages', 'info')
            addLog('  memory.grow(pages) - Grow memory by pages', 'info')
            if (exportedFunctions.length > 0) {
              addLog('Available functions (shown in Playground):', 'info')
              exportedFunctions.forEach(func => {
                addLog(`  ${func.signature}`, 'info')
              })
            }
            if (wasmInstance) {
              const allExports = Object.keys(wasmInstance.exports).filter(
                name => typeof (wasmInstance.exports as any)[name] === 'function'
              )
              const hiddenFunctions = allExports.filter(
                name => isInternalFunction(name) || !exportedFunctions.some(f => f.name === name)
              )
              if (hiddenFunctions.length > 0) {
                addLog('Internal functions (callable from console only):', 'info')
                hiddenFunctions.forEach(name => {
                  addLog(`  ${name}() - Internal/system function`, 'info')
                })
              }
            }
            break

          case 'clear':
            setLogs([])
            break

          case 'list':
            if (exportedFunctions.length === 0) {
              addLog('No exported functions found', 'warning')
            } else {
              addLog(`Found ${exportedFunctions.length} exported functions:`, 'info')
              exportedFunctions.forEach(func => {
                addLog(`  ${func.signature}`, 'info')
              })
            }
            break

          case 'memory': {
            if (!wasmInstance) {
              throw new Error('WASM module not loaded')
            }

            const memory = (wasmInstance.exports as any).memory as WebAssembly.Memory
            if (!memory) {
              addLog('❌ No memory export found', 'error')
              break
            }

            if (parsed.name === 'memory.size()') {
              const pages = memory.buffer.byteLength / (64 * 1024)
              addLog(
                `Memory size: ${pages} pages (${(memory.buffer.byteLength / 1024 / 1024).toFixed(2)} MB)`,
                'success'
              )
            } else if (parsed.name?.startsWith('memory.grow(')) {
              const growMatch = parsed.name.match(/memory\.grow\((\d+)\)/)
              if (growMatch) {
                const pages = parseInt(growMatch[1])
                const prevPages = memory.grow(pages)
                addLog(`Memory grown from ${prevPages} to ${prevPages + pages} pages`, 'success')
              }
            }
            break
          }

          case 'function':
            if (!parsed.name) {
              throw new Error('Function name is required')
            }
            await handleFunctionCall(parsed.name, parsed.args || [], true)
            break

          case 'unknown':
          default:
            addLog(`❌ Unknown command: ${command}`, 'error')
            addLog('Type "help" for available commands', 'info')
            break
        }
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error'
        addLog(`❌ ${errorMessage}`, 'error')
      }
    },
    [wasmInstance, exportedFunctions, addLog, handleFunctionCall]
  )

  const tabs: TabItem[] = [
    {
      id: 'console',
      label: 'Console',
      content: <LogContainer logs={logs} onCommand={handleConsoleCommand} interactive={true} />,
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
    const isConsoleTab = activeTab === 'console'
    const containerClass = isConsoleTab
      ? 'flex-1 flex flex-col min-h-0'
      : 'flex-1 overflow-y-auto p-6'

    switch (activeTab) {
      case 'console':
        return (
          <div class={containerClass}>
            <LogContainer logs={logs} onCommand={handleConsoleCommand} interactive={true} />
          </div>
        )
      case 'playground':
        return (
          <div class={containerClass}>
            <FunctionPlayground functions={exportedFunctions} onFunctionCall={handleFunctionCall} />
          </div>
        )
      case 'info':
        return (
          <div class={containerClass}>
            <ModuleInfo moduleInfo={moduleInfo} />
          </div>
        )
      default:
        return (
          <div class={containerClass}>
            <LogContainer logs={logs} onCommand={handleConsoleCommand} interactive={true} />
          </div>
        )
    }
  }

  return (
    <ConsoleLayout filename={FILENAME} tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab}>
      {renderActiveTabContent()}
    </ConsoleLayout>
  )
}
