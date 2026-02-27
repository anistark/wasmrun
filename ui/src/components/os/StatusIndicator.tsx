import { clsx } from 'clsx'
import type { StatusType } from '../../types/osTypes'

interface StatusIndicatorProps {
  status: StatusType
  label: string
}

export default function StatusIndicator({ status, label }: StatusIndicatorProps) {
  return (
    <div
      className={clsx('flex items-center gap-2 px-3 py-2 rounded-full text-sm font-medium border', {
        'bg-yellow-500/20 border-yellow-500/50 text-yellow-200': status === 'loading',
        'bg-green-500/20 border-green-500/50 text-green-200': status === 'running',
        'bg-blue-500/20 border-blue-500/50 text-blue-200': status === 'stopped',
        'bg-red-500/20 border-red-500/50 text-red-200': status === 'error',
      })}
    >
      <div
        className={clsx('w-2 h-2 rounded-full', {
          'bg-yellow-400 animate-pulse': status === 'loading',
          'bg-green-400 animate-pulse': status === 'running',
          'bg-blue-400': status === 'stopped',
          'bg-red-400': status === 'error',
        })}
      />
      <span>{label}</span>
    </div>
  )
}
