import { useState, useEffect, useCallback } from 'preact/hooks'
import Header from '../components/os/Header'
import Sidebar from '../components/os/Sidebar'
import ApplicationPanel from '../components/os/ApplicationPanel'
import KernelStatusPanel from '../components/os/KernelStatusPanel'
import FilesystemPanel from '../components/os/FilesystemPanel'
import LogsPanel from '../components/os/LogsPanel'
import { panels } from '../components/os/panels'
import { formatUptime, formatBytes } from '../utils/osUtils'
import type { KernelStats, FilesystemStats, DirEntry, StatusType } from '../types/osTypes'

export default function OSMode() {
  const [activePanel, setActivePanel] = useState('project')
  const [kernelStats, setKernelStats] = useState<KernelStats | null>(null)
  const [kernelStatus, setKernelStatus] = useState<StatusType>('loading')
  const [runtimeStatus, setRuntimeStatus] = useState<StatusType>('loading')
  const [startTime] = useState(Date.now())
  const [uptime, setUptime] = useState(0)

  // Filesystem state
  const [fsStats, setFsStats] = useState<FilesystemStats | null>(null)
  const [dirEntries, setDirEntries] = useState<DirEntry[]>([])
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [fileContent, setFileContent] = useState<string>('')
  const [isEditing, setIsEditing] = useState(false)

  const projectName = (window as any).PROJECT_NAME || 'Unknown Project'
  const [currentPath, setCurrentPath] = useState(`/${projectName}`)
  const language = (window as any).LANGUAGE || 'unknown'
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

  const handleNavigateUp = () => {
    const newPath = currentPath.split('/').slice(0, -1).join('/') || `/${projectName}`
    setCurrentPath(newPath)
    fetchDirectory(newPath)
  }

  const handleRefresh = () => {
    fetchDirectory(currentPath)
  }

  const handleNavigate = (path: string) => {
    setCurrentPath(path)
    fetchDirectory(path)
  }

  const handleFileSelect = (path: string) => {
    readFile(path)
  }

  const handleSave = () => {
    if (selectedFile) {
      saveFile(selectedFile, fileContent)
    }
  }

  const handleCancel = () => {
    setIsEditing(false)
    if (selectedFile) {
      readFile(selectedFile)
    }
  }

  const renderPanel = () => {
    switch (activePanel) {
      case 'project':
        return (
          <ApplicationPanel
            projectName={projectName}
            language={language}
            port={port}
            kernelStats={kernelStats}
          />
        )

      case 'kernel':
        return (
          <KernelStatusPanel
            kernelStats={kernelStats}
            uptime={uptime}
            formatUptime={formatUptime}
          />
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
          <FilesystemPanel
            fsStats={fsStats}
            currentPath={currentPath}
            projectName={projectName}
            dirEntries={dirEntries}
            selectedFile={selectedFile}
            fileContent={fileContent}
            isEditing={isEditing}
            onNavigateUp={handleNavigateUp}
            onRefresh={handleRefresh}
            onNavigate={handleNavigate}
            onFileSelect={handleFileSelect}
            onEdit={() => setIsEditing(true)}
            onSave={handleSave}
            onCancel={handleCancel}
            onContentChange={setFileContent}
            formatBytes={formatBytes}
          />
        )

      case 'logs':
        return <LogsPanel />

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
      <Header
        projectName={projectName}
        kernelStatus={kernelStatus}
        runtimeStatus={runtimeStatus}
        kernelStats={kernelStats}
      />

      <div className="flex h-[calc(100vh-80px)]">
        <Sidebar panels={panels} activePanel={activePanel} onPanelChange={setActivePanel} />
        <main className="flex-1 overflow-hidden bg-black/10 backdrop-blur-sm">{renderPanel()}</main>
      </div>

      <div className="fixed bottom-4 right-4 text-xs text-white/50">
        🌟 Running in <strong>wasmrun OS mode</strong> - A browser-based WebAssembly execution
        environment
      </div>
    </div>
  )
}
