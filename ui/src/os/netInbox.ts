// Network events, page to worker, over shared memory. A message to a
// worker is only delivered when its event loop turns, and a worker running a
// guest never turns it, so the bytes have to be somewhere the guest's own
// WASI calls can look: a ring buffer in a SharedArrayBuffer. The page is the
// only writer, the worker the only reader.
//
// Layout: three Int32 header words, then the ring.
//   [0] wake     set to 1 and notified after every write; the worker waits on
//                it with a timeout in poll_oneoff and resets it before reading
//   [1] head     write offset, owned by the page
//   [2] tail     read offset, owned by the worker
// Each frame is [u32 kind][u32 id][u32 len][len bytes], padded to 4, and may
// wrap around the end of the ring.

const HEADER_BYTES = 16
const FRAME_HEADER = 12
const MAX_DATA_FRAME = 64 * 1024

export const WAKE = 0
const HEAD = 1
const TAIL = 2

const KIND_ACCEPTED = 1
const KIND_DATA = 2
const KIND_CLOSED = 3
const KIND_CONNECTED = 4
const KIND_REFUSED = 5

// `connected` and `refused` answer an outbound connect, so their id is the
// worker's request number rather than a socket id
export type InboxFrame =
  | { kind: 'accepted'; id: number; connId: number; remote: string }
  | { kind: 'data'; id: number; bytes: Uint8Array }
  | { kind: 'closed'; id: number }
  | { kind: 'connected'; id: number; connId: number }
  | { kind: 'refused'; id: number; errno: number }

export function createInbox(capacity = 1 << 20): SharedArrayBuffer {
  return new SharedArrayBuffer(HEADER_BYTES + capacity)
}

class Ring {
  protected header: Int32Array
  protected bytes: Uint8Array
  protected capacity: number

  constructor(buffer: SharedArrayBuffer) {
    this.header = new Int32Array(buffer, 0, 4)
    this.bytes = new Uint8Array(buffer, HEADER_BYTES)
    this.capacity = this.bytes.length
  }

  protected copyIn(offset: number, src: Uint8Array): number {
    const first = Math.min(src.length, this.capacity - offset)
    this.bytes.set(src.subarray(0, first), offset)
    if (first < src.length) {
      this.bytes.set(src.subarray(first), 0)
    }
    return (offset + src.length) % this.capacity
  }

  protected copyOut(offset: number, length: number): Uint8Array {
    const out = new Uint8Array(length)
    const first = Math.min(length, this.capacity - offset)
    out.set(this.bytes.subarray(offset, offset + first))
    if (first < length) {
      out.set(this.bytes.subarray(0, length - first), first)
    }
    return out
  }
}

export class InboxWriter extends Ring {
  private pending: Uint8Array[] = []
  private retry: ReturnType<typeof setTimeout> | null = null

  accepted(id: number, connId: number, remote: string): void {
    const remoteBytes = new TextEncoder().encode(remote)
    const payload = new Uint8Array(4 + remoteBytes.length)
    new DataView(payload.buffer).setUint32(0, connId, true)
    payload.set(remoteBytes, 4)
    this.push(KIND_ACCEPTED, id, payload)
  }

  // Chunked, so no single frame can be larger than the ring and stall it
  data(id: number, bytes: Uint8Array): void {
    for (let at = 0; at < bytes.length; at += MAX_DATA_FRAME) {
      this.push(KIND_DATA, id, bytes.subarray(at, at + MAX_DATA_FRAME))
    }
  }

  closed(id: number): void {
    this.push(KIND_CLOSED, id, new Uint8Array(0))
  }

  connected(request: number, connId: number): void {
    this.push(KIND_CONNECTED, request, u32(connId))
  }

  refused(request: number, errno: number): void {
    this.push(KIND_REFUSED, request, u32(errno))
  }

  private push(kind: number, id: number, payload: Uint8Array): void {
    const padded = (payload.length + 3) & ~3
    const frame = new Uint8Array(FRAME_HEADER + padded)
    const view = new DataView(frame.buffer)
    view.setUint32(0, kind, true)
    view.setUint32(4, id, true)
    view.setUint32(8, payload.length, true)
    frame.set(payload, FRAME_HEADER)
    this.pending.push(frame)
    this.flush()
  }

  // Frames go in whole and in order. When the ring is full the rest waits
  // for the worker to drain some; nothing here can block on it
  private flush(): void {
    while (this.pending.length > 0) {
      const frame = this.pending[0]
      const head = Atomics.load(this.header, HEAD)
      const tail = Atomics.load(this.header, TAIL)
      const free = (tail - head - 1 + this.capacity) % this.capacity
      if (frame.length > free) break
      const next = this.copyIn(head, frame)
      Atomics.store(this.header, HEAD, next)
      this.pending.shift()
    }
    Atomics.store(this.header, WAKE, 1)
    Atomics.notify(this.header, WAKE)
    if (this.pending.length > 0 && this.retry === null) {
      this.retry = setTimeout(() => {
        this.retry = null
        this.flush()
      }, 5)
    }
  }
}

export class InboxReader extends Ring {
  // Everything written so far. Resets the wake flag first, so a write that
  // lands during the drain sets it again and the next wait returns at once
  drain(): InboxFrame[] {
    Atomics.store(this.header, WAKE, 0)
    const frames: InboxFrame[] = []
    let tail = Atomics.load(this.header, TAIL)
    const head = Atomics.load(this.header, HEAD)
    while (tail !== head) {
      const header = new DataView(this.copyOut(tail, FRAME_HEADER).buffer)
      const kind = header.getUint32(0, true)
      const id = header.getUint32(4, true)
      const length = header.getUint32(8, true)
      const payloadAt = (tail + FRAME_HEADER) % this.capacity
      const payload = this.copyOut(payloadAt, length)
      tail = (payloadAt + ((length + 3) & ~3)) % this.capacity
      frames.push(decode(kind, id, payload))
    }
    Atomics.store(this.header, TAIL, tail)
    return frames
  }

  // Block until something is written or `ms` passes. Only valid in a worker
  wait(ms: number): void {
    Atomics.wait(this.header, WAKE, 0, ms)
  }
}

function decode(kind: number, id: number, payload: Uint8Array): InboxFrame {
  switch (kind) {
    case KIND_ACCEPTED: {
      const connId = new DataView(payload.buffer, payload.byteOffset).getUint32(0, true)
      const remote = new TextDecoder().decode(payload.subarray(4))
      return { kind: 'accepted', id, connId, remote }
    }
    case KIND_DATA:
      return { kind: 'data', id, bytes: payload }
    case KIND_CONNECTED:
      return { kind: 'connected', id, connId: readU32(payload) }
    case KIND_REFUSED:
      return { kind: 'refused', id, errno: readU32(payload) }
    default:
      return { kind: 'closed', id }
  }
}

function u32(value: number): Uint8Array {
  const bytes = new Uint8Array(4)
  new DataView(bytes.buffer).setUint32(0, value, true)
  return bytes
}

function readU32(payload: Uint8Array): number {
  return new DataView(payload.buffer, payload.byteOffset).getUint32(0, true)
}
