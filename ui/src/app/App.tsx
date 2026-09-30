import { ComponentChildren } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import clsx from 'clsx'
import { ThemeToggle } from '@/components/ThemeToggle'
import { LogContainer } from '@/components/LogContainer'
import { ModuleDetailsCard, ExportsCard, ImportsCard, PluginCard } from '@/components/modules'
import { useVersion } from '@/hooks/useVersion'
import { DevLogEntry, DevSnapshot, LogEntry, WasmModuleInfo } from '@/types'
import { analyzeWasmModule, fetchModuleInspection, formatBytes } from '@/utils/wasm'

// The control center: the web app runs on its own port in the frame, and this page watches it

type PanelTab = 'console' | 'requests' | 'build' | 'module'

interface AppMessage {
  source: 'wasmrun-app'
  type: 'console' | 'loaded'
  level?: 'log' | 'info' | 'warn' | 'error' | 'debug'
  message?: string
  ms?: number | null
}

const MAX_ENTRIES = 1000

function append<T>(list: T[], items: T[]): T[] {
  const next = list.concat(items)
  return next.length > MAX_ENTRIES ? next.slice(next.length - MAX_ENTRIES) : next
}

/** The app is reachable as 127.0.0.1 or localhost, and a message carries whichever the frame used */
function loopbackOrigins(url: string): string[] {
  const origin = new URL(url).origin
  return [
    origin,
    origin.replace('127.0.0.1', 'localhost'),
    origin.replace('localhost', '127.0.0.1'),
  ]
}

function consoleType(level: AppMessage['level']): LogEntry['type'] {
  if (level === 'error') return 'error'
  if (level === 'warn') return 'warning'
  return 'info'
}

function toLogEntry(entry: DevLogEntry): LogEntry {
  return { timestamp: new Date(entry.time), message: entry.message, type: entry.level }
}

function usePolledJson<T>(url: () => string, onData: (data: T) => void, onError?: () => void) {
  const target = useRef(url)
  target.current = url
  const handler = useRef(onData)
  handler.current = onData
  const errorHandler = useRef(onError)
  errorHandler.current = onError

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>
    let cancelled = false
    const poll = async () => {
      try {
        const response = await fetch(target.current(), { cache: 'no-store' })
        handler.current(await response.json())
      } catch {
        errorHandler.current?.()
      }
      if (!cancelled) timer = setTimeout(poll, 1000)
    }
    poll()
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [])
}

