import type { WasmRunnerStatus } from '../../os/WasmRunner'
import type { TunnelInfo } from '../../os/tunnel'

interface ApplicationPanelProps {
  projectName: string
  language: string
  wasmStatus: WasmRunnerStatus
  onRun: () => void
  onStop: () => void
  onOpenConsole: () => void
  listenAddress: string | null
  tunnel: TunnelInfo | null
}

// The project at a glance: whether the program is running in the browser VM,
// and where to reach it. The program serves on a real host port that the
// network proxy bound for it, so it opens in its own tab: under the
// cross-origin isolation the VM needs, this page cannot frame it
export default function ApplicationPanel({
  projectName,
  language,
  wasmStatus,
  onRun,
  onStop,
  onOpenConsole,
  listenAddress,
  tunnel,
}: ApplicationPanelProps) {
  const canRun = wasmStatus === 'idle' || wasmStatus === 'stopped' || wasmStatus === 'error'
  const running = wasmStatus === 'running'
  const busy = !canRun && !running
  const publicUrl = tunnel?.status === 'Connected' ? tunnel.public_url : null

  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-white/10 bg-white/5 p-6">
        <h2 className="text-2xl font-bold mb-2">{projectName}</h2>
        <p className="text-white/80">{language} project • runs in the browser VM</p>
      </div>
      <div className="flex-1 p-6 space-y-6">
        <div className="bg-black/30 backdrop-blur-lg border border-green-500/30 rounded-xl p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-green-400">Program</h3>
            <div className="flex gap-3">
              {canRun && (
                <button
                  onClick={onRun}
                  className="px-4 py-2 bg-green-600/80 hover:bg-green-600 border border-green-400/30 rounded-lg font-medium transition-all"
                >
                  ▶ Run
                </button>
              )}
              {!canRun && (
                <button
                  onClick={onStop}
                  className="px-4 py-2 bg-red-600/80 hover:bg-red-600 border border-red-400/30 rounded-lg font-medium transition-all"
                >
                  ■ Stop
                </button>
              )}
              <button
                onClick={onOpenConsole}
                className="px-4 py-2 bg-white/10 hover:bg-white/20 border border-white/20 rounded-lg font-medium transition-all"
              >
                Console
              </button>
            </div>
          </div>
          <div className="bg-black/60 border border-green-500/20 p-4 rounded-lg font-mono text-sm space-y-2">
            <div className="text-white/80">
              {running && 'Running'}
              {busy && 'Starting…'}
              {wasmStatus === 'idle' && 'Not started. Run starts it in a worker on this page.'}
              {wasmStatus === 'stopped' && 'Stopped.'}
              {wasmStatus === 'error' && 'Failed. The console has the error.'}
            </div>
            {listenAddress ? (
              <div className="text-green-400">
                Serving on{' '}
                <a
                  href={`http://${listenAddress}/`}
                  target="_blank"
                  rel="noreferrer"
                  className="underline hover:text-green-300"
                >
                  http://{listenAddress}/
                </a>
              </div>
            ) : (
              running && <div className="text-white/50">Not serving a port.</div>
            )}
            {publicUrl && (
              <div className="text-green-400">
                Public at{' '}
                <a
                  href={publicUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="underline hover:text-green-300"
                >
                  {publicUrl}
                </a>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
