import { WasmModuleInfo } from '@/types'

interface PluginCardProps {
  moduleInfo: WasmModuleInfo | null
}

export function PluginCard({ moduleInfo }: PluginCardProps) {
  // Mock plugin data for now - in real implementation this would come from the server
  const pluginInfo = moduleInfo?.plugin || {
    name: 'wasmrust',
    version: '0.3.1',
    type: 'external' as const,
    description: 'Rust WebAssembly compilation plugin with wasm-bindgen support',
  }

  return (
    <div class="bg-light-surface2 dark:bg-dark-surface2 rounded-xl p-6 border border-light-surface3 dark:border-dark-surface3 h-full">
      <div class="flex items-center mb-4">
        <div class="w-3 h-3 bg-cyan-500 rounded-full mr-3"></div>
        <h3 class="text-lg font-semibold text-light-textPrimary dark:text-dark-textPrimary">
          Build Plugin
        </h3>
      </div>
      
      <div class="space-y-4">
        <div class="flex items-center justify-center">
          <div class="w-16 h-16 bg-gradient-to-br from-cyan-500 to-blue-600 rounded-xl flex items-center justify-center">
            <span class="text-white text-2xl font-bold">
              {pluginInfo.name.slice(0, 2).toUpperCase()}
            </span>
          </div>
        </div>
        
        <div class="text-center space-y-2">
          <h4 class="font-semibold text-light-textPrimary dark:text-dark-textPrimary">
            {pluginInfo.name}
          </h4>
          <div class="flex items-center justify-center gap-2">
            <span class="text-sm text-light-textDim dark:text-dark-textDim">
              v{pluginInfo.version}
            </span>
            <span
              class={`px-2 py-1 rounded-full text-xs font-medium ${
                pluginInfo.type === 'external'
                  ? 'bg-blue-100 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300'
                  : 'bg-gray-100 dark:bg-gray-900/20 text-gray-700 dark:text-gray-300'
              }`}
            >
              {pluginInfo.type === 'external' ? 'External' : 'Built-in'}
            </span>
          </div>
        </div>
        
        <div class="p-3 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
          <p class="text-xs text-light-textDim dark:text-dark-textDim text-center">
            {pluginInfo.description}
          </p>
        </div>
        
        <div class="grid grid-cols-2 gap-2 text-center">
          <div class="p-2 bg-green-50 dark:bg-green-900/20 rounded">
            <div class="text-lg font-bold text-green-600 dark:text-green-400">✓</div>
            <div class="text-xs text-green-700 dark:text-green-300">Active</div>
          </div>
          <div class="p-2 bg-blue-50 dark:bg-blue-900/20 rounded">
            <div class="text-lg font-bold text-blue-600 dark:text-blue-400">⚡</div>
            <div class="text-xs text-blue-700 dark:text-blue-300">Optimized</div>
          </div>
        </div>
      </div>
    </div>
  )
}