import { useState, useEffect, useCallback } from 'preact/hooks'
import { clsx } from 'clsx'

interface KernelStats {
  status: string
  active_processes: number
  total_memory_usage: number
  active_runtimes: string[]
  project_pid: number | null
}

interface FilesystemStats {
  total_mounts: number
  total_size: number
  open_fds: number
  mounts: Array<{
    guest_path: string
    host_path: string
    size: number
  }>
}

interface DirEntry {
  name: string
  is_dir: boolean
  is_file: boolean
  size: number
}

interface PanelType {
  id: string
  name: string
  icon: string
}

const panels: PanelType[] = [
  { id: 'project', name: 'Application', icon: '🌐' },
  { id: 'kernel', name: 'Kernel Status', icon: '⚙️' },
  { id: 'console', name: 'Console', icon: '📟' },
  { id: 'filesystem', name: 'File System', icon: '📁' },
  { id: 'processes', name: 'Processes', icon: '🔄' },
  { id: 'metrics', name: 'Metrics', icon: '📈' },
  { id: 'logs', name: 'Logs', icon: '📋' },
]

export default function OSMode() {
  const [activePanel, setActivePanel] = useState('project')
  const [kernelStats, setKernelStats] = useState<KernelStats | null>(null)
  const [kernelStatus, setKernelStatus] = useState<'loading' | 'running' | 'error'>('loading')
  const [runtimeStatus, setRuntimeStatus] = useState<'loading' | 'running' | 'error'>('loading')
  const [startTime] = useState(Date.now())
  const [uptime, setUptime] = useState(0)

  // Filesystem state
  const [fsStats, setFsStats] = useState<FilesystemStats | null>(null)
  const [currentPath, setCurrentPath] = useState('/project')
  const [dirEntries, setDirEntries] = useState<DirEntry[]>([])
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [fileContent, setFileContent] = useState<string>('')
  const [isEditing, setIsEditing] = useState(false)

  const projectName = (window as any).PROJECT_NAME || 'Unknown Project'
  const language = (window as any).LANGUAGE || 'unknown'
  // const projectPath = (window as any).PROJECT_PATH || ''
  const port = (window as any).PORT || '8420'

  const fetchKernelStats = useCallback(async () => {
    try {
      const response = await fetch('/api/kernel/stats')
      const stats = await response.json()
      setKernelStats(stats)
      setKernelStatus('running')
      if (stats.project_pid) {
        setRuntimeStatus('running')
      }
    } catch (error) {
      console.error('Failed to fetch kernel stats:', error)
      setKernelStatus('error')
    }
  }, [])

  const updateUptime = useCallback(() => {
    const seconds = Math.floor((Date.now() - startTime) / 1000)
    setUptime(seconds)
  }, [startTime])

  // Filesystem functions
  const fetchFsStats = useCallback(async () => {
    try {
      const response = await fetch('/api/fs/stats')
      const stats = await response.json()
      setFsStats(stats)
    } catch (error) {
      console.error('Failed to fetch filesystem stats:', error)
    }
  }, [])

  const fetchDirectory = useCallback(async (path: string) => {
    try {
      const response = await fetch(`/api/fs/list${path}`)
      const data = await response.json()
      if (data.success) {
        setDirEntries(data.entries)
      }
    } catch (error) {
      console.error('Failed to list directory:', error)
    }
  }, [])

  const readFile = useCallback(async (path: string) => {
    try {
      const response = await fetch(`/api/fs/read${path}`)
      const data = await response.json()
      if (data.success && data.type === 'text') {
        setFileContent(data.content)
        setSelectedFile(path)
      } else {
        setFileContent('Binary file - cannot display')
      }
    } catch (error) {
      console.error('Failed to read file:', error)
      setFileContent('Error reading file')
    }
  }, [])

  const saveFile = useCallback(async (path: string, content: string) => {
    try {
      const response = await fetch(`/api/fs/write${path}`, {
        method: 'POST',
        headers: { 'Content-Type': 'text/plain' },
        body: content,
      })
      const data = await response.json()
      if (data.success) {
        setIsEditing(false)
      }
    } catch (error) {
      console.error('Failed to save file:', error)
    }
  }, [])

  useEffect(() => {
    fetchKernelStats()
    setRuntimeStatus('running')

    const statsInterval = setInterval(fetchKernelStats, 3000)
    const uptimeInterval = setInterval(updateUptime, 1000)

    return () => {
      clearInterval(statsInterval)
      clearInterval(uptimeInterval)
    }
  }, [fetchKernelStats, updateUptime])

  // Fetch filesystem data when filesystem panel is active
  useEffect(() => {
    if (activePanel === 'filesystem') {
      fetchFsStats()
      fetchDirectory(currentPath)
    }
  }, [activePanel, currentPath, fetchFsStats, fetchDirectory])

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    const secs = seconds % 60

    if (hours > 0) {
      return `${hours}h ${minutes}m`
    } else if (minutes > 0) {
      return `${minutes}m ${secs}s`
    } else {
      return `${secs}s`
    }
  }

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i]
  }

  const StatusIndicator = ({
    status,
    label,
  }: {
    status: 'loading' | 'running' | 'error'
    label: string
  }) => (
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

  const renderPanel = () => {
    switch (activePanel) {
      case 'project':
        return (
          <div className="h-full flex flex-col">
            <div className="border-b border-white/10 bg-white/5 p-6">
              <h2 className="text-2xl font-bold mb-2">{projectName}</h2>
              <p className="text-white/80">{language} Project • OS Mode</p>
            </div>
            <div className="flex-1 p-6 space-y-6">
              <div className="bg-black/30 backdrop-blur-lg border border-green-500/30 rounded-xl p-6">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-semibold text-green-400">🏃‍♂️ Runtime Environment</h3>
                  <div className="flex gap-3">
                    <button className="px-4 py-2 bg-green-600/80 hover:bg-green-600 backdrop-blur-sm border border-green-400/30 rounded-lg font-medium transition-all">
                      ▶️ Start
                    </button>
                    <button className="px-4 py-2 bg-yellow-600/80 hover:bg-yellow-600 backdrop-blur-sm border border-yellow-400/30 rounded-lg font-medium transition-all">
                      🔄 Restart
                    </button>
                  </div>
                </div>
                <div className="bg-black/60 backdrop-blur-sm border border-green-500/20 p-4 rounded-lg font-mono text-sm">
                  <div className="text-white/70">Runtime initializing {language}...</div>
                  {kernelStats?.project_pid && (
                    <div className="text-green-400">
                      ✅ Project running with PID: {kernelStats.project_pid}
                    </div>
                  )}
                </div>
              </div>

              <div className="bg-white/5 backdrop-blur-lg border border-green-500/30 rounded-xl h-96 overflow-hidden">
                <iframe
                  src={`http://localhost:${port}/project/`}
                  className="w-full h-full rounded-xl"
                  title="Project Application"
                />
              </div>
            </div>
          </div>
        )

      case 'kernel':
        return (
          <div className="h-full flex flex-col">
            <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-6">
              <h2 className="text-2xl font-bold mb-2 text-green-400">Kernel Status</h2>
              <p className="text-white/80">WebAssembly Micro-Kernel Information</p>
            </div>
            <div className="flex-1 p-6">
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
                <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-4 hover:scale-105 transition-transform">
                  <div className="text-sm font-medium text-green-400/90 mb-2">Kernel Status</div>
                  <div className="text-2xl font-bold">{kernelStats?.status || 'Loading...'}</div>
                </div>
                <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-4 hover:scale-105 transition-transform">
                  <div className="text-sm font-medium text-green-400/90 mb-2">Active Processes</div>
                  <div className="text-2xl font-bold">{kernelStats?.active_processes || 0}</div>
                </div>
                <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-4 hover:scale-105 transition-transform">
                  <div className="text-sm font-medium text-green-400/90 mb-2">Memory Usage</div>
                  <div className="text-2xl font-bold">
                    {kernelStats?.total_memory_usage || 0} MB
                  </div>
                </div>
                <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-4 hover:scale-105 transition-transform">
                  <div className="text-sm font-medium text-green-400/90 mb-2">Uptime</div>
                  <div className="text-2xl font-bold">{formatUptime(uptime)}</div>
                </div>
              </div>

              {kernelStats?.active_runtimes && (
                <div className="bg-black/20 backdrop-blur-lg border border-green-500/30 rounded-xl p-6">
                  <h3 className="text-lg font-semibold mb-4 text-green-400">Active Runtimes</h3>
                  <div className="flex flex-wrap gap-2">
                    {kernelStats.active_runtimes.map(runtime => (
                      <span
                        key={runtime}
                        className="px-3 py-1 bg-green-500/30 border border-green-400/50 rounded-full text-sm"
                      >
                        {runtime}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        )

      case 'console':
        return (
          <div className="h-full flex flex-col">
            <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-6">
              <h2 className="text-2xl font-bold mb-2 text-green-400">Development Console</h2>
              <p className="text-white/80">Runtime logs and debugging information</p>
            </div>
            <div className="flex-1 p-6">
              <div className="bg-black/50 backdrop-blur-lg border border-green-500/30 rounded-xl p-4 h-96 overflow-y-auto font-mono text-sm">
                <div className="text-white/70">[OS] Kernel initialized</div>
                <div className="text-white/70">[{language.toUpperCase()}] Runtime loading...</div>
                {kernelStats?.project_pid && (
                  <div className="text-green-400">
                    [{language.toUpperCase()}] Runtime started successfully (PID:{' '}
                    {kernelStats.project_pid})
                  </div>
                )}
                <div className="text-green-400">[UI] OS Mode interface loaded</div>
                <div className="text-white/70">Waiting for application logs...</div>
              </div>
            </div>
          </div>
        )

      case 'filesystem':
        return (
          <div className="h-full flex flex-col">
            <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-6">
              <h2 className="text-2xl font-bold mb-2 text-green-400">WASI File System</h2>
              <p className="text-white/80">Mounted directories and file operations</p>
            </div>
            <div className="flex-1 flex">
              {/* Left sidebar - file browser */}
              <div className="w-1/3 border-r border-green-500/20 bg-black/10 p-4 overflow-y-auto">
                <div className="mb-4">
                  <div className="flex items-center gap-2 mb-2">
                    <button
                      onClick={() => {
                        const newPath = currentPath.split('/').slice(0, -1).join('/') || '/project'
                        setCurrentPath(newPath)
                        fetchDirectory(newPath)
                      }}
                      disabled={currentPath === '/project'}
                      className="px-3 py-1 bg-green-600/30 hover:bg-green-600/50 disabled:opacity-30 disabled:cursor-not-allowed border border-green-500/30 rounded text-sm"
                    >
                      ⬆️ Up
                    </button>
                    <button
                      onClick={() => fetchDirectory(currentPath)}
                      className="px-3 py-1 bg-green-600/30 hover:bg-green-600/50 border border-green-500/30 rounded text-sm"
                    >
                      🔄 Refresh
                    </button>
                  </div>
                  <div className="text-sm text-green-400 font-mono mb-2">📂 {currentPath}</div>
                </div>

                <div className="space-y-1">
                  {dirEntries.map(entry => (
                    <button
                      key={entry.name}
                      onClick={() => {
                        const fullPath = `${currentPath}/${entry.name}`
                        if (entry.is_dir) {
                          setCurrentPath(fullPath)
                          fetchDirectory(fullPath)
                        } else {
                          readFile(fullPath)
                        }
                      }}
                      className={clsx(
                        'w-full flex items-center justify-between px-3 py-2 rounded hover:bg-green-500/20 transition-colors text-left',
                        {
                          'bg-green-500/30': selectedFile === `${currentPath}/${entry.name}`,
                        }
                      )}
                    >
                      <div className="flex items-center gap-2">
                        <span>{entry.is_dir ? '📁' : '📄'}</span>
                        <span className="text-sm font-mono">{entry.name}</span>
                      </div>
                      {entry.is_file && (
                        <span className="text-xs text-white/50">{formatBytes(entry.size)}</span>
                      )}
                    </button>
                  ))}
                </div>

                {dirEntries.length === 0 && (
                  <div className="text-center text-white/50 py-8">
                    <div className="text-4xl mb-2">📂</div>
                    <div>Empty directory</div>
                  </div>
                )}
              </div>

              {/* Right panel - file viewer/editor and stats */}
              <div className="flex-1 flex flex-col">
                {/* Filesystem stats */}
                <div className="border-b border-green-500/20 bg-black/10 p-4">
                  <div className="grid grid-cols-3 gap-4">
                    <div className="bg-black/30 border border-green-500/30 rounded-lg p-3">
                      <div className="text-xs text-green-400/80 mb-1">Mounted</div>
                      <div className="text-xl font-bold">{fsStats?.total_mounts || 0}</div>
                    </div>
                    <div className="bg-black/30 border border-green-500/30 rounded-lg p-3">
                      <div className="text-xs text-green-400/80 mb-1">Total Size</div>
                      <div className="text-xl font-bold">
                        {formatBytes(fsStats?.total_size || 0)}
                      </div>
                    </div>
                    <div className="bg-black/30 border border-green-500/30 rounded-lg p-3">
                      <div className="text-xs text-green-400/80 mb-1">Open FDs</div>
                      <div className="text-xl font-bold">{fsStats?.open_fds || 0}</div>
                    </div>
                  </div>
                </div>

                {/* File content viewer/editor */}
                <div className="flex-1 p-4 overflow-hidden">
                  {selectedFile ? (
                    <div className="h-full flex flex-col">
                      <div className="flex items-center justify-between mb-3">
                        <div className="text-sm text-green-400 font-mono">{selectedFile}</div>
                        <div className="flex gap-2">
                          {isEditing ? (
                            <>
                              <button
                                onClick={() => saveFile(selectedFile, fileContent)}
                                className="px-3 py-1 bg-green-600 hover:bg-green-700 border border-green-400/50 rounded text-sm"
                              >
                                💾 Save
                              </button>
                              <button
                                onClick={() => {
                                  setIsEditing(false)
                                  readFile(selectedFile)
                                }}
                                className="px-3 py-1 bg-gray-600 hover:bg-gray-700 border border-gray-400/50 rounded text-sm"
                              >
                                ❌ Cancel
                              </button>
                            </>
                          ) : (
                            <button
                              onClick={() => setIsEditing(true)}
                              className="px-3 py-1 bg-blue-600/80 hover:bg-blue-600 border border-blue-400/50 rounded text-sm"
                            >
                              ✏️ Edit
                            </button>
                          )}
                        </div>
                      </div>
                      <div className="flex-1 bg-black/50 border border-green-500/30 rounded-lg overflow-hidden">
                        {isEditing ? (
                          <textarea
                            value={fileContent}
                            onChange={e => setFileContent(e.currentTarget.value)}
                            className="w-full h-full bg-transparent text-white font-mono text-sm p-4 resize-none focus:outline-none"
                          />
                        ) : (
                          <pre className="w-full h-full text-white font-mono text-sm p-4 overflow-auto">
                            {fileContent}
                          </pre>
                        )}
                      </div>
                    </div>
                  ) : (
                    <div className="h-full flex items-center justify-center text-white/50">
                      <div className="text-center">
                        <div className="text-6xl mb-4">📄</div>
                        <div>Select a file to view or edit</div>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        )

      default:
        return (
          <div className="h-full flex items-center justify-center">
            <div className="text-center">
              <div className="text-6xl mb-4">🚧</div>
              <h3 className="text-xl font-semibold mb-2">
                {panels.find(p => p.id === activePanel)?.name}
              </h3>
              <p className="text-white/70">This panel is under development</p>
            </div>
          </div>
        )
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-black via-gray-900 to-green-900 text-white">
      <header className="bg-black/30 backdrop-blur-xl border-b border-green-500/20 p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <img src="/assets/logo-text.png" alt="wasmrun OS" className="h-8 object-contain" />
            <div className="flex flex-col">
              <span className="text-white font-bold text-lg">OS</span>
              <span className="text-green-400/80 text-xs">{projectName}</span>
            </div>
          </div>
          <div className="flex items-center gap-4">
            <StatusIndicator
              status={kernelStatus}
              label={kernelStatus === 'running' ? 'Kernel Active' : 'Initializing Kernel...'}
            />
            <StatusIndicator
              status={runtimeStatus}
              label={
                runtimeStatus === 'running'
                  ? `Runtime Active (PID: ${kernelStats?.project_pid || 'N/A'})`
                  : 'Loading Runtime...'
              }
            />
          </div>
        </div>
      </header>

      <div className="flex h-[calc(100vh-80px)]">
        <nav className="w-80 bg-black/20 backdrop-blur-lg border-r border-green-500/20 p-6">
          <div className="space-y-8">
            <div>
              <h3 className="text-sm font-semibold text-green-400/90 mb-4 tracking-wide">
                🎯 PROJECT
              </h3>
              <div className="space-y-2">
                {panels.slice(0, 2).map(panel => (
                  <button
                    key={panel.id}
                    onClick={() => setActivePanel(panel.id)}
                    className={clsx(
                      'w-full flex items-center gap-3 px-4 py-3 rounded-lg backdrop-blur-sm transition-all duration-200',
                      {
                        'bg-green-600/30 border border-green-400/50 text-white':
                          activePanel === panel.id,
                        'bg-white/5 border border-green-500/20 text-white/80 hover:bg-green-500/20 hover:text-white hover:translate-x-1':
                          activePanel !== panel.id,
                      }
                    )}
                  >
                    <span>{panel.icon}</span>
                    <span className="font-medium">{panel.name}</span>
                  </button>
                ))}
              </div>
            </div>

            <div>
              <h3 className="text-sm font-semibold text-green-400/90 mb-4 tracking-wide">
                🔧 DEVELOPMENT
              </h3>
              <div className="space-y-2">
                {panels.slice(2, 5).map(panel => (
                  <button
                    key={panel.id}
                    onClick={() => setActivePanel(panel.id)}
                    className={clsx(
                      'w-full flex items-center gap-3 px-4 py-3 rounded-lg backdrop-blur-sm transition-all duration-200',
                      {
                        'bg-green-600/30 border border-green-400/50 text-white':
                          activePanel === panel.id,
                        'bg-white/5 border border-green-500/20 text-white/80 hover:bg-green-500/20 hover:text-white hover:translate-x-1':
                          activePanel !== panel.id,
                      }
                    )}
                  >
                    <span>{panel.icon}</span>
                    <span className="font-medium">{panel.name}</span>
                  </button>
                ))}
              </div>
            </div>

            <div>
              <h3 className="text-sm font-semibold text-green-400/90 mb-4 tracking-wide">
                📊 MONITORING
              </h3>
              <div className="space-y-2">
                {panels.slice(5).map(panel => (
                  <button
                    key={panel.id}
                    onClick={() => setActivePanel(panel.id)}
                    className={clsx(
                      'w-full flex items-center gap-3 px-4 py-3 rounded-lg backdrop-blur-sm transition-all duration-200',
                      {
                        'bg-green-600/30 border border-green-400/50 text-white':
                          activePanel === panel.id,
                        'bg-white/5 border border-green-500/20 text-white/80 hover:bg-green-500/20 hover:text-white hover:translate-x-1':
                          activePanel !== panel.id,
                      }
                    )}
                  >
                    <span>{panel.icon}</span>
                    <span className="font-medium">{panel.name}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </nav>

        <main className="flex-1 overflow-hidden bg-black/10 backdrop-blur-sm">{renderPanel()}</main>
      </div>

      <div className="fixed bottom-4 right-4 text-xs text-white/50">
        🌟 Running in <strong>wasmrun OS mode</strong> - A browser-based WebAssembly execution
        environment
      </div>
    </div>
  )
}