export function App() {
  const { version } = useVersion()
  const [dev, setDev] = useState<DevSnapshot | null>(null)
  const [disconnected, setDisconnected] = useState(false)
  const [consoleLogs, setConsoleLogs] = useState<LogEntry[]>([])
  const [requestLogs, setRequestLogs] = useState<LogEntry[]>([])
  const [buildLogs, setBuildLogs] = useState<LogEntry[]>([])
  const [loadMs, setLoadMs] = useState<number | null>(null)
  const [tab, setTab] = useState<PanelTab>('console')
  const [frameKey, setFrameKey] = useState(0)
  const [moduleInfo, setModuleInfo] = useState<WasmModuleInfo | null>(null)
  const logsSince = useRef(0)
  const generation = dev?.build.generation

  usePolledJson<DevSnapshot>(
    () => '/api/dev',
    data => {
      setDev(data)
      setDisconnected(false)
    },
    () => setDisconnected(true)
  )

  usePolledJson<{ entries: DevLogEntry[]; next: number }>(
    () => `/api/logs?since=${logsSince.current}`,
    ({ entries, next }) => {
      logsSince.current = next
      const requests = entries.filter(e => e.source === 'http').map(toLogEntry)
      const builds = entries.filter(e => e.source !== 'http').map(toLogEntry)
      if (requests.length) setRequestLogs(prev => append(prev, requests))
      if (builds.length) setBuildLogs(prev => append(prev, builds))
    }
  )

  useEffect(() => {
    if (dev?.project) document.title = `${dev.project} · Wasmrun`
  }, [dev?.project])

  useEffect(() => {
    if (!dev?.app_url) return
    const origins = loopbackOrigins(dev.app_url)
    const onMessage = (event: MessageEvent<AppMessage>) => {
      if (!origins.includes(event.origin) || event.data?.source !== 'wasmrun-app') return
      const data = event.data
      if (data.type === 'console') {
        setConsoleLogs(prev =>
          append(prev, [
            { timestamp: new Date(), message: data.message ?? '', type: consoleType(data.level) },
          ])
        )
      } else if (data.type === 'loaded') {
        setLoadMs(data.ms ?? null)
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [dev?.app_url])

  // Each build reloads the app, so mark where one run's output ends and the next begins
  useEffect(() => {
    if (generation === undefined || generation === 1) return
    setConsoleLogs(prev =>
      append(prev, [
        { timestamp: new Date(), message: `── reloaded (build ${generation}) ──`, type: 'success' },
      ])
    )
  }, [generation])

  useEffect(() => {
    if (!dev) return
    const wasmFile = dev.build.wasm_file
    let cancelled = false
    ;(async () => {
      try {
        const bytes = await (await fetch(`/${wasmFile}`, { cache: 'no-store' })).arrayBuffer()
        const analysis = analyzeWasmModule(await WebAssembly.compile(bytes))
        const inspection = await fetchModuleInspection()
        if (cancelled) return
        setModuleInfo({
          name: wasmFile,
          size: bytes.byteLength,
          imports: analysis.imports ?? [],
          exports: analysis.exports ?? [],
          isWasi: analysis.isWasi ?? false,
          plugin: inspection?.plugin,
          inspection: inspection ?? undefined,
        })
      } catch (error) {
        console.error('Module analysis failed:', error)
      }
    })()
    return () => {
      cancelled = true
    }
    // Re-analyze when a build lands, not on every poll
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [generation])

  const build = dev?.build
  const consoleErrors = consoleLogs.filter(l => l.type === 'error').length
  const tabs: { id: PanelTab; label: string; badge?: number; alert?: boolean }[] = [
    { id: 'console', label: 'Console', badge: consoleLogs.length, alert: consoleErrors > 0 },
    { id: 'requests', label: 'Requests', badge: requestLogs.length },
    { id: 'build', label: 'Build', alert: build?.status === 'failed' },
    { id: 'module', label: 'Module' },
  ]

  return (
    <div class="h-screen flex flex-col bg-light-bg dark:bg-dark-bg text-light-text dark:text-dark-text overflow-hidden">
      <header class="shrink-0 flex flex-wrap items-center gap-4 px-6 py-3 bg-light-surface dark:bg-dark-surface shadow-lg">
        <img src="/assets/logo-text.png" alt="Wasmrun" class="h-8 w-auto" />
        <div class="flex items-center gap-3 min-w-0">
          <span class="font-semibold truncate">{dev?.project ?? 'Loading...'}</span>
          <BuildPill dev={dev} disconnected={disconnected} />
        </div>
        <div class="flex-1" />
        {dev?.app_url && (
          <a
            href={dev.app_url}
            target="_blank"
            class="font-mono text-sm text-light-accent2 dark:text-dark-accent2 hover:underline"
            title="Open the app in its own tab"
          >
            {dev.app_url}
          </a>
        )}
        <button
          class="px-3 py-1.5 text-sm rounded bg-light-surface3 dark:bg-dark-surface3 hover:opacity-80"
          onClick={() => setFrameKey(k => k + 1)}
          title="Reload the app"
        >
          Reload
        </button>
        <ThemeToggle />
      </header>

      <div class="flex-1 flex flex-col lg:flex-row min-h-0">
        <section class="relative flex-1 min-h-[40vh] lg:min-h-0 bg-white">
          {dev?.app_url ? (
            <iframe
              key={frameKey}
              src={dev.app_url}
              title={dev.project}
              class="w-full h-full border-0"
            />
          ) : (
            <div class="p-8 text-light-textDim dark:text-dark-textDim">
              {disconnected ? 'The wasmrun server is not responding.' : 'Waiting for the app...'}
            </div>
          )}
          {build?.status === 'failed' && (
            <div class="absolute inset-x-0 top-0 max-h-1/2 overflow-auto p-4 bg-red-950/95 text-red-100 shadow-lg">
              <p class="font-semibold mb-2">
                Build failed. The app is still running the last good build.
              </p>
              <pre class="text-xs whitespace-pre-wrap font-mono">{build.error}</pre>
            </div>
          )}
        </section>

        <aside class="flex flex-col min-h-0 h-[45vh] lg:h-auto lg:w-[40%] lg:min-w-[380px] border-t lg:border-t-0 lg:border-l border-light-surface3 dark:border-dark-surface3 bg-light-surface dark:bg-dark-surface">
          <Metrics dev={dev} loadMs={loadMs} />
          <nav class="flex shrink-0 bg-light-surface2 dark:bg-dark-surface2 border-y border-light-surface3 dark:border-dark-surface3">
            {tabs.map(t => (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                class={clsx('px-4 py-2 text-sm font-medium flex items-center gap-2', {
                  'bg-light-surface dark:bg-dark-surface border-b-2 border-light-accent2 dark:border-dark-accent':
                    tab === t.id,
                  'text-light-textMuted dark:text-dark-textMuted hover:bg-light-surface3 dark:hover:bg-dark-surface3':
                    tab !== t.id,
                })}
              >
                {t.label}
                {t.badge ? <span class="text-xs opacity-60">{t.badge}</span> : null}
                {t.alert && <span class="w-2 h-2 rounded-full bg-red-500" />}
              </button>
            ))}
            <div class="flex-1" />
            {tab !== 'module' && (
              <button
                class="px-3 text-xs text-light-textDim dark:text-dark-textDim hover:underline"
                onClick={() => {
                  if (tab === 'console') setConsoleLogs([])
                  if (tab === 'requests') setRequestLogs([])
                  if (tab === 'build') setBuildLogs([])
                }}
              >
                Clear
              </button>
            )}
          </nav>
          <div class="flex-1 min-h-0 p-3 overflow-auto">
            {tab === 'console' && <LogContainer logs={consoleLogs} />}
            {tab === 'requests' && <LogContainer logs={requestLogs} />}
            {tab === 'build' && <LogContainer logs={buildLogs} />}
            {tab === 'module' && (
              <div class="space-y-4">
                <ModuleDetailsCard moduleInfo={moduleInfo} />
                <ExportsCard moduleInfo={moduleInfo} />
                <ImportsCard moduleInfo={moduleInfo} />
                <PluginCard moduleInfo={moduleInfo} />
              </div>
            )}
          </div>
          <footer class="shrink-0 px-3 py-1.5 text-xs text-light-textDim dark:text-dark-textDim border-t border-light-surface3 dark:border-dark-surface3">
            Wasmrun{version && ` v${version}`}
            {dev?.watch && ' · watching for changes'}
          </footer>
        </aside>
      </div>
    </div>
  )
}

function BuildPill({ dev, disconnected }: { dev: DevSnapshot | null; disconnected: boolean }) {
  if (disconnected) {
    return <Pill class="bg-red-500/15 text-red-500">server stopped</Pill>
  }
  const build = dev?.build
  if (!build) return null
  if (build.status === 'building') {
    return <Pill class="bg-amber-500/15 text-amber-500">building...</Pill>
  }
  if (build.status === 'failed') {
    return <Pill class="bg-red-500/15 text-red-500">build failed</Pill>
  }
  return (
    <Pill class="bg-green-500/15 text-green-500">
      {build.duration_ms !== null ? `built in ${build.duration_ms} ms` : 'ready'}
    </Pill>
  )
}

function Pill({ class: className, children }: { class: string; children: ComponentChildren }) {
  return (
    <span class={clsx('px-2 py-0.5 rounded-full text-xs font-medium', className)}>{children}</span>
  )
}

function Metrics({ dev, loadMs }: { dev: DevSnapshot | null; loadMs: number | null }) {
  const m = dev?.metrics
  const build = dev?.build
  const items: [string, string][] = [
    ['Requests', m ? String(m.requests) : '-'],
    ['Sent', m ? formatBytes(m.bytes_sent) : '-'],
    ['404s', m ? String(m.not_found) : '-'],
    ['Errors', m ? String(m.errors) : '-'],
    ['Page load', loadMs !== null ? `${loadMs} ms` : '-'],
    ['Builds', build ? String(build.builds) : '-'],
    ['WASM', build ? `${build.wasm_file} · ${formatBytes(build.wasm_size)}` : '-'],
  ]
  return (
    <dl class="shrink-0 grid grid-cols-3 xl:grid-cols-4 gap-x-4 gap-y-2 px-4 py-3 text-xs">
      {items.map(([label, value]) => (
        <div key={label} class={clsx('min-w-0', label === 'WASM' && 'col-span-2')}>
          <dt class="text-light-textDim dark:text-dark-textDim">{label}</dt>
          <dd class="font-mono truncate" title={value}>
            {value}
          </dd>
        </div>
      ))}
    </dl>
  )
}
