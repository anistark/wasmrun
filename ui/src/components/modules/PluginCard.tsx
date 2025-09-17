import { WasmModuleInfo } from '@/types'

interface PluginCardProps {
  moduleInfo: WasmModuleInfo | null
}

export function PluginCard({ moduleInfo }: PluginCardProps) {
  const pluginInfo = moduleInfo?.inspection?.plugin ||
    moduleInfo?.plugin || {
      name: 'unknown',
      version: '0.0.0',
      type: 'builtin' as const,
      description: 'Plugin information unavailable',
      author: undefined,
      source: undefined,
      capabilities: undefined,
    }

  const getSourceEmoji = (source?: { type: string }) => {
    switch (source?.type) {
      case 'crates.io':
        return '🦀'
      case 'local':
        return '💻'
      case 'git':
        return '🌐'
      default:
        return '📦'
    }
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
          <div class="w-16 h-16 bg-gradient-to-br from-cyan-500 to-blue-600 rounded-xl flex items-center justify-center p-2">
            <img
              src="/assets/logo-w.png"
              alt="Plugin"
              class="w-full h-full object-contain"
              onError={e => {
                // Fallback to text if image fails to load
                const img = e.currentTarget as HTMLImageElement
                const fallback = img.nextElementSibling as HTMLSpanElement
                img.style.display = 'none'
                if (fallback) fallback.style.display = 'flex'
              }}
            />
            <span class="text-white text-2xl font-bold hidden">
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
          {(pluginInfo.author || pluginInfo.source) && (
            <div class="flex items-center justify-center gap-3 text-xs text-light-textDim dark:text-dark-textDim">
              {pluginInfo.author && <span>by {pluginInfo.author}</span>}
              {pluginInfo.source && (
                <span class="flex items-center gap-1">
                  {getSourceEmoji(pluginInfo.source)}
                  {pluginInfo.source.url ? (
                    <a
                      href={pluginInfo.source.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      class="hover:text-blue-500"
                    >
                      {pluginInfo.source.type}
                    </a>
                  ) : (
                    <span>{pluginInfo.source.type}</span>
                  )}
                </span>
              )}
            </div>
          )}
        </div>

        <div class="p-3 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
          <p class="text-xs text-light-textDim dark:text-dark-textDim text-center">
            {pluginInfo.description}
          </p>
        </div>

        <div class="space-y-2">
          <div class="text-xs font-medium text-light-textDim dark:text-dark-textDim text-center">
            Capabilities
          </div>
          <div class="grid grid-cols-2 gap-2 text-center">
            <div
              class={`p-2 rounded ${
                pluginInfo.capabilities?.compile_wasm
                  ? 'bg-green-50 dark:bg-green-900/20'
                  : 'bg-gray-50 dark:bg-gray-900/20'
              }`}
            >
              <div
                class={`text-lg font-bold ${
                  pluginInfo.capabilities?.compile_wasm
                    ? 'text-green-600 dark:text-green-400'
                    : 'text-gray-400 dark:text-gray-600'
                }`}
              >
                {pluginInfo.capabilities?.compile_wasm ? '🔨' : '❌'}
              </div>
              <div
                class={`text-xs ${
                  pluginInfo.capabilities?.compile_wasm
                    ? 'text-green-700 dark:text-green-300'
                    : 'text-gray-500 dark:text-gray-400'
                }`}
              >
                WASM
              </div>
            </div>
            <div
              class={`p-2 rounded ${
                pluginInfo.capabilities?.optimization
                  ? 'bg-blue-50 dark:bg-blue-900/20'
                  : 'bg-gray-50 dark:bg-gray-900/20'
              }`}
            >
              <div
                class={`text-lg font-bold ${
                  pluginInfo.capabilities?.optimization
                    ? 'text-blue-600 dark:text-blue-400'
                    : 'text-gray-400 dark:text-gray-600'
                }`}
              >
                {pluginInfo.capabilities?.optimization ? '⚡' : '❌'}
              </div>
              <div
                class={`text-xs ${
                  pluginInfo.capabilities?.optimization
                    ? 'text-blue-700 dark:text-blue-300'
                    : 'text-gray-500 dark:text-gray-400'
                }`}
              >
                Optimize
              </div>
            </div>
            <div
              class={`p-2 rounded ${
                pluginInfo.capabilities?.live_reload
                  ? 'bg-purple-50 dark:bg-purple-900/20'
                  : 'bg-gray-50 dark:bg-gray-900/20'
              }`}
            >
              <div
                class={`text-lg font-bold ${
                  pluginInfo.capabilities?.live_reload
                    ? 'text-purple-600 dark:text-purple-400'
                    : 'text-gray-400 dark:text-gray-600'
                }`}
              >
                {pluginInfo.capabilities?.live_reload ? '🔄' : '❌'}
              </div>
              <div
                class={`text-xs ${
                  pluginInfo.capabilities?.live_reload
                    ? 'text-purple-700 dark:text-purple-300'
                    : 'text-gray-500 dark:text-gray-400'
                }`}
              >
                Hot Reload
              </div>
            </div>
            <div
              class={`p-2 rounded ${
                pluginInfo.capabilities?.compile_webapp
                  ? 'bg-orange-50 dark:bg-orange-900/20'
                  : 'bg-gray-50 dark:bg-gray-900/20'
              }`}
            >
              <div
                class={`text-lg font-bold ${
                  pluginInfo.capabilities?.compile_webapp
                    ? 'text-orange-600 dark:text-orange-400'
                    : 'text-gray-400 dark:text-gray-600'
                }`}
              >
                {pluginInfo.capabilities?.compile_webapp ? '🌐' : '❌'}
              </div>
              <div
                class={`text-xs ${
                  pluginInfo.capabilities?.compile_webapp
                    ? 'text-orange-700 dark:text-orange-300'
                    : 'text-gray-500 dark:text-gray-400'
                }`}
              >
                WebApp
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
