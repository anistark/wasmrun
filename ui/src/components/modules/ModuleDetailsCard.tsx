import { WasmModuleInfo } from '@/types'
import { formatBytes } from '@/utils/wasm'

interface ModuleDetailsCardProps {
  moduleInfo: WasmModuleInfo | null
}

export function ModuleDetailsCard({ moduleInfo }: ModuleDetailsCardProps) {
  const inspection = moduleInfo?.inspection

  return (
    <div class="bg-light-surface2 dark:bg-dark-surface2 rounded-xl p-6 border border-light-surface3 dark:border-dark-surface3">
      <div class="flex items-center mb-4">
        <div class="w-3 h-3 bg-blue-500 rounded-full mr-3"></div>
        <h3 class="text-lg font-semibold text-light-textPrimary dark:text-dark-textPrimary">
          Module Details
        </h3>
      </div>

      {moduleInfo ? (
        <div class="space-y-6">
          {/* Basic Details */}
          <div class="space-y-3">
            <div class="flex justify-between items-center">
              <span class="text-light-textDim dark:text-dark-textDim text-sm">Name</span>
              <span class="font-mono text-light-textPrimary dark:text-dark-textPrimary text-sm">
                {moduleInfo.name}
              </span>
            </div>
            <div class="flex justify-between items-center">
              <span class="text-light-textDim dark:text-dark-textDim text-sm">Size</span>
              <span class="font-mono text-light-textPrimary dark:text-dark-textPrimary text-sm">
                {formatBytes(moduleInfo.size)}
              </span>
            </div>
            <div class="flex justify-between items-center">
              <span class="text-light-textDim dark:text-dark-textDim text-sm">Type</span>
              <span
                class={`px-2 py-1 rounded-full text-xs font-medium ${
                  moduleInfo.isWasi
                    ? 'bg-green-100 dark:bg-green-900/20 text-green-700 dark:text-green-300'
                    : 'bg-blue-100 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300'
                }`}
              >
                {moduleInfo.isWasi ? 'WASI Module' : 'WebAssembly'}
              </span>
            </div>
          </div>

          {/* Binary Analysis Section */}
          {inspection && (
            <div class="border-t border-light-surface3 dark:border-dark-surface3 pt-4">
              <div class="flex items-center mb-3">
                <div class="w-2 h-2 bg-pink-500 rounded-full mr-2"></div>
                <h4 class="text-sm font-semibold text-light-textPrimary dark:text-dark-textPrimary">
                  Binary Analysis
                </h4>
              </div>

              <div class="grid grid-cols-3 gap-3 mb-4 text-center">
                <div class="p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
                  <div class="text-sm font-bold text-light-textPrimary dark:text-dark-textPrimary">
                    {inspection.section_count}
                  </div>
                  <div class="text-xs text-light-textDim dark:text-dark-textDim">Sections</div>
                </div>
                <div class="p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
                  <div class="text-sm font-bold text-light-textPrimary dark:text-dark-textPrimary">
                    v1
                  </div>
                  <div class="text-xs text-light-textDim dark:text-dark-textDim">WASM</div>
                </div>
                <div class="p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
                  <span
                    class={`text-xs px-2 py-1 rounded-full font-medium ${
                      inspection.valid_magic
                        ? 'bg-green-100 dark:bg-green-900/20 text-green-700 dark:text-green-300'
                        : 'bg-red-100 dark:bg-red-900/20 text-red-700 dark:text-red-300'
                    }`}
                  >
                    {inspection.valid_magic ? 'Valid' : 'Invalid'}
                  </span>
                  <div class="text-xs text-light-textDim dark:text-dark-textDim mt-1">Status</div>
                </div>
              </div>

              <div class="space-y-2 mb-3">
                <div class="flex items-center justify-between text-sm">
                  <span class="text-light-textDim dark:text-dark-textDim">File Size</span>
                  <span class="font-mono text-xs bg-light-surface3 dark:bg-dark-surface3 px-2 py-1 rounded">
                    {formatBytes(inspection.file_size)}
                  </span>
                </div>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-light-textDim dark:text-dark-textDim">Functions</span>
                  <span class="font-mono text-xs bg-light-surface3 dark:bg-dark-surface3 px-2 py-1 rounded">
                    {inspection.function_count}
                  </span>
                </div>
              </div>

              {inspection.sections && inspection.sections.length > 0 && (
                <div>
                  <h5 class="text-xs font-medium text-light-textPrimary dark:text-dark-textPrimary mb-2">
                    Key Sections
                  </h5>
                  <div class="max-h-24 overflow-y-auto space-y-1">
                    {inspection.sections.slice(0, 4).map((section, idx) => (
                      <div
                        key={idx}
                        class="flex items-center justify-between text-xs p-2 bg-light-surface3 dark:bg-dark-surface3 rounded"
                      >
                        <span class="font-medium text-light-textPrimary dark:text-dark-textPrimary">
                          {section.name}
                        </span>
                        <span class="text-light-textDim dark:text-dark-textDim">
                          {formatBytes(section.size)}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {inspection.has_memory_section && (
                <div class="mt-3 p-2 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                  <div class="text-xs text-blue-700 dark:text-blue-300 font-medium mb-1">
                    💾 WASI Memory Detected
                  </div>
                  {inspection.memory_limits && (
                    <div class="text-xs text-blue-600 dark:text-blue-400">
                      Initial: {inspection.memory_limits[0]} pages
                      {inspection.memory_limits[1] && `, Max: ${inspection.memory_limits[1]} pages`}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      ) : (
        <div class="text-center py-8">
          <div class="w-12 h-12 mx-auto mb-3 rounded-full bg-light-surface3 dark:bg-dark-surface3 flex items-center justify-center">
            <span class="text-2xl">⏳</span>
          </div>
          <p class="text-light-textDim dark:text-dark-textDim text-sm">
            Module analysis in progress...
          </p>
        </div>
      )}
    </div>
  )
}
