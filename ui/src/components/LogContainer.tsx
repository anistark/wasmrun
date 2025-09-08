import { useEffect, useRef } from 'preact/hooks'
import { LogEntry } from '@/types'
import clsx from 'clsx'

interface LogContainerProps {
  logs: LogEntry[]
  height?: string
}

export function LogContainer({ logs, height = 'h-80' }: LogContainerProps) {
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
        'bg-dark-bg border border-dark-surface3 rounded p-4 overflow-y-auto text-left font-mono text-sm',
        height
      )}
    >
      {logs.length === 0 ? (
        <div class="text-dark-textDim italic">No logs yet...</div>
      ) : (
        logs.map((log, index) => (
          <div
            key={index}
            class={clsx('mb-1', {
              'text-dark-success': log.type === 'success',
              'text-dark-error': log.type === 'error',
              'text-dark-info': log.type === 'info',
              'text-dark-warning': log.type === 'warning',
            })}
          >
            <span class="text-dark-textDim">[{log.timestamp.toLocaleTimeString()}]</span>{' '}
            {log.message}
          </div>
        ))
      )}
    </div>
  )
}
