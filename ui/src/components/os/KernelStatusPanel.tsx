import type { KernelStats } from '../../types/osTypes'

interface KernelStatusPanelProps {
  kernelStats: KernelStats | null
  uptime: number
  formatUptime: (seconds: number) => string
}

export default function KernelStatusPanel({
  kernelStats,
  uptime,
  formatUptime,
}: KernelStatusPanelProps) {
  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-6">
        <h2 className="text-2xl font-bold mb-2 text-green-400">Kernel Status</h2>
        <p className="text-white/80">WebAssembly Micro-Kernel Information</p>
      </div>
      <div className="flex-1 p-6 overflow-y-auto">
        {/* Primary Stats Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
          <StatCard title="Kernel Status" value={kernelStats?.status || 'Loading...'} />
          <StatCard title="Active Processes" value={kernelStats?.active_processes || 0} />
          <StatCard title="Memory Usage" value={`${kernelStats?.total_memory_usage || 0} MB`} />
          <StatCard title="Uptime" value={formatUptime(uptime)} />
        </div>

        {/* System Information */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
          <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-6">
            <h3 className="text-lg font-semibold mb-4 text-green-400">System Information</h3>
            <div className="space-y-3">
              <InfoRow label="Operating System" value={kernelStats?.os || 'N/A'} />
              <InfoRow label="Architecture" value={kernelStats?.arch || 'N/A'} />
              <InfoRow
                label="Kernel Version"
                value={`v${kernelStats?.kernel_version || '0.0.0'}`}
              />
              <InfoRow label="Filesystem Mounts" value={kernelStats?.filesystem_mounts || 0} />
              <InfoRow label="Dev Servers" value={kernelStats?.active_dev_servers || 0} />
            </div>
          </div>

          {/* WASI Capabilities */}
          <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-6">
            <h3 className="text-lg font-semibold mb-4 text-green-400">WASI Capabilities</h3>
            <div className="flex flex-wrap gap-2">
              {kernelStats?.wasi_capabilities?.map(capability => (
                <span
                  key={capability}
                  className="px-3 py-1 bg-blue-500/30 border border-blue-400/50 rounded-full text-sm"
                >
                  {capability}
                </span>
              )) || <span className="text-white/50 text-sm">No capabilities loaded</span>}
            </div>
          </div>
        </div>

        {/* Active Runtimes */}
        {kernelStats?.active_runtimes && kernelStats.active_runtimes.length > 0 && (
          <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-6 mb-8">
            <h3 className="text-lg font-semibold mb-4 text-green-400">Active Runtimes</h3>
            <div className="flex flex-wrap gap-2">
              {kernelStats.active_runtimes.map(runtime => (
                <span
                  key={runtime}
                  className="px-3 py-1 bg-green-500/30 border border-green-400/50 rounded-full text-sm"
                >
                  {runtime}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Supported Languages */}
        {kernelStats?.supported_languages && kernelStats.supported_languages.length > 0 && (
          <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-6">
            <h3 className="text-lg font-semibold mb-4 text-green-400">Supported Languages</h3>
            <div className="flex flex-wrap gap-2">
              {kernelStats.supported_languages.map(lang => (
                <span
                  key={lang}
                  className="px-3 py-1 bg-purple-500/30 border border-purple-400/50 rounded-full text-sm"
                >
                  {lang}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

interface StatCardProps {
  title: string
  value: string | number
}

function StatCard({ title, value }: StatCardProps) {
  return (
    <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-4 hover:scale-105 transition-transform">
      <div className="text-sm font-medium text-green-400/90 mb-2">{title}</div>
      <div className="text-2xl font-bold">{value}</div>
    </div>
  )
}

interface InfoRowProps {
  label: string
  value: string | number
}

function InfoRow({ label, value }: InfoRowProps) {
  return (
    <div className="flex justify-between items-center">
      <span className="text-white/70">{label}</span>
      <span className="font-mono text-green-300">{value}</span>
    </div>
  )
}
