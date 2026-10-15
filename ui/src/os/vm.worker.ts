// The VM. Runs off the page so a guest can block: every WASI call that waits
// does so on Atomics.wait here, while the page stays free to paint, receive
// and terminate.

import { WASIImplementation, WASI_ERRNO } from '../wasi/wasmrun_wasi_impl.js'
import type { SocketBridge } from '../wasi/wasmrun_wasi_impl.js'
import { base64ToUint8Array } from './vm'
import type { PageToVm, VmStartMessage, VmToPage } from './vm'
import { InboxReader, WAKE } from './netInbox'

const post = (message: VmToPage) => self.postMessage(message)

self.onmessage = (event: MessageEvent<PageToVm>) => {
  if (event.data.type === 'start') {
    run(event.data).catch(reportFailure)
  }
}

async function run(start: VmStartMessage): Promise<void> {
  const inbox = start.net ? new InboxReader(start.net.inbox) : null
  const wake = start.net
    ? new Int32Array(start.net.inbox, 0, 1)
    : new Int32Array(new SharedArrayBuffer(4))

  const sockets: SocketBridge | undefined = start.net
    ? {
        listenerId: start.net.listenerId,
        poll: () => inbox!.drain(),
        send: (id, bytes) => post({ type: 'sock', op: 'send', id, bytes: bytes.slice() }),
        close: id => post({ type: 'sock', op: 'close', id }),
        connect: (id, host, port) => post({ type: 'sock', op: 'connect', id, host, port }),
      }
    : undefined

  const env: Record<string, string> = {}
  const wasi = new WASIImplementation({
    args: start.args,
    env,
    preopens: { '/': '/' },
    stdout: text => post({ type: 'stdout', text }),
    stderr: text => post({ type: 'stderr', text }),
    wait: ms => {
      Atomics.wait(wake, WAKE, 0, ms)
    },
    sockets,
  })

  // The runtime finds its listener the way it does under `exec --tcplisten`
  // and agent serve: a descriptor number and address in the environment
  if (start.net && wasi.listenFd() >= 0) {
    env.WASMHUB_LISTEN_FD = String(wasi.listenFd())
    env.WASMHUB_LISTEN_ADDR = start.net.address
  }

  post({ type: 'status', status: 'populating-fs' })
  populateFilesystem(wasi, start.files)

  post({ type: 'status', status: 'starting' })
  let memory: WebAssembly.Memory | undefined
  const imports = withMemoryCap(wasi.getImportObject(), start.maxMemoryBytes, () => memory)
  const { instance } = await WebAssembly.instantiate(start.runtimeBytes, imports)
  wasi.initialize(instance)
  memory = instance.exports.memory as WebAssembly.Memory | undefined

  const startFn = instance.exports._start as (() => void) | undefined
  if (!startFn) {
    throw new Error('No _start export found in WASM runtime')
  }

  post({ type: 'status', status: 'running' })
  startFn()
  post({ type: 'exit', code: 0 })
}

// [os.limits] max_memory_mb. The runtime exports its memory, so no maximum
// can be imposed when instantiating it; every WASI call checks the size
// instead, and a program past the cap is ended there with a clear error.
// Allocation is cheap to grow and rare to do without any WASI call between
function withMemoryCap(
  imports: WebAssembly.Imports,
  maxBytes: number,
  memory: () => WebAssembly.Memory | undefined
): WebAssembly.Imports {
  if (!maxBytes) return imports
  const check = () => {
    const size = memory()?.buffer.byteLength ?? 0
    if (size > maxBytes) {
      throw new Error(
        `memory limit exceeded: ${Math.round(size / (1024 * 1024))} MB is past [os.limits] max_memory_mb = ${Math.round(maxBytes / (1024 * 1024))}`
      )
    }
  }
  const wrapped: WebAssembly.Imports = {}
  for (const [module, fns] of Object.entries(imports)) {
    const out: WebAssembly.ModuleImports = {}
    for (const [name, value] of Object.entries(fns)) {
      out[name] =
        typeof value === 'function'
          ? (...args: unknown[]) => {
              check()
              return (value as (...a: unknown[]) => unknown)(...args)
            }
          : value
    }
    wrapped[module] = out
  }
  return wrapped
}

function populateFilesystem(wasi: WASIImplementation, files: Record<string, string>): void {
  const dirs = new Set<string>()
  for (const relativePath of Object.keys(files)) {
    const parts = relativePath.split('/')
    for (let i = 1; i < parts.length; i++) {
      dirs.add('/' + parts.slice(0, i).join('/'))
    }
  }

  for (const dir of Array.from(dirs).sort()) {
    wasi.fs.mkdir(dir)
  }

  for (const [relativePath, base64Content] of Object.entries(files)) {
    const absolutePath = '/' + relativePath
    const result = wasi.fs.writeFile(absolutePath, base64ToUint8Array(base64Content))
    if (result !== WASI_ERRNO.ERRNO_SUCCESS) {
      post({ type: 'stderr', text: `Warning: failed to write ${absolutePath} to virtual FS\n` })
    }
  }
}

function reportFailure(err: unknown): void {
  const message = err instanceof Error ? err.message : String(err)
  const exitMatch = message.match(/process exited with code (\d+)/)
  if (exitMatch) {
    post({ type: 'exit', code: parseInt(exitMatch[1], 10) })
  } else {
    post({ type: 'error', message })
  }
}
