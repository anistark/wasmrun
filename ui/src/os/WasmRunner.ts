// Page-side half of the VM. Fetches what the worker needs, hands it over, and
// relays what comes back. The program itself runs in vm.worker.ts, which is
// what lets it block without freezing the page and lets stop() actually stop.

import VmWorker from './vm.worker?worker&inline'
import { NetBridge } from './NetBridge'
import { base64ToUint8Array } from './vm'
import type { VmStartMessage, VmToPage, WasmRunnerStatus } from './vm'

export type { WasmRunnerStatus }

export interface WasmRunnerCallbacks {
  onStdout?: (text: string) => void
  onStderr?: (text: string) => void
  onStatusChange?: (status: WasmRunnerStatus, detail?: string) => void
  onError?: (error: Error) => void
  onExit?: (code: number) => void
  onListening?: (address: string) => void
}

interface ProjectFilesResponse {
  success: boolean
  files: Record<string, string>
  file_count: number
  total_size: number
  project_path: string
  skipped: Array<{ path: string; reason: string }>
}

interface RuntimeInfoResponse {
  detected_language: string
  wasmhub_runtime: string
  cached: boolean
  cached_version?: string
}

const ENTRY_CANDIDATES: Record<string, string[]> = {
  nodejs: [
    'index.js',
    'src/index.js',
    'main.js',
    'app.js',
    'src/main.js',
    'src/app.js',
    'server.js',
    'src/server.js',
  ],
  javascript: ['index.js', 'src/index.js', 'main.js', 'app.js'],
  python: ['main.py', 'app.py', '__main__.py', 'src/main.py', 'src/app.py'],
}

// argv per runtime. The wasmhub nodejs runtime dispatches on a subcommand,
// `run <file>`, the same way agent mode invokes it (src/agent/executor.rs)
function runtimeArgs(runtimeLang: string, entryFile: string): string[] {
  switch (runtimeLang) {
    case 'nodejs':
      return entryFile ? ['nodejs-runtime', 'run', entryFile] : ['nodejs-runtime']
    case 'rust':
    case 'go':
      return ['program']
    default:
      return entryFile ? [runtimeLang, entryFile] : [runtimeLang]
  }
}

export class WasmRunner {
  private status: WasmRunnerStatus = 'idle'
  private callbacks: WasmRunnerCallbacks
  private worker: Worker | null = null
  private net: NetBridge | null = null

  constructor(callbacks: WasmRunnerCallbacks = {}) {
    this.callbacks = callbacks
  }

  getStatus(): WasmRunnerStatus {
    return this.status
  }

  async run(): Promise<void> {
    try {
      this.setStatus('loading-runtime')
      const runtimeInfo = await this.fetchRuntimeInfo()
      const runtimeLang = runtimeInfo.wasmhub_runtime

      const [runtimeBytes, projectFiles] = await Promise.all([
        this.fetchRuntime(runtimeLang).then(bytes => {
          this.setStatus('loading-files')
          return bytes
        }),
        this.fetchProjectFiles(),
      ])

      const entryFile = this.detectEntryFile(runtimeInfo.detected_language, projectFiles.files)

      // A port for the program, if the proxy is up and the policy has one.
      // Without it the program still runs; it just cannot listen
      const net = new NetBridge({
        onListening: address => this.callbacks.onListening?.(address),
        onNetworkError: message => this.callbacks.onStderr?.(`network: ${message}\n`),
      })
      this.net = (await net.open()) ? net : null

      const worker = new VmWorker()
      this.worker = worker
      worker.onmessage = (event: MessageEvent<VmToPage>) => this.onWorkerMessage(event.data)
      worker.onerror = event => this.fail(new Error(event.message || 'VM worker failed'))

      const message: VmStartMessage = {
        type: 'start',
        runtimeBytes,
        args: runtimeArgs(runtimeLang, entryFile),
        files: projectFiles.files,
        net: this.net
          ? { inbox: this.net.inbox, listenerId: this.net.listener, address: this.net.address }
          : undefined,
      }
      worker.postMessage(message, [runtimeBytes])
    } catch (err) {
      this.fail(err instanceof Error ? err : new Error(String(err)))
    }
  }

