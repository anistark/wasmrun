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

export function ModuleInfo({ moduleInfo }: ModuleInfoProps) {
  return (
    <div class="space-y-6">
      <div>
        <h3 class="text-xl font-medium text-dark-textMuted mb-4">WebAssembly Module Information</h3>

        {moduleInfo ? (
          <div class="bg-dark-surface2 rounded-lg p-4 border border-dark-surface3">
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
              <div>
                <h4 class="font-medium text-dark-textMuted mb-2">Module Details</h4>
                <div class="space-y-1 text-sm">
                  <div>
                    <span class="text-dark-textDim">Name:</span> {moduleInfo.name}
                  </div>
                  <div>
                    <span class="text-dark-textDim">Size:</span> {formatBytes(moduleInfo.size)}
                  </div>
                  <div>
                    <span class="text-dark-textDim">WASI Support:</span>{' '}
                    {moduleInfo.isWasi ? 'Yes' : 'No'}
                  </div>
                </div>
              </div>

              <div>
                <h4 class="font-medium text-dark-textMuted mb-2">Exports</h4>
                <div class="text-sm text-dark-textDim max-h-32 overflow-y-auto">
                  {moduleInfo.exports.length > 0 ? (
                    <ul class="space-y-1 font-mono">
                      {moduleInfo.exports.map((exp, idx) => (
                        <li key={idx}>• {exp}</li>
                      ))}
                    </ul>
                  ) : (
                    <span class="italic">No exports found</span>
                  )}
                </div>
              </div>
            </div>

            {moduleInfo.imports.length > 0 && (
              <div>
                <h4 class="font-medium text-dark-textMuted mb-2">Imports</h4>
                <div class="text-sm text-dark-textDim max-h-32 overflow-y-auto">
                  <ul class="space-y-1 font-mono">
                    {moduleInfo.imports.map((imp, idx) => (
                      <li key={idx}>• {imp}</li>
                    ))}
                  </ul>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div class="bg-dark-surface2 rounded-lg p-4 border border-dark-surface3 text-center">
            <p class="text-dark-textDim">Module will be analyzed after loading...</p>
          </div>
        )}
      </div>

      <div>
        <h3 class="text-xl font-medium text-dark-textMuted mb-4">WASI Support</h3>
        <p class="text-sm text-dark-textDim mb-4">
          Wasmrun includes a full WASI implementation for running WebAssembly System Interface
          modules directly in your browser.
        </p>

        <div class="overflow-x-auto">
          <table class="w-full border-collapse">
            <thead>
              <tr class="bg-dark-surface2">
                <th class="text-left p-3 font-medium border-b border-dark-surface3">Feature</th>
                <th class="text-left p-3 font-medium border-b border-dark-surface3">Status</th>
                <th class="text-left p-3 font-medium border-b border-dark-surface3">Description</th>
              </tr>
            </thead>
            <tbody>
              {WASI_FEATURES.map((feature, idx) => (
                <tr key={idx}>
                  <td class="p-3 border-b border-dark-surface3 font-medium">{feature.name}</td>
                  <td
                    class={`p-3 border-b border-dark-surface3 font-medium ${
                      feature.status === 'supported'
                        ? 'text-green-400'
                        : feature.status === 'partial'
                          ? 'text-yellow-400'
                          : 'text-red-400'
                    }`}
                  >
                    {feature.status === 'supported'
                      ? 'Supported'
                      : feature.status === 'partial'
                        ? 'Partial'
                        : 'Unsupported'}
                  </td>
                  <td class="p-3 border-b border-dark-surface3 text-sm text-dark-textDim">
                    {feature.description}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}
