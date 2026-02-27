import { useEffect, useRef } from 'preact/hooks'
import { clsx } from 'clsx'
import type { ConsoleLine, StatusType } from '../../types/osTypes'
import type { WasmRunnerStatus } from '../../os/WasmRunner'

interface ConsolePanelProps {
  lines: ConsoleLine[]
  wasmStatus: WasmRunnerStatus
  runtimeStatus: StatusType
  onClear: () => void
  onRun: () => void
  onStop: () => void
}

function formatTimestamp(ts: number): string {
  const d = new Date(ts)
  return (
    d.toLocaleTimeString('en-US', { hour12: false }) +
    '.' +
    String(d.getMilliseconds()).padStart(3, '0')
  )
}

function statusLabel(wasmStatus: WasmRunnerStatus): string {
  switch (wasmStatus) {
    case 'idle':
      return 'Ready'
    case 'loading-runtime':
      return 'Loading runtime…'
    case 'loading-files':
      return 'Loading project files…'
    case 'populating-fs':
      return 'Populating filesystem…'
    case 'starting':
      return 'Starting…'
    case 'running':
      return 'Running'
    case 'stopped':
      return 'Stopped'
    case 'error':
      return 'Error'
  }
}

export default function ConsolePanel({
  lines,
  wasmStatus,
  runtimeStatus,
  onClear,
  onRun,
  onStop,
}: ConsolePanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [lines.length])

  const canRun = wasmStatus === 'idle' || wasmStatus === 'stopped' || wasmStatus === 'error'
  const canStop =
    wasmStatus === 'loading-runtime' || wasmStatus === 'loading-files' || wasmStatus === 'running'

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-4 flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold text-green-400">Console</h2>
          <div className="flex items-center gap-3 mt-1">
            <span
              className={clsx('text-xs px-2 py-0.5 rounded-full border', {
                'bg-gray-500/20 border-gray-500/40 text-gray-300': wasmStatus === 'idle',
                'bg-yellow-500/20 border-yellow-500/40 text-yellow-300':
                  wasmStatus === 'loading-runtime' ||
                  wasmStatus === 'loading-files' ||
                  wasmStatus === 'populating-fs' ||
                  wasmStatus === 'starting',
                'bg-green-500/20 border-green-500/40 text-green-300': wasmStatus === 'running',
                'bg-blue-500/20 border-blue-500/40 text-blue-300': wasmStatus === 'stopped',
                'bg-red-500/20 border-red-500/40 text-red-300': wasmStatus === 'error',
              })}
            >
              {statusLabel(wasmStatus)}
            </span>
            <span className="text-xs text-white/50">{lines.length} lines</span>
          </div>
        </div>
        <div className="flex gap-2">
          {canRun && (
            <button
              onClick={onRun}
              className="px-3 py-1.5 text-sm bg-green-600/80 hover:bg-green-600 border border-green-400/30 rounded-lg transition-all"
            >
              ▶ Run
            </button>
          )}
          {canStop && (
            <button
              onClick={onStop}
              className="px-3 py-1.5 text-sm bg-red-600/80 hover:bg-red-600 border border-red-400/30 rounded-lg transition-all"
            >
              ■ Stop
            </button>
          )}
          <button
            onClick={onClear}
            className="px-3 py-1.5 text-sm bg-white/10 hover:bg-white/20 border border-white/20 rounded-lg transition-all"
          >
            Clear
          </button>
        </div>
      </div>

      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto bg-black/60 font-mono text-sm p-4 space-y-px"
      >
        {lines.length === 0 && runtimeStatus !== 'running' && (
          <div className="text-white/30 text-center py-8">
            Press ▶ Run to start the WASM runtime
          </div>
        )}
        {lines.map(line => (
          <div key={line.id} className="flex gap-2 hover:bg-white/5 px-1 rounded">
            <span className="text-white/25 select-none shrink-0">
              {formatTimestamp(line.timestamp)}
            </span>
            <span
              className={clsx('whitespace-pre-wrap break-all', {
                'text-green-100': line.stream === 'stdout',
                'text-red-400': line.stream === 'stderr',
                'text-blue-400': line.stream === 'system',
              })}
            >
              {line.text}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
