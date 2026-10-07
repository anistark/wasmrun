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
  // Present when the network proxy is up. The inbox is the shared ring the
  // page writes socket events into (netInbox.ts); the worker also waits on
  // its wake word for clocks. `listenerId` is -1 when no port was bound, and
  // the program can still connect out
  net?: {
    inbox: SharedArrayBuffer
    listenerId: number
    address: string
  }
}

export type PageToVm = VmStartMessage

// Socket traffic to the page. Sent from inside a WASI import, which is fine:
// posting never waits for the page. A connect's `id` is the worker's request
// number, answered through the inbox
export type VmSocketMessage =
  | { type: 'sock'; op: 'send'; id: number; bytes: Uint8Array }
  | { type: 'sock'; op: 'close'; id: number }
  | { type: 'sock'; op: 'connect'; id: number; host: string; port: number }

export type VmToPage =
  | { type: 'stdout'; text: string }
  | { type: 'stderr'; text: string }
  | { type: 'status'; status: WasmRunnerStatus; detail?: string }
  | { type: 'exit'; code: number }
  | { type: 'error'; message: string }
  | VmSocketMessage

export function base64ToUint8Array(base64: string): Uint8Array {
  const binaryString = atob(base64)
  const bytes = new Uint8Array(binaryString.length)
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i)
  }
  return bytes
}
