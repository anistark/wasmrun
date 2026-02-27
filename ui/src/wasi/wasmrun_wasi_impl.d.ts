export interface WasiOptions {
  args?: string[]
  env?: Record<string, string>
  preopens?: Record<string, string>
  stdout?: (text: string) => void
  stderr?: (text: string) => void
  stdin?: () => string | null
}

export declare class WasiFS {
  mkdir(path: string): number
  writeFile(path: string, data: string | Uint8Array): number
  readFile(path: string): Uint8Array | null
  readdir(path: string): string[] | null
  open(path: string, flags?: number): { fd: number; errno: number }
  close(fd: number): number
}

export declare class WASIImplementation {
  fs: WasiFS
  memory: WebAssembly.Memory | null
  options: Required<WasiOptions>

  constructor(options?: WasiOptions)
  initialize(instance: WebAssembly.Instance): void
  getImportObject(): WebAssembly.Imports
  createVirtualFile(path: string, content: string | Uint8Array): number
  readVirtualFile(path: string): string | null
}

export declare const WASI_ERRNO: {
  ERRNO_SUCCESS: number
  ERRNO_BADF: number
  ERRNO_INVAL: number
  ERRNO_IO: number
  ERRNO_NOENT: number
  ERRNO_NOSYS: number
  FD_STDIN: number
  FD_STDOUT: number
  FD_STDERR: number
}

export declare const FILETYPE: {
  UNKNOWN: number
  BLOCK_DEVICE: number
  CHARACTER_DEVICE: number
  DIRECTORY: number
  REGULAR_FILE: number
  SOCKET_DGRAM: number
  SOCKET_STREAM: number
  SYMBOLIC_LINK: number
}
