import { useEffect, useRef } from 'preact/hooks'
import { LogEntry } from '@/types'
import clsx from 'clsx'

interface LogContainerProps {
  logs: LogEntry[]
  height?: string
}

export function LogContainer({ logs, height = 'h-full' }: LogContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight
    }
  }, [logs])

  return (
    <div
      ref={containerRef}
      class={clsx(
        'bg-light-bg dark:bg-dark-bg border border-light-surface3 dark:border-dark-surface3 rounded p-4 overflow-y-auto text-left font-mono text-sm',
        height
      )}
    >
      {logs.length === 0 ? (
        <div class="text-light-textDim dark:text-dark-textDim italic">No logs yet...</div>
      ) : (
        logs.map((log, index) => (
          <div
            key={index}
            class={clsx('mb-1', {
              'text-light-success dark:text-dark-success': log.type === 'success',
              'text-light-error dark:text-dark-error': log.type === 'error',
              'text-light-info dark:text-dark-info': log.type === 'info',
              'text-light-warning dark:text-dark-warning': log.type === 'warning',
            })}
          >
            <span class="text-light-textDim dark:text-dark-textDim">
              [{log.timestamp.toLocaleTimeString()}]
            </span>{' '}
            {log.message}
          </div>
        ))
      )}
    </div>
  )
}
