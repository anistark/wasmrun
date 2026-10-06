import { LogEntry, WasmModuleInfo, WasmInspectionInfo } from '@/types'

export function log(message: string, type: LogEntry['type'] = 'info'): LogEntry {
  const entry: LogEntry = {
    timestamp: new Date(),
    message,
    type,
  }

  // Log to browser console for debugging
  console.log(`[${entry.timestamp.toLocaleTimeString()}] ${message}`)

  return entry
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 Bytes'

  const k = 1024
  const sizes = ['Bytes', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))

  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

export function detectWasiModule(wasmBytes: ArrayBuffer): boolean {
  // Skip analysis for large files
  if (wasmBytes.byteLength > 8 * 1024 * 1024) {
    return true // Assume it might be a WASI module
  }

  try {
    const module = new WebAssembly.Module(wasmBytes)
    const imports = WebAssembly.Module.imports(module)

    // Check if any import is from a WASI namespace
    return imports.some(
      imp =>
        imp.module === 'wasmrun_wasi_impl' ||
        imp.module === 'wasi_unstable' ||
        imp.module === 'wasi'
    )
  } catch (err) {
    console.error('Error detecting WASI:', err)
    // TODO: Better error handling
    return true // Assume it might be a WASI module if we can't detect
  }
}

// Compile only: the console instantiates, with imports built for the module
export async function loadWasmModule(filename: string): Promise<WebAssembly.Module> {
  const response = await fetch(filename)
  if (!response.ok) {
    throw new Error(`Failed to fetch ${filename}: ${response.status}`)
  }
  return WebAssembly.compile(await response.arrayBuffer())
}

export function analyzeWasmModule(module: WebAssembly.Module): Partial<WasmModuleInfo> {
  try {
    const imports = WebAssembly.Module.imports(module)
    const exports = WebAssembly.Module.exports(module)

    return {
      imports: imports.map(imp => `${imp.module}.${imp.name}`),
      exports: exports.map(exp => exp.name),
      isWasi: imports.some(
        imp =>
          imp.module === 'wasmrun_wasi_impl' ||
          imp.module === 'wasi_unstable' ||
          imp.module === 'wasi'
      ),
    }
  } catch (err) {
    console.error('Error analyzing WASM module:', err)
    return {}
  }
}

export async function fetchModuleInspection(): Promise<WasmInspectionInfo | null> {
  try {
    const response = await fetch('/api/module-info')

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`)
    }

    const data = await response.json()

    if (data.error) {
      console.error('Module inspection error:', data.error)
      return null
    }

    return data as WasmInspectionInfo
  } catch (error) {
    console.error('Error fetching module inspection:', error)
    return null
  }
}
