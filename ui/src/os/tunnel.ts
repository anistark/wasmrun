// The public tunnel `wasmrun os --expose` opens. The server owns the tunnel;
// the page only knows which port the program got, so it reports that

export interface TunnelInfo {
  enabled: boolean
  status: string
  server?: string
  public_url?: string | null
  public_port?: number | null
  target_port?: number | null
  error?: string | null
}

export async function fetchTunnelStatus(): Promise<TunnelInfo | null> {
  try {
    return await (await fetch('/api/tunnel/status')).json()
  } catch {
    return null
  }
}

// Point the tunnel at the program's port, or at nothing when it stops.
// Throws with the server's reason when it refuses
export async function setTunnelTarget(port: number | null): Promise<TunnelInfo> {
  const response = await fetch('/api/tunnel/target', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ port }),
  })
  const body = await response.json()
  if (!response.ok || !body.success) {
    throw new Error(body.error || `HTTP ${response.status}`)
  }
  return body
}
