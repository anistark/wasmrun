import { useEffect, useRef, useState } from 'preact/hooks'
import { LogEntry } from '@/types'
import clsx from 'clsx'

interface LogContainerProps {
  logs: LogEntry[]
  height?: string
  onCommand?: (command: string) => void
  interactive?: boolean
}

export function LogContainer({
  logs,
  height = 'h-full',
  onCommand,
  interactive = false,
}: LogContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const [currentCommand, setCurrentCommand] = useState('')
  const [commandHistory, setCommandHistory] = useState<string[]>([])
  const [historyIndex, setHistoryIndex] = useState(-1)
  const [isUserScrolling, setIsUserScrolling] = useState(false)

  useEffect(() => {
    if (containerRef.current && !isUserScrolling) {
      containerRef.current.scrollTo({
        top: containerRef.current.scrollHeight,
        behavior: 'smooth',
      })
    }
  }, [logs, isUserScrolling])

  const handleScroll = () => {
    if (containerRef.current) {
      const { scrollTop, scrollHeight, clientHeight } = containerRef.current
      const isAtBottom = Math.abs(scrollHeight - scrollTop - clientHeight) < 5
      setIsUserScrolling(!isAtBottom)
    }
  }

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      if (currentCommand.trim() && onCommand) {
        onCommand(currentCommand.trim())
        setCommandHistory(prev => [...prev, currentCommand.trim()])
        setCurrentCommand('')
        setHistoryIndex(-1)
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (commandHistory.length > 0) {
        const newIndex =
          historyIndex === -1 ? commandHistory.length - 1 : Math.max(0, historyIndex - 1)
        setHistoryIndex(newIndex)
        setCurrentCommand(commandHistory[newIndex])
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (historyIndex !== -1) {
        const newIndex = historyIndex + 1
        if (newIndex >= commandHistory.length) {
          setHistoryIndex(-1)
          setCurrentCommand('')
        } else {
          setHistoryIndex(newIndex)
          setCurrentCommand(commandHistory[newIndex])
        }
      }
    }
  }

  const handleContainerClick = () => {
    if (interactive && inputRef.current) {
      inputRef.current.focus()
    }
  }

  return (
    <div class={clsx('flex flex-col', height)} onClick={handleContainerClick}>
      <div
        ref={containerRef}
        onScroll={handleScroll}
        class={clsx(
          'bg-light-bg dark:bg-dark-bg border border-light-surface3 dark:border-dark-surface3 rounded-t p-4 overflow-y-auto text-left font-mono text-sm flex-1 cursor-text max-h-full',
          !interactive && 'rounded-b'
        )}
        style={{ scrollBehavior: 'smooth' }}
      >
        {logs.length === 0 ? (
          <div class="text-light-textDim dark:text-dark-textDim italic">
            {interactive ? '' : 'No logs yet...'}
          </div>
        ) : (
          logs.map((log, index) => (
            <div
              key={index}
              class={clsx('mb-1 flex justify-between', {
                'text-light-success dark:text-dark-success': log.type === 'success',
                'text-light-error dark:text-dark-error': log.type === 'error',
                'text-light-info dark:text-dark-info': log.type === 'info',
                'text-light-warning dark:text-dark-warning': log.type === 'warning',
              })}
            >
              <span class="flex-1">{log.message}</span>
              <span class="text-light-textDim dark:text-dark-textDim text-xs ml-4">
                {log.timestamp.toLocaleTimeString()}
              </span>
            </div>
          ))
        )}
      </div>

      {interactive && (
        <div class="bg-light-bg dark:bg-dark-bg border-l border-r border-b border-light-surface3 dark:border-dark-surface3 rounded-b p-4">
          <div class="flex items-center gap-2">
            <span class="text-light-accent2 dark:text-dark-accent2 font-mono text-sm font-bold">
              &gt;
            </span>
            <input
              ref={inputRef}
              type="text"
              value={currentCommand}
              onInput={e => setCurrentCommand((e.target as HTMLInputElement).value)}
              onKeyDown={handleKeyDown}
              class="flex-1 bg-transparent text-light-textMuted dark:text-dark-textMuted border-none outline-none font-mono text-sm"
              autoFocus
            />
          </div>
        </div>
      )}
    </div>
  )
}
