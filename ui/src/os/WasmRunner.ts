import { WASIImplementation, WASI_ERRNO } from '../wasi/wasmrun_wasi_impl.js'

export type WasmRunnerStatus =
  | 'idle'
  | 'loading-runtime'
  | 'loading-files'
  | 'populating-fs'
  | 'starting'
  | 'running'
  | 'stopped'
  | 'error'

export interface WasmRunnerCallbacks {
  onStdout?: (text: string) => void
  onStderr?: (text: string) => void
  onStatusChange?: (status: WasmRunnerStatus, detail?: string) => void
  onError?: (error: Error) => void
  onExit?: (code: number) => void
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

const RUNTIME_NAMES: Record<string, string> = {
  quickjs: 'qjs',
  rustpython: 'rustpython',
  rust: 'program',
  go: 'program',
}

export class WasmRunner {
  private status: WasmRunnerStatus = 'idle'
  private callbacks: WasmRunnerCallbacks
  private wasiInstance: WASIImplementation | null = null

  constructor(callbacks: WasmRunnerCallbacks = {}) {
    this.callbacks = callbacks
  }

  getStatus(): WasmRunnerStatus {
    return this.status
  }

  getWasi(): WASIImplementation | null {
    return this.wasiInstance
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

      this.setStatus('populating-fs')
      const entryFile = this.detectEntryFile(runtimeInfo.detected_language, projectFiles.files)

      this.wasiInstance = this.createWasiInstance(runtimeLang, entryFile)
      this.populateFilesystem(projectFiles.files)

      this.setStatus('starting')
      const importObject = this.wasiInstance.getImportObject()
      const { instance } = await WebAssembly.instantiate(runtimeBytes, importObject)
      this.wasiInstance.initialize(instance)

      this.setStatus('running')
      const start = instance.exports._start as (() => void) | undefined
      if (!start) {
        throw new Error('No _start export found in WASM runtime')
      }

      start()
      this.setStatus('stopped')
      this.callbacks.onExit?.(0)
    } catch (err) {
      this.handleExecutionError(err)
    }
  }

  stop(): void {
    this.wasiInstance = null
    this.setStatus('stopped')
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

  private createWasiInstance(runtimeLang: string, entryFile: string): WASIImplementation {
    const runtimeName = RUNTIME_NAMES[runtimeLang] || runtimeLang
    const args = entryFile ? [runtimeName, entryFile] : [runtimeName]

    return new WASIImplementation({
      args,
      env: {},
      preopens: { '/': '/' },
      stdout: (text: string) => this.callbacks.onStdout?.(text),
      stderr: (text: string) => this.callbacks.onStderr?.(text),
    })
  }

  private populateFilesystem(files: Record<string, string>): void {
    if (!this.wasiInstance) return

    const dirs = new Set<string>()
    for (const relativePath of Object.keys(files)) {
      const parts = relativePath.split('/')
      for (let i = 1; i < parts.length; i++) {
        dirs.add('/' + parts.slice(0, i).join('/'))
      }
    }

    const sortedDirs = Array.from(dirs).sort()
    for (const dir of sortedDirs) {
      this.wasiInstance.fs.mkdir(dir)
    }

    for (const [relativePath, base64Content] of Object.entries(files)) {
      const bytes = base64ToUint8Array(base64Content)
      const absolutePath = '/' + relativePath
      const result = this.wasiInstance.fs.writeFile(absolutePath, bytes)
      if (result !== WASI_ERRNO.ERRNO_SUCCESS) {
        this.callbacks.onStderr?.(`Warning: failed to write ${absolutePath} to virtual FS\n`)
      }
    }
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

  private handleExecutionError(err: unknown): void {
    if (err instanceof Error) {
      const exitMatch = err.message.match(/process exited with code (\d+)/)
      if (exitMatch) {
        const code = parseInt(exitMatch[1], 10)
        this.callbacks.onExit?.(code)
        this.setStatus('stopped')
        return
      }

      this.setStatus('error')
      this.callbacks.onError?.(err)
    } else {
      this.setStatus('error')
      this.callbacks.onError?.(new Error(String(err)))
    }
  }
}

function base64ToUint8Array(base64: string): Uint8Array {
  const binaryString = atob(base64)
  const bytes = new Uint8Array(binaryString.length)
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i)
  }
  return bytes
}
