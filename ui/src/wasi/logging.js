class LogsPanel {
  constructor(port = 8420) {
    this.port = port
    this.logs = []
    this.maxLogs = 1000
    this.autoRefresh = true
    this.refreshInterval = 2000
    this.filterLevel = 'all'
    this.filterSource = 'all'
  }

  async fetchLogs(recent = true) {
    try {
      const endpoint = recent ? '/api/logs/recent' : '/api/logs'
      const response = await fetch(`http://localhost:${this.port}${endpoint}`)
      const data = await response.json()
      if (data.success) {
        this.logs = data.logs || []
        return this.logs
      }
    } catch (error) {
      console.error('Failed to fetch logs:', error)
    }
    return []
  }

  filterLogs(logs = null) {
    const source = logs || this.logs
    return source.filter(log => {
      const levelMatch = this.filterLevel === 'all' || log.level === this.filterLevel
      const sourceMatch =
        this.filterSource === 'all' ||
        log.source === this.filterSource ||
        log.source.startsWith(this.filterSource)
      return levelMatch && sourceMatch
    })
  }

  formatTimestamp(timestamp) {
    try {
      const date = new Date(timestamp)
      return date.toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        fractionalSecondDigits: 3,
      })
    } catch {
      return timestamp
    }
  }

  getLevelColor(level) {
    const colors = {
      DEBUG: '#888888',
      INFO: '#4ade80',
      WARN: '#facc15',
      ERROR: '#f87171',
    }
    return colors[level] || '#ffffff'
  }

  getSourceColor(source) {
    const colors = {
      KERNEL: '#60a5fa',
      WASM: '#8b5cf6',
      DEV_SERVER: '#ec4899',
      FS: '#f97316',
      SYSCALL: '#06b6d4',
      UNKNOWN: '#9ca3af',
    }
    return colors[source] || '#ffffff'
  }

  startAutoRefresh() {
    if (this.autoRefresh) {
      setInterval(() => this.fetchLogs(true), this.refreshInterval)
    }
  }

  formatLogEntry(log) {
    return {
      timestamp: this.formatTimestamp(log.timestamp),
      level: log.level,
      source: log.source,
      message: log.message,
      pid: log.pid,
      levelColor: this.getLevelColor(log.level),
      sourceColor: this.getSourceColor(log.source),
    }
  }

  async clear() {
    this.logs = []
  }

  async exportLogs(format = 'json') {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
    const filename = `logs-${timestamp}.${format}`

    let content = ''
    if (format === 'json') {
      content = JSON.stringify(this.logs, null, 2)
    } else if (format === 'csv') {
      content = 'timestamp,level,source,pid,message\n'
      this.logs.forEach(log => {
        const message = (log.message || '').replace(/"/g, '""')
        content += `"${log.timestamp}","${log.level}","${log.source}","${log.pid || ''}","${message}"\n`
      })
    } else {
      content = this.logs
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
}

if (typeof window !== 'undefined') {
  window.LogsPanel = LogsPanel
}

export default LogsPanel
