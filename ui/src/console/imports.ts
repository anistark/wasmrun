// Imports for a module the console loads on its own, with no JS glue beside
// it. Every function import gets something callable, so a module instantiates
// whatever toolchain built it: the known AssemblyScript and wasm-bindgen
// imports do what their glue would, and anything else logs that it was called

type Log = (message: string, type?: 'info' | 'warning' | 'error') => void

// An AssemblyScript module imports env.abort, which its runtime calls on a
// failed assertion or an out-of-bounds access
export function isAssemblyScript(module: WebAssembly.Module): boolean {
  return WebAssembly.Module.imports(module).some(i => i.module === 'env' && i.name === 'abort')
}

// An AssemblyScript string: UTF-16LE, its byte length in the u32 before it
export function readAsString(memory: WebAssembly.Memory, ptr: number): string | null {
  if (ptr < 4 || ptr + 4 > memory.buffer.byteLength) return null
  const bytes = new DataView(memory.buffer).getUint32(ptr - 4, true)
  if (bytes % 2 !== 0 || ptr + bytes > memory.buffer.byteLength) return null
  return new TextDecoder('utf-16le').decode(new Uint8Array(memory.buffer, ptr, bytes))
}

export function buildImports(
  module: WebAssembly.Module,
  memory: () => WebAssembly.Memory | undefined,
  log: Log
): WebAssembly.Imports {
  const assemblyScript = isAssemblyScript(module)
  const asString = (ptr: number) => {
    const mem = memory()
    return (mem && readAsString(mem, ptr)) ?? String(ptr)
  }

  const imports: Record<string, Record<string, (...args: number[]) => number | void>> = {}
  for (const { module: ns, name, kind } of WebAssembly.Module.imports(module)) {
    if (kind !== 'function') continue
    imports[ns] ??= {}
    imports[ns][name] = handlerFor(ns, name)
  }
  return imports

  function handlerFor(ns: string, name: string): (...args: number[]) => number | void {
    if (assemblyScript && ns === 'env') {
      if (name === 'abort') {
        return (message, file, line, column) => {
          throw new Error(`abort: ${asString(message)} at ${asString(file)}:${line}:${column}`)
        }
      }
      if (name === 'trace') {
        return (message, count, ...values) =>
          log(`trace: ${asString(message)} ${values.slice(0, count).join(', ')}`.trim())
      }
      if (name === 'seed') {
        return () => Date.now()
      }
    }

    // wasm-bindgen's console.log shim: (ptr, len) of UTF-8
    if (ns === 'wbg' && name.startsWith('__wbg_log_')) {
      return (ptr, len) => {
        const mem = memory()
        log(mem ? new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len)) : `${ptr}, ${len}`)
      }
    }
    if (ns === 'wbg' && name === '__wbindgen_init_externref_table') {
      return () => {}
    }

    // A logging import an AssemblyScript module declared itself, like
    // asc-hello's consoleLog(ptr): its one argument is a string
    if (assemblyScript && /log|print/i.test(name)) {
      return (ptr, ...rest) => log(rest.length === 0 ? asString(ptr) : [ptr, ...rest].join(', '))
    }

    return (...args) => {
      log(`${ns}.${name}(${args.join(', ')}) called; the console has no implementation`, 'warning')
      return 0
    }
  }
}
