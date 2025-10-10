import StatusIndicator from './StatusIndicator'
import type { StatusType, KernelStats } from '../../types/osTypes'

interface HeaderProps {
  projectName: string
  kernelStatus: StatusType
  runtimeStatus: StatusType
  kernelStats: KernelStats | null
}

export default function Header({
  projectName,
  kernelStatus,
  runtimeStatus,
  kernelStats,
}: HeaderProps) {
  return (
    <header className="bg-black/30 backdrop-blur-xl border-b border-green-500/20 p-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <img src="/assets/logo-text.png" alt="wasmrun OS" className="h-8 object-contain" />
          <div className="flex flex-col">
            <span className="text-white font-bold text-lg">OS</span>
            <span className="text-green-400/80 text-xs">{projectName}</span>
          </div>
        </div>
        <div className="flex items-center gap-4">
          <StatusIndicator
            status={kernelStatus}
            label={kernelStatus === 'running' ? 'Kernel Active' : 'Initializing Kernel...'}
          />
          <StatusIndicator
            status={runtimeStatus}
            label={
              runtimeStatus === 'running'
                ? `Runtime Active (PID: ${kernelStats?.project_pid || 'N/A'})`
                : 'Loading Runtime...'
            }
          />
        </div>
      </div>
    </header>
  )
}
