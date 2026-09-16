// What crosses between the OS page and the VM worker. The page fetches; the
// worker runs. Nothing here touches the DOM or the network.

export type WasmRunnerStatus =
  | 'idle'
  | 'loading-runtime'
  | 'loading-files'
  | 'populating-fs'
  | 'starting'
  | 'running'
  | 'stopped'
  | 'error'

export interface VmStartMessage {
  type: 'start'
  runtimeBytes: ArrayBuffer
  args: string[]
  files: Record<string, string>
  // One Int32 the page bumps and notifies to wake a blocked worker. Unused
  // by the page until sockets land; clocks wait on it with a timeout
  wake: SharedArrayBuffer
}

export type PageToVm = VmStartMessage

export type VmToPage =
  | { type: 'stdout'; text: string }
  | { type: 'stderr'; text: string }
  | { type: 'status'; status: WasmRunnerStatus; detail?: string }
  | { type: 'exit'; code: number }
  | { type: 'error'; message: string }

export function base64ToUint8Array(base64: string): Uint8Array {
  const binaryString = atob(base64)
  const bytes = new Uint8Array(binaryString.length)
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i)
  }
  return bytes
}
