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
        'bg-red-500/20 border-red-500/50 text-red-200': status === 'error',
      })}
    >
      <div
        className={clsx('w-2 h-2 rounded-full animate-pulse', {
          'bg-yellow-400': status === 'loading',
          'bg-green-400': status === 'running',
          'bg-red-400': status === 'error',
        })}
      />
      <span>{label}</span>
    </div>
  )
}
