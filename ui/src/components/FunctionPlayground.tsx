import { useState } from 'preact/hooks'
import { ExportedFunction } from '@/types'
import clsx from 'clsx'

interface FunctionPlaygroundProps {
  functions: ExportedFunction[]
  onFunctionCall?: (functionName: string, args: any[]) => Promise<any>
}

export function FunctionPlayground({ functions, onFunctionCall }: FunctionPlaygroundProps) {
  const [results, setResults] = useState<Record<string, any>>({})
  const [loading, setLoading] = useState<Record<string, boolean>>({})

  const handleFunctionCall = async (func: ExportedFunction) => {
    if (!onFunctionCall) return

    const functionName = func.name
    setLoading(prev => ({ ...prev, [functionName]: true }))

    try {
      // Get parameter values from form inputs
      const args = func.parameters.map(param => {
        const input = document.querySelector(
          `input[data-function="${functionName}"][data-param="${param.name}"]`
        ) as HTMLInputElement
        const value = input?.value || ''

        // Convert based on type
        switch (param.type) {
          case 'i32':
          case 'i64':
            return parseInt(value) || 0
          case 'f32':
          case 'f64':
            return parseFloat(value) || 0.0
          default:
            return value
        }
      })

      const result = await onFunctionCall(functionName, args)
      setResults(prev => ({ ...prev, [functionName]: result }))
    } catch (error) {
      setResults(prev => ({
        ...prev,
        [functionName]: `Error: ${error instanceof Error ? error.message : 'Unknown error'}`,
      }))
    } finally {
      setLoading(prev => ({ ...prev, [functionName]: false }))
    }
  }

  if (functions.length === 0) {
    return (
      <div class="text-center py-8 bg-light-surface2 dark:bg-dark-surface2 rounded-lg border border-light-surface3 dark:border-dark-surface3">
        <h4 class="text-lg font-medium text-light-textMuted dark:text-dark-textMuted mb-2">No exported functions found</h4>
        <p class="text-light-textDim dark:text-dark-textDim">
          This WASM module doesn't export any callable functions, or the module hasn't loaded yet.
        </p>
      </div>
    )
  }

  return (
    <div class="space-y-4">
      <div class="mb-6">
        <h3 class="text-xl font-medium text-light-textMuted dark:text-dark-textMuted mb-2">Function Playground</h3>
        <p class="text-sm text-light-textDim dark:text-dark-textDim">Interact with the WASM module's exported functions</p>
      </div>

      {functions.map(func => (
        <div key={func.name} class="bg-light-surface2 dark:bg-dark-surface2 rounded-lg p-4 border border-light-surface3 dark:border-dark-surface3">
          <div class="mb-4 pb-2 border-b border-light-surface3 dark:border-dark-surface3">
            <h4 class="text-lg font-semibold text-light-accent2 dark:text-purple-400 font-mono mb-1">{func.name}</h4>
            <div class="bg-light-bg dark:bg-dark-bg px-2 py-1 rounded text-sm font-mono text-light-textDim dark:text-dark-textDim mb-2">
              {func.signature}
            </div>
            {func.description && <p class="text-sm text-light-textMuted dark:text-dark-textMuted">{func.description}</p>}
          </div>

          <div class="space-y-3">
            {func.parameters.map(param => (
              <div key={param.name} class="space-y-1">
                <label class="text-sm font-medium text-light-warning dark:text-orange-300 font-mono">
                  {param.name}
                  <span class="text-xs text-light-textDim dark:text-dark-textDim italic ml-2">({param.type})</span>
                </label>
                <input
                  type="text"
                  data-function={func.name}
                  data-param={param.name}
                  defaultValue={param.value || ''}
                  class="w-full px-3 py-2 bg-light-bg dark:bg-dark-bg text-light-textMuted dark:text-dark-textMuted border border-light-surface3 dark:border-dark-surface3 rounded focus:outline-none focus:border-light-accent dark:focus:border-dark-accent font-mono text-sm"
                  placeholder={`Enter ${param.type} value`}
                />
              </div>
            ))}

            <div class="flex items-center gap-4 pt-3 border-t border-light-surface3 dark:border-dark-surface3">
              <button
                onClick={() => handleFunctionCall(func)}
                disabled={loading[func.name]}
                class={clsx(
                  'px-4 py-2 bg-light-accent2 dark:bg-dark-accent2 text-white rounded font-semibold transition-colors',
                  {
                    'hover:bg-opacity-90': !loading[func.name],
                    'opacity-50 cursor-not-allowed': loading[func.name],
                  }
                )}
              >
                {loading[func.name] ? 'Calling...' : 'Call Function'}
              </button>

              <div class="flex-1 px-3 py-2 bg-light-bg dark:bg-dark-bg border border-light-surface3 dark:border-dark-surface3 rounded font-mono text-sm min-h-8 flex items-center">
                {results[func.name] !== undefined ? (
                  <span
                    class={clsx(
                      typeof results[func.name] === 'string' &&
                        results[func.name].startsWith('Error:')
                        ? 'text-light-error dark:text-dark-error'
                        : 'text-light-success dark:text-dark-success'
                    )}
                  >
                    {String(results[func.name])}
                  </span>
                ) : (
                  <span class="text-light-textDim dark:text-dark-textDim italic">No result yet</span>
                )}
              </div>
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}
