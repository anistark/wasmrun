import { StatusMessage } from '@/types'
import clsx from 'clsx'

interface StatusBarProps {
  status: StatusMessage
}

export function StatusBar({ status }: StatusBarProps) {
  return (
    <div class="bg-light-surface2 dark:bg-dark-surface2 px-4 py-2 flex items-center justify-between border-b border-light-surface3 dark:border-dark-surface3">
      <div
        class={clsx('flex items-center', {
          'text-light-success dark:text-dark-success': status.type === 'success',
          'text-light-error dark:text-dark-error': status.type === 'error',
          'text-light-info dark:text-dark-info': status.type === 'info',
          'text-light-warning dark:text-dark-warning': status.type === 'warning',
        })}
      >
        {status.type === 'info' && (
          <div class="inline-block w-5 h-5 border-2 border-light-textMuted/30 dark:border-white/30 border-t-light-textMuted dark:border-t-white rounded-full animate-spin mr-2"></div>
        )}
        <span>{status.message}</span>
      </div>
    </div>
  )
}
