import type { KernelStats } from '../../types/osTypes'

interface ApplicationPanelProps {
  projectName: string
  language: string
  port: string
  kernelStats: KernelStats | null
}

export default function ApplicationPanel({
  projectName,
  language,
  port,
  kernelStats,
}: ApplicationPanelProps) {
  const handleStart = async () => {
    try {
      const response = await fetch(`http://localhost:${port}/api/kernel/start`, {
        method: 'POST',
      })
      const data = await response.json()
      if (data.success) {
        alert(`Project started with PID: ${data.pid}`)
        window.location.reload()
      } else {
        alert(`Failed to start: ${data.error}`)
      }
    } catch (error) {
      alert(`Error starting project: ${error}`)
    }
  }

  const handleRestart = async () => {
    try {
      const response = await fetch(`http://localhost:${port}/api/kernel/restart`, {
        method: 'POST',
      })
      const data = await response.json()
      if (data.success) {
        alert(`Project restarted with PID: ${data.pid}`)
        window.location.reload()
      } else {
        alert(`Failed to restart: ${data.error}`)
      }
    } catch (error) {
      alert(`Error restarting project: ${error}`)
    }
  }

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
              <button
                onClick={handleStart}
                className="px-4 py-2 bg-green-600/80 hover:bg-green-600 backdrop-blur-sm border border-green-400/30 rounded-lg font-medium transition-all"
              >
                ▶️ Start
              </button>
              <button
                onClick={handleRestart}
                className="px-4 py-2 bg-yellow-600/80 hover:bg-yellow-600 backdrop-blur-sm border border-yellow-400/30 rounded-lg font-medium transition-all"
              >
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
            src={`http://localhost:${port}/app/`}
            className="w-full h-full rounded-xl"
            title="Project Application"
          />
        </div>
      </div>
    </div>
  )
}
