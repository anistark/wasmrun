import { WasmModuleInfo } from '@/types'

interface ExportsCardProps {
  moduleInfo: WasmModuleInfo | null
}

export function ExportsCard({ moduleInfo }: ExportsCardProps) {
  return (
    <div class="bg-light-surface2 dark:bg-dark-surface2 rounded-xl p-6 border border-light-surface3 dark:border-dark-surface3 h-full">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center">
          <div class="w-3 h-3 bg-green-500 rounded-full mr-3"></div>
          <h3 class="text-lg font-semibold text-light-textPrimary dark:text-dark-textPrimary">
            Exports
          </h3>
        </div>
        {moduleInfo && (
          <span class="text-xs bg-light-surface3 dark:bg-dark-surface3 px-2 py-1 rounded-full text-light-textDim dark:text-dark-textDim">
            {moduleInfo.exports?.length || 0}
          </span>
        )}
      </div>
      
      <div class="max-h-48 overflow-y-auto">
        {moduleInfo?.exports && moduleInfo.exports.length > 0 ? (
          <div class="space-y-2">
            {moduleInfo.exports.map((exp, idx) => (
              <div
                key={idx}
                class="flex items-center p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg"
              >
                <span class="w-2 h-2 bg-green-400 rounded-full mr-3 flex-shrink-0"></span>
                <code class="font-mono text-sm text-light-textPrimary dark:text-dark-textPrimary">
                  {exp}
                </code>
              </div>
            ))}
          </div>
        ) : moduleInfo ? (
          <div class="text-center py-4">
            <span class="text-2xl mb-2 block">📭</span>
            <p class="text-light-textDim dark:text-dark-textDim text-sm">No exports found</p>
          </div>
        ) : (
          <div class="text-center py-4">
            <span class="text-2xl mb-2 block">⏳</span>
            <p class="text-light-textDim dark:text-dark-textDim text-sm">Loading exports...</p>
          </div>
        )}
      </div>
    </div>
  )
}