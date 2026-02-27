import { useState, useEffect, useCallback, useRef } from 'preact/hooks'
import Header from '../components/os/Header'
import Sidebar from '../components/os/Sidebar'
import ApplicationPanel from '../components/os/ApplicationPanel'
import KernelStatusPanel from '../components/os/KernelStatusPanel'
import ConsolePanel from '../components/os/ConsolePanel'
import FilesystemPanel from '../components/os/FilesystemPanel'
import LogsPanel from '../components/os/LogsPanel'
import { panels } from '../components/os/panels'
import { formatUptime, formatBytes } from '../utils/osUtils'
import { WasmRunner } from './WasmRunner'
import type { WasmRunnerStatus } from './WasmRunner'
import type {
  KernelStats,
  FilesystemStats,
  DirEntry,
  StatusType,
  ConsoleLine,
} from '../types/osTypes'

function wasmToRuntimeStatus(ws: WasmRunnerStatus): StatusType {
  switch (ws) {
    case 'running':
      return 'running'
    case 'error':
      return 'error'
    case 'stopped':
      return 'stopped'
    default:
      return 'loading'
  }
}

export default function OSMode() {
  const [activePanel, setActivePanel] = useState('project')
  const [kernelStats, setKernelStats] = useState<KernelStats | null>(null)
  const [kernelStatus, setKernelStatus] = useState<StatusType>('loading')
  const [startTime] = useState(Date.now())
  const [uptime, setUptime] = useState(0)

  // Console / WasmRunner state
  const [consoleLines, setConsoleLines] = useState<ConsoleLine[]>([])
  const [wasmStatus, setWasmStatus] = useState<WasmRunnerStatus>('idle')
  const lineIdRef = useRef(0)
  const runnerRef = useRef<WasmRunner | null>(null)

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

  const runtimeStatus = wasmToRuntimeStatus(wasmStatus)

  // --- Console helpers ---

  const addLine = useCallback((stream: ConsoleLine['stream'], text: string) => {
    const line: ConsoleLine = {
      id: lineIdRef.current++,
      stream,
      text,
      timestamp: Date.now(),
    }
    setConsoleLines(prev => [...prev, line])
  }, [])

  const clearConsole = useCallback(() => {
    setConsoleLines([])
  }, [])

  // --- WasmRunner ---

  const startWasmRunner = useCallback(() => {
    if (runnerRef.current) {
      runnerRef.current.stop()
    }

    addLine('system', 'Starting WASM runtime…')

    const runner = new WasmRunner({
      onStdout: text => addLine('stdout', text),
      onStderr: text => addLine('stderr', text),
      onStatusChange: status => {
        setWasmStatus(status)
        if (status === 'loading-runtime') addLine('system', 'Fetching runtime binary…')
        if (status === 'loading-files') addLine('system', 'Fetching project files…')
        if (status === 'populating-fs') addLine('system', 'Populating virtual filesystem…')
        if (status === 'starting') addLine('system', 'Instantiating WASM module…')
        if (status === 'running') addLine('system', 'Runtime started')
      },
      onError: error => addLine('stderr', `Error: ${error.message}`),
      onExit: code => addLine('system', `Process exited with code ${code}`),
    })

    runnerRef.current = runner
    runner.run()
  }, [addLine])

  const stopWasmRunner = useCallback(() => {
    if (runnerRef.current) {
      runnerRef.current.stop()
      runnerRef.current = null
      addLine('system', 'Runtime stopped')
    }
  }, [addLine])

  // --- Kernel stats ---

  const fetchKernelStats = useCallback(async () => {
    try {
      const response = await fetch('/api/kernel/stats')
      const stats = await response.json()
      setKernelStats(stats)
      setKernelStatus('running')
    } catch {
      setKernelStatus('error')
    }
  }, [])

  const updateUptime = useCallback(() => {
    const seconds = Math.floor((Date.now() - startTime) / 1000)
    setUptime(seconds)
  }, [startTime])

  // --- Filesystem ---

  const fetchFsStats = useCallback(async () => {
    try {
      const response = await fetch('/api/fs/stats')
      const stats = await response.json()
      setFsStats(stats)
    } catch {
      // ignore
    }
  }, [])

  const fetchDirectory = useCallback(async (path: string) => {
    try {
      const response = await fetch(`/api/fs/list${path}`)
      const data = await response.json()
      if (data.success) {
        setDirEntries(data.entries)
      }
    } catch {
      // ignore
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
    } catch {
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
    } catch {
      // ignore
    }
  }, [])

  // --- Effects ---

  useEffect(() => {
    fetchKernelStats()

    const statsInterval = setInterval(fetchKernelStats, 3000)
    const uptimeInterval = setInterval(updateUptime, 1000)

    return () => {
      clearInterval(statsInterval)
      clearInterval(uptimeInterval)
    }
  }, [fetchKernelStats, updateUptime])

  useEffect(() => {
    if (activePanel === 'filesystem') {
      fetchFsStats()
      fetchDirectory(currentPath)
    }
  }, [activePanel, currentPath, fetchFsStats, fetchDirectory])

  // --- Filesystem handlers ---

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

  // --- Render ---

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
          <ConsolePanel
            lines={consoleLines}
            wasmStatus={wasmStatus}
            runtimeStatus={runtimeStatus}
            onClear={clearConsole}
            onRun={startWasmRunner}
            onStop={stopWasmRunner}
          />
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
        🌟 Running in <strong>wasmrun OS mode</strong> — browser-based WebAssembly execution
      </div>
    </div>
  )
}