  // Terminating the worker is what makes this work mid-run: the program owns
  // its thread, so nothing cooperative could interrupt it
  stop(): void {
    if (this.worker) {
      this.worker.terminate()
      this.worker = null
    }
    this.closeNet()
    this.setStatus('stopped')
  }

  private closeNet(): void {
    this.net?.close()
    this.net = null
  }

  private onWorkerMessage(message: VmToPage): void {
    switch (message.type) {
      case 'stdout':
        this.callbacks.onStdout?.(message.text)
        break
      case 'stderr':
        this.callbacks.onStderr?.(message.text)
        break
      case 'status':
        this.setStatus(message.status, message.detail)
        break
      case 'exit':
        this.worker = null
        this.closeNet()
        this.setStatus('stopped')
        this.callbacks.onExit?.(message.code)
        break
      case 'error':
        this.worker = null
        this.closeNet()
        this.fail(new Error(message.message))
        break
      case 'sock':
        this.net?.handle(message)
        break
    }
  }

  private fail(error: Error): void {
    this.setStatus('error')
    this.callbacks.onError?.(error)
  }

  private setStatus(status: WasmRunnerStatus, detail?: string): void {
    this.status = status
    this.callbacks.onStatusChange?.(status, detail)
  }

  private async fetchRuntimeInfo(): Promise<RuntimeInfoResponse> {
    const response = await fetch('/api/runtimes')
    if (!response.ok) {
      throw new Error(`Failed to fetch runtime info: ${response.status}`)
    }
    return response.json()
  }

  private async fetchRuntime(runtimeLang: string): Promise<ArrayBuffer> {
    const response = await fetch(`/api/runtime/${runtimeLang}`)
    if (!response.ok) {
      const body = await response.text()
      throw new Error(`Failed to fetch ${runtimeLang} runtime: ${body}`)
    }
    return response.arrayBuffer()
  }

  private async fetchProjectFiles(): Promise<ProjectFilesResponse> {
    const response = await fetch('/api/project/files')
    if (!response.ok) {
      throw new Error(`Failed to fetch project files: ${response.status}`)
    }
    const data: ProjectFilesResponse = await response.json()
    if (!data.success) {
      throw new Error('Server returned unsuccessful project files response')
    }
    return data
  }

  private detectEntryFile(detectedLanguage: string, files: Record<string, string>): string {
    const paths = Object.keys(files)

    if (detectedLanguage === 'nodejs' || detectedLanguage === 'javascript') {
      const entry = this.entryFromPackageJson(files)
      if (entry) return entry
    }

    const candidates = ENTRY_CANDIDATES[detectedLanguage]
    if (candidates) {
      for (const candidate of candidates) {
        if (paths.includes(candidate)) return `/${candidate}`
      }

      const ext = detectedLanguage === 'python' ? '.py' : '.js'
      const fallback = paths.find(p => p.endsWith(ext))
      if (fallback) return `/${fallback}`
    }

    // Compiled languages (rust, go) don't need an entry file argument
    return ''
  }

  private entryFromPackageJson(files: Record<string, string>): string | null {
    const pkgBase64 = files['package.json']
    if (!pkgBase64) return null

    try {
      const pkgJson = JSON.parse(new TextDecoder().decode(base64ToUint8Array(pkgBase64)))
      const main = pkgJson.main || pkgJson.module
      if (main && typeof main === 'string') {
        const normalized = main.startsWith('./') ? main.slice(2) : main
        if (Object.keys(files).includes(normalized)) {
          return `/${normalized}`
        }
      }
    } catch {
      // Invalid package.json, fall through
    }

    return null
  }
}
