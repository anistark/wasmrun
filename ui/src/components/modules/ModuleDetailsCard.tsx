import { WasmModuleInfo } from '@/types'
import { formatBytes } from '@/utils/wasm'

interface ModuleDetailsCardProps {
  moduleInfo: WasmModuleInfo | null
}

export function ModuleDetailsCard({ moduleInfo }: ModuleDetailsCardProps) {
  // Mock inspection data - in real implementation this would come from the inspect command
  const inspection = moduleInfo?.inspection || {
    magicBytes: '00 61 73 6D',
    version: 1,
    totalSections: 14,
    isValid: true,
    warnings: ['Sections in incorrect order', 'Memory section with invalid configuration'],
    sections: [
      { name: 'Type', id: 1, size: 83, offset: '0x00000008-0x0000005C' },
      { name: 'Import', id: 2, size: 232, offset: '0x0000005D-0x00000147' },
      { name: 'Function', id: 3, size: 98, offset: '0x00000148-0x000001AB' },
      { name: 'Export', id: 7, size: 435, offset: '0x000001D3-0x00000388' },
      { name: 'Code', id: 10, size: 19937, offset: '0x000003AB-0x0000518F' },
    ]
  }

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
                  {inspection.totalSections}
                </div>
                <div class="text-xs text-light-textDim dark:text-dark-textDim">Sections</div>
              </div>
              <div class="p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
                <div class="text-sm font-bold text-light-textPrimary dark:text-dark-textPrimary">
                  v{inspection.version}
                </div>
                <div class="text-xs text-light-textDim dark:text-dark-textDim">WASM</div>
              </div>
              <div class="p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
                <span
                  class={`text-xs px-2 py-1 rounded-full font-medium ${
                    inspection.isValid
                      ? 'bg-green-100 dark:bg-green-900/20 text-green-700 dark:text-green-300'
                      : 'bg-red-100 dark:bg-red-900/20 text-red-700 dark:text-red-300'
                  }`}
                >
                  {inspection.isValid ? 'Valid' : 'Invalid'}
                </span>
                <div class="text-xs text-light-textDim dark:text-dark-textDim mt-1">Status</div>
              </div>
            </div>

            <div class="space-y-2 mb-3">
              <div class="flex items-center justify-between text-sm">
                <span class="text-light-textDim dark:text-dark-textDim">Magic Bytes</span>
                <code class="font-mono text-xs bg-light-surface3 dark:bg-dark-surface3 px-2 py-1 rounded">
                  {inspection.magicBytes}
                </code>
              </div>
            </div>

            <div>
              <h5 class="text-xs font-medium text-light-textPrimary dark:text-dark-textPrimary mb-2">
                Key Sections
              </h5>
              <div class="max-h-24 overflow-y-auto space-y-1">
                {inspection.sections.slice(0, 4).map((section, idx) => (
                  <div key={idx} class="flex items-center justify-between text-xs p-2 bg-light-surface3 dark:bg-dark-surface3 rounded">
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

            {inspection.warnings.length > 0 && (
              <div class="mt-3 p-2 bg-yellow-50 dark:bg-yellow-900/20 rounded-lg">
                <div class="text-xs text-yellow-700 dark:text-yellow-300 font-medium mb-1">
                  ⚠️ {inspection.warnings.length} Warning(s)
                </div>
                <div class="text-xs text-yellow-600 dark:text-yellow-400">
                  {inspection.warnings[0]}
                </div>
              </div>
            )}
          </div>
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