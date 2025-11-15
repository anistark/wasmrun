import { useState, useEffect, useCallback } from 'preact/hooks'
import type { LogEntry } from '../../types/osTypes'

interface LogsPanelProps {}

export default function LogsPanel(_props: LogsPanelProps) {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [filteredLogs, setFilteredLogs] = useState<LogEntry[]>([])
  const [levelFilter, setLevelFilter] = useState<string>('all')
  const [sourceFilter, setSourceFilter] = useState<string>('all')
  const [isAutoRefresh, setIsAutoRefresh] = useState(true)
  const [lastUpdateTime, setLastUpdateTime] = useState<string>('')

  const fetchLogs = useCallback(async () => {
    try {
      const response = await fetch('/api/logs/recent')
      const data = await response.json()
      if (data.success && data.logs) {
        setLogs(data.logs)
        setLastUpdateTime(new Date().toLocaleTimeString())
      }
    } catch (error) {
      console.error('Failed to fetch logs:', error)
    }
  }, [])

  // Filter logs whenever filters change
  useEffect(() => {
    const filtered = logs.filter(log => {
      const levelMatch = levelFilter === 'all' || log.level === levelFilter
      const sourceMatch = sourceFilter === 'all' || log.source === sourceFilter
      return levelMatch && sourceMatch
    })
    setFilteredLogs(filtered)
  }, [logs, levelFilter, sourceFilter])

  // Initial load and auto-refresh
  useEffect(() => {
    fetchLogs()
    if (isAutoRefresh) {
      const interval = setInterval(fetchLogs, 2000)
      return () => clearInterval(interval)
    }
  }, [fetchLogs, isAutoRefresh])

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp)
      const time = date.toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
      const ms = date.getMilliseconds().toString().padStart(3, '0')
      return `${time}.${ms}`
    } catch {
      return timestamp
    }
  }

  const getLevelColor = (level: string) => {
    const colors: Record<string, string> = {
      DEBUG: 'text-gray-400',
      INFO: 'text-green-400',
      WARN: 'text-yellow-400',
      ERROR: 'text-red-400',
    }
    return colors[level] || 'text-white'
  }

  const getSourceColor = (source: string) => {
    const colors: Record<string, string> = {
      KERNEL: 'text-blue-400',
      WASM: 'text-purple-400',
      DEV_SERVER: 'text-pink-400',
      FS: 'text-orange-400',
      SYSCALL: 'text-cyan-400',
    }
    return colors[source] || 'text-white'
  }

  const getSourceBorderColor = (source: string) => {
    const colors: Record<string, string> = {
      KERNEL: 'border-l-blue-500',
      WASM: 'border-l-purple-500',
      DEV_SERVER: 'border-l-pink-500',
      FS: 'border-l-orange-500',
      SYSCALL: 'border-l-cyan-500',
    }
    return colors[source] || 'border-l-green-500'
  }

  const exportLogs = (format: 'json' | 'csv' | 'txt') => {
    let content = ''
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
    const filename = `logs-${timestamp}.${format}`

    if (format === 'json') {
      content = JSON.stringify(filteredLogs, null, 2)
    } else if (format === 'csv') {
      content = 'timestamp,level,source,pid,message\n'
      filteredLogs.forEach(log => {
        const message = (log.message || '').replace(/"/g, '""')
        content += `"${log.timestamp}","${log.level}","${log.source}","${log.pid || ''}","${message}"\n`
      })
    } else {
      content = filteredLogs
        .map(
          log =>
            `[${log.timestamp}] [${log.level}] [${log.source}${log.pid ? `:${log.pid}` : ''}] ${log.message}`
        )
        .join('\n')
    }

    const blob = new Blob([content], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-6">
        <h2 className="text-2xl font-bold mb-2 text-green-400">📋 Application Logs</h2>
        <p className="text-white/80">Real-time log streaming from WASM VM and wasmrun</p>
      </div>

      {/* Controls */}
      <div className="border-b border-green-500/20 bg-black/10 backdrop-blur-lg p-4 flex flex-wrap gap-4 items-center">
        <div className="flex gap-2">
          <label className="text-sm text-white/80">Level:</label>
          <select
            value={levelFilter}
            onChange={e => setLevelFilter((e.target as HTMLSelectElement).value)}
            className="bg-black/30 border border-green-500/30 rounded px-3 py-1 text-sm text-white hover:bg-green-500/20"
          >
            <option value="all">All</option>
            <option value="DEBUG">Debug</option>
            <option value="INFO">Info</option>
            <option value="WARN">Warn</option>
            <option value="ERROR">Error</option>
          </select>
        </div>

        <div className="flex gap-2">
          <label className="text-sm text-white/80">Source:</label>
          <select
            value={sourceFilter}
            onChange={e => setSourceFilter((e.target as HTMLSelectElement).value)}
            className="bg-black/30 border border-green-500/30 rounded px-3 py-1 text-sm text-white hover:bg-green-500/20"
          >
            <option value="all">All</option>
            <option value="KERNEL">Kernel</option>
            <option value="WASM">WASM</option>
            <option value="DEV_SERVER">Dev Server</option>
            <option value="FS">Filesystem</option>
            <option value="SYSCALL">Syscall</option>
          </select>
        </div>

        <button
          onClick={() => fetchLogs()}
          className="px-3 py-1 bg-green-600/30 hover:bg-green-600/50 border border-green-500/30 rounded text-sm text-white transition-colors"
        >
          🔄 Refresh
        </button>

        <label className="flex items-center gap-2 text-sm text-white/80 cursor-pointer ml-auto">
          <input
            type="checkbox"
            checked={isAutoRefresh}
            onChange={e => setIsAutoRefresh((e.target as HTMLInputElement).checked)}
            className="w-4 h-4"
          />
          Auto-refresh
        </label>

        <div className="flex gap-1">
          <button
            onClick={() => exportLogs('json')}
            className="px-2 py-1 bg-blue-600/30 hover:bg-blue-600/50 border border-blue-500/30 rounded text-xs text-white transition-colors"
          >
            JSON
          </button>
          <button
            onClick={() => exportLogs('csv')}
            className="px-2 py-1 bg-blue-600/30 hover:bg-blue-600/50 border border-blue-500/30 rounded text-xs text-white transition-colors"
          >
            CSV
          </button>
          <button
            onClick={() => exportLogs('txt')}
            className="px-2 py-1 bg-blue-600/30 hover:bg-blue-600/50 border border-blue-500/30 rounded text-xs text-white transition-colors"
          >
            TXT
          </button>
        </div>
      </div>

      {/* Stats */}
      <div className="bg-black/10 backdrop-blur-lg px-6 py-3 border-b border-green-500/20 flex gap-6 text-sm">
        <div className="text-white/70">
          Total: <span className="text-green-400 font-bold">{logs.length}</span>
        </div>
        <div className="text-white/70">
          Filtered: <span className="text-green-400 font-bold">{filteredLogs.length}</span>
        </div>
        <div className="text-white/70 ml-auto">
          Last updated:{' '}
          <span className="text-green-400 font-mono text-xs">{lastUpdateTime || '-'}</span>
        </div>
      </div>

      {/* Logs View */}
      <div className="flex-1 overflow-y-auto p-4 space-y-1">
        {filteredLogs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-white/50">
            <div className="text-center">
              <div className="text-4xl mb-2">🤐</div>
              <div>No logs match the current filters</div>
            </div>
          </div>
        ) : (
          filteredLogs.map((log, idx) => (
            <div
              key={idx}
              className={`flex gap-3 p-2 text-sm border-l-4 ${getSourceBorderColor(log.source)} hover:bg-white/5 transition-colors`}
            >
              <div className="text-gray-500 font-mono text-xs min-w-fit flex-shrink-0">
                {formatTimestamp(log.timestamp)}
              </div>
              <div
                className={`font-bold text-xs min-w-fit flex-shrink-0 ${getLevelColor(log.level)}`}
              >
                [{log.level}]
              </div>
              <div
                className={`font-bold text-xs min-w-fit flex-shrink-0 ${getSourceColor(log.source)}`}
              >
                [{log.source}]
              </div>
              {log.pid && (
                <div className="text-gray-500 text-xs min-w-fit flex-shrink-0">PID:{log.pid}</div>
              )}
              <div className="text-white/90 font-mono text-xs break-words flex-1">
                {log.message}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}
