import { useState } from 'preact/hooks'
import { WasmModuleInfo } from '@/types'
import { formatBytes } from '@/utils/wasm'

interface ModuleInfoProps {
  moduleInfo: WasmModuleInfo | null
}

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

function ModuleDetailsCard({ moduleInfo }: { moduleInfo: WasmModuleInfo | null }) {
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

function ExportsCard({ moduleInfo }: { moduleInfo: WasmModuleInfo | null }) {
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

function ImportsCard({ moduleInfo }: { moduleInfo: WasmModuleInfo | null }) {
  return (
    <div class="bg-light-surface2 dark:bg-dark-surface2 rounded-xl p-6 border border-light-surface3 dark:border-dark-surface3 h-full">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center">
          <div class="w-3 h-3 bg-purple-500 rounded-full mr-3"></div>
          <h3 class="text-lg font-semibold text-light-textPrimary dark:text-dark-textPrimary">
            Imports
          </h3>
        </div>
        {moduleInfo && (
          <span class="text-xs bg-light-surface3 dark:bg-dark-surface3 px-2 py-1 rounded-full text-light-textDim dark:text-dark-textDim">
            {moduleInfo.imports?.length || 0}
          </span>
        )}
      </div>

      <div class="max-h-48 overflow-y-auto">
        {moduleInfo?.imports && moduleInfo.imports.length > 0 ? (
          <div class="space-y-2">
            {moduleInfo.imports.map((imp, idx) => (
              <div
                key={idx}
                class="flex items-center p-2 bg-light-surface3 dark:bg-dark-surface3 rounded-lg"
              >
                <span class="w-2 h-2 bg-purple-400 rounded-full mr-3 flex-shrink-0"></span>
                <code class="font-mono text-sm text-light-textPrimary dark:text-dark-textPrimary">
                  {imp}
                </code>
              </div>
            ))}
          </div>
        ) : moduleInfo ? (
          <div class="text-center py-4">
            <span class="text-2xl mb-2 block">📭</span>
            <p class="text-light-textDim dark:text-dark-textDim text-sm">No imports required</p>
          </div>
        ) : (
          <div class="text-center py-4">
            <span class="text-2xl mb-2 block">⏳</span>
            <p class="text-light-textDim dark:text-dark-textDim text-sm">Loading imports...</p>
          </div>
        )}
      </div>
    </div>
  )
}

function WasiSupportCard() {
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
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
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

function PluginCard({ moduleInfo }: { moduleInfo: WasmModuleInfo | null }) {
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


export function ModuleInfo({ moduleInfo }: ModuleInfoProps) {
  return (
    <div class="p-6 space-y-6">
      <div>
        <h2 class="text-2xl font-bold text-light-textPrimary dark:text-dark-textPrimary mb-2">
          WebAssembly Module Analysis
        </h2>
        <p class="text-light-textDim dark:text-dark-textDim">
          Comprehensive analysis of your WebAssembly module including binary inspection and plugin
          details
        </p>
      </div>

      {/* Responsive Bento Grid */}
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        {/* Module Details with Binary Analysis - Taller card */}
        <div class="md:col-span-1 xl:col-span-1">
          <ModuleDetailsCard moduleInfo={moduleInfo} />
        </div>

        {/* Exports */}
        <div class="md:col-span-1 xl:col-span-1">
          <ExportsCard moduleInfo={moduleInfo} />
        </div>

        {/* Imports */}
        <div class="md:col-span-1 xl:col-span-1">
          <ImportsCard moduleInfo={moduleInfo} />
        </div>

        {/* Plugin Info - New row */}
        <div class="md:col-span-1 xl:col-span-1">
          <PluginCard moduleInfo={moduleInfo} />
        </div>

        {/* WASI Support with tabs - spans 2 columns */}
        <div class="md:col-span-2 xl:col-span-2">
          <WasiSupportCard />
        </div>
      </div>
    </div>
  )
}
