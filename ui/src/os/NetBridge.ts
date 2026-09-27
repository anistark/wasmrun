// The page's end of OS mode networking. Talks to the wasmnet proxy over a
// WebSocket, which only a thread with a running event loop can do, and hands
// what arrives to the VM worker through the shared inbox. The worker cannot
// receive a message while a guest runs, so the WebSocket lives here.

import { WasmnetClient } from 'wasmnet'
import { InboxWriter, createInbox } from './netInbox'
import type { VmSocketMessage } from './vm'
import { setTunnelTarget } from './tunnel'
import type { TunnelInfo } from './tunnel'

interface NetworkStatus {
  enabled: boolean
  url?: string
  bind_ports?: string
  reason?: string
  tunnel?: boolean
}

export interface NetBridgeCallbacks {
  onListening?: (address: string) => void
  onNetworkError?: (message: string) => void
  onTunnel?: (tunnel: TunnelInfo) => void
}

const MAX_BIND_ATTEMPTS = 32

export class NetBridge {
  private client: WasmnetClient | null = null
  private writer: InboxWriter | null = null
  private listenerId = -1
  private tunneled = false
  readonly inbox: SharedArrayBuffer
  address = ''

  constructor(private callbacks: NetBridgeCallbacks) {
    this.inbox = createInbox()
  }

  get listener(): number {
    return this.listenerId
  }

  // Connect to the proxy and bind a port the policy allows. Returns false,
  // after reporting why, when the program will have to run without one
  async open(): Promise<boolean> {
    let status: NetworkStatus
    try {
      status = await (await fetch('/api/network/status')).json()
    } catch {
      return false
    }
    if (!status.enabled || !status.url) {
      return false
    }

    const client = new WasmnetClient(status.url, { binary: true })
    try {
      await client.ready()
    } catch (err) {
      this.callbacks.onNetworkError?.(`network proxy unreachable at ${status.url}: ${String(err)}`)
      return false
    }

    const candidates = bindCandidates(status.bind_ports || '')
    let bound: { id: number; port: number } | null = null
    let lastError = ''
    for (const port of candidates) {
      try {
        bound = await client.bind('127.0.0.1', port)
        break
      } catch (err) {
        lastError = err instanceof Error ? err.message : String(err)
      }
    }
    if (!bound) {
      client.disconnect()
      this.callbacks.onNetworkError?.(
        `no port to bind in "${status.bind_ports}" (${candidates.length} tried): ${lastError}`
      )
      return false
    }

    this.client = client
    this.listenerId = bound.id
    this.address = `127.0.0.1:${bound.port}`
    this.writer = new InboxWriter(this.inbox)

    client.listen(bound.id)
    client.onAccept(bound.id, (connId, remote) => {
      this.writer?.accepted(bound.id, connId, remote)
      client.onData(connId, bytes => this.writer?.data(connId, bytes))
      client.onClose(connId, () => this.writer?.closed(connId))
    })

    this.callbacks.onListening?.(this.address)

    if (status.tunnel) {
      this.tunneled = true
      setTunnelTarget(bound.port)
        .then(tunnel => this.callbacks.onTunnel?.(tunnel))
        .catch(err => this.callbacks.onNetworkError?.(`public tunnel: ${String(err)}`))
    }
    return true
  }

  // What the worker sent back from inside a WASI call
  handle(message: VmSocketMessage): void {
    if (!this.client) return
    if (message.op === 'send') {
      this.client.send(message.id, message.bytes)
    } else {
      this.client.close(message.id)
    }
  }

  close(): void {
    if (this.client) {
      if (this.listenerId >= 0) this.client.close(this.listenerId)
      this.client.disconnect()
      this.client = null
    }
    this.writer = null
    this.listenerId = -1
    if (this.tunneled) {
      this.tunneled = false
      setTunnelTarget(null)
        .then(tunnel => this.callbacks.onTunnel?.(tunnel))
        .catch(() => {})
    }
  }
}

// The first ports of each range in a wasmnet `bind_ports` string
// ("3000-9999", "8080,9000-9100"), in order, capped so a fully occupied range
// fails fast rather than walking thousands of ports
function bindCandidates(spec: string): number[] {
  const ports: number[] = []
  for (const part of spec.split(',')) {
    const [lo, hi] = part
      .trim()
      .split('-')
      .map(s => parseInt(s, 10))
    if (!Number.isInteger(lo)) continue
    const end = Number.isInteger(hi) ? hi : lo
    for (let p = lo; p <= end && ports.length < MAX_BIND_ATTEMPTS; p++) {
      ports.push(p)
    }
    if (ports.length >= MAX_BIND_ATTEMPTS) break
  }
  return ports
}
