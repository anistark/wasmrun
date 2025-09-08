import { StatusMessage } from '@/types'
import clsx from 'clsx'

interface StatusBarProps {
  status: StatusMessage
}

export function StatusBar({ status }: StatusBarProps) {
  return (
    <div class="bg-dark-surface2 px-4 py-2 flex items-center justify-between border-b border-dark-surface3">
      <div
        class={clsx('flex items-center', {
          'text-dark-success': status.type === 'success',
          'text-dark-error': status.type === 'error',
          'text-dark-info': status.type === 'info',
          'text-dark-warning': status.type === 'warning',
        })}
      >
        {status.type === 'info' && (
          <div class="inline-block w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin mr-2"></div>
        )}
        <span>{status.message}</span>
      </div>
    </div>
  )
}
