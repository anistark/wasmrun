import { useState } from 'preact/hooks'

interface WasiFeature {
  name: string
  status: 'supported' | 'partial' | 'unsupported'
  description: string
}

const WASI_FEATURES: WasiFeature[] = [
  {
    name: 'Virtual Filesystem',
    status: 'supported',
    description: 'Full filesystem with directory structure, file creation and manipulation',
  },
  {
    name: 'Standard I/O',
    status: 'supported',
    description: 'stdout, stderr with console integration',
  },
  {
    name: 'Environment Variables',
    status: 'supported',
    description: 'Reading environment variables',
  },
  {
    name: 'Command Arguments',
    status: 'supported',
    description: 'Access to command-line arguments',
  },
  {
    name: 'File I/O',
    status: 'supported',
    description: 'Read/write operations on files',
  },
  {
    name: 'Random Number Generation',
    status: 'supported',
    description: 'Secure random number generation using crypto API',
  },
  {
    name: 'Time Functions',
    status: 'supported',
    description: 'Access to system time and high-precision timers',
  },
  {
    name: 'Pre-opened Directories',
    status: 'supported',
    description: 'Access to pre-opened filesystem paths',
  },
  {
    name: 'Network Sockets',
    status: 'unsupported',
    description: 'Network operations (will be added in future updates)',
  },
  {
    name: 'Multi-threading',
    status: 'unsupported',
    description: 'Thread-related APIs (will be added in future updates)',
  },
]

export function WasiSupportCard() {
  const [activeTab, setActiveTab] = useState<'supported' | 'partial' | 'unsupported' | 'all'>('all')

  const supportedFeatures = WASI_FEATURES.filter(f => f.status === 'supported')
  const partialFeatures = WASI_FEATURES.filter(f => f.status === 'partial')
  const unsupportedFeatures = WASI_FEATURES.filter(f => f.status === 'unsupported')

  const getFilteredFeatures = () => {
    switch (activeTab) {
      case 'supported':
        return supportedFeatures
      case 'partial':
        return partialFeatures
      case 'unsupported':
        return unsupportedFeatures
      default:
        return WASI_FEATURES
    }
  }

  const tabs = [
    { id: 'all' as const, label: 'All Features', count: WASI_FEATURES.length },
    { id: 'supported' as const, label: 'Supported', count: supportedFeatures.length },
    { id: 'partial' as const, label: 'Partial', count: partialFeatures.length },
    { id: 'unsupported' as const, label: 'Coming Soon', count: unsupportedFeatures.length },
  ]

  return (
    <div class="bg-light-surface2 dark:bg-dark-surface2 rounded-xl p-6 border border-light-surface3 dark:border-dark-surface3 h-full">
      <div class="flex items-center mb-4">
        <div class="w-3 h-3 bg-orange-500 rounded-full mr-3"></div>
        <h3 class="text-lg font-semibold text-light-textPrimary dark:text-dark-textPrimary">
          WASI Support
        </h3>
      </div>

      <div class="space-y-4">
        <p class="text-sm text-light-textDim dark:text-dark-textDim">
          Comprehensive WASI implementation for WebAssembly System Interface modules.
        </p>

        {/* Tab Navigation */}
        <div class="flex flex-wrap gap-1 p-1 bg-light-surface3 dark:bg-dark-surface3 rounded-lg">
          {tabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              class={`px-3 py-2 text-xs font-medium rounded-md transition-all flex items-center gap-2 ${
                activeTab === tab.id
                  ? 'bg-light-surface2 dark:bg-dark-surface2 text-light-textPrimary dark:text-dark-textPrimary shadow-sm'
                  : 'text-light-textDim dark:text-dark-textDim hover:text-light-textPrimary dark:hover:text-dark-textPrimary'
              }`}
            >
              {tab.label}
              <span
                class={`px-1.5 py-0.5 rounded-full text-xs ${
                  activeTab === tab.id
                    ? 'bg-light-surface3 dark:bg-dark-surface3'
                    : 'bg-light-surface2 dark:bg-dark-surface2'
                }`}
              >
                {tab.count}
              </span>
            </button>
          ))}
        </div>

        {/* Features Grid */}
        <div class="max-h-56 overflow-y-auto">
          <div class="grid grid-cols-2 sm:grid-cols-6 gap-3">
            {getFilteredFeatures().map((feature, idx) => (
              <div
                key={idx}
                class={`p-4 rounded-lg border transition-all hover:shadow-sm ${
                  feature.status === 'supported'
                    ? 'bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800/30'
                    : feature.status === 'partial'
                      ? 'bg-yellow-50 dark:bg-yellow-900/20 border-yellow-200 dark:border-yellow-800/30'
                      : 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800/30'
                }`}
              >
                <div class="flex items-center justify-between mb-2">
                  <div
                    class={`w-8 h-8 rounded-full flex items-center justify-center ${
                      feature.status === 'supported'
                        ? 'bg-green-100 dark:bg-green-800/50'
                        : feature.status === 'partial'
                          ? 'bg-yellow-100 dark:bg-yellow-800/50'
                          : 'bg-red-100 dark:bg-red-800/50'
                    }`}
                  >
                    <span
                      class={`text-sm ${
                        feature.status === 'supported'
                          ? 'text-green-600 dark:text-green-400'
                          : feature.status === 'partial'
                            ? 'text-yellow-600 dark:text-yellow-400'
                            : 'text-red-600 dark:text-red-400'
                      }`}
                    >
                      {feature.status === 'supported'
                        ? '✓'
                        : feature.status === 'partial'
                          ? '⚡'
                          : '○'}
                    </span>
                  </div>
                  <span
                    class={`text-xs px-2 py-1 rounded-full font-medium ${
                      feature.status === 'supported'
                        ? 'bg-green-100 dark:bg-green-800/30 text-green-700 dark:text-green-300'
                        : feature.status === 'partial'
                          ? 'bg-yellow-100 dark:bg-yellow-800/30 text-yellow-700 dark:text-yellow-300'
                          : 'bg-red-100 dark:bg-red-800/30 text-red-700 dark:text-red-300'
                    }`}
                  >
                    {feature.status === 'supported'
                      ? 'Ready'
                      : feature.status === 'partial'
                        ? 'Beta'
                        : 'Soon'}
                  </span>
                </div>
                <h4 class="font-semibold text-light-textPrimary dark:text-dark-textPrimary text-sm mb-1">
                  {feature.name}
                </h4>
                <p class="text-xs text-light-textDim dark:text-dark-textDim leading-relaxed">
                  {feature.description}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
