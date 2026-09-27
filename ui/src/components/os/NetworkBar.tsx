import { useState } from 'preact/hooks'
import { clsx } from 'clsx'
import type { TunnelInfo } from '../../os/tunnel'

interface NetworkBarProps {
  listenAddress: string | null
  tunnel: TunnelInfo | null
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      // Clipboard needs a focused, secure context; the URL is still selectable
    }
  }
  return (
    <button
      onClick={copy}
      className="px-2 py-0.5 text-xs bg-white/10 hover:bg-white/20 border border-white/20 rounded transition-all"
    >
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
}

// Where the program can be reached: locally once it listens, and publicly
// when the server was started with --expose
export default function NetworkBar({ listenAddress, tunnel }: NetworkBarProps) {
  const exposed = tunnel?.enabled === true
  if (!listenAddress && !exposed) return null

  const localUrl = listenAddress ? `http://${listenAddress}` : null
  const status = tunnel?.status || ''
  const connected = status === 'Connected'
  const forwarding = connected && tunnel?.target_port != null

  return (
    <div className="border-b border-green-500/20 bg-black/30 px-4 py-2 flex flex-wrap items-center gap-x-6 gap-y-1 text-sm">
      <div className="flex items-center gap-2">
        <span className="text-white/50">Local</span>
        {localUrl ? (
          <a href={localUrl} target="_blank" rel="noreferrer" className="font-mono text-green-300">
            {localUrl}
          </a>
        ) : (
          <span className="text-white/40">not listening</span>
        )}
      </div>

      {exposed && (
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-white/50">Public</span>
          <span
            className={clsx('w-2 h-2 rounded-full shrink-0', {
              'bg-green-400': forwarding,
              'bg-blue-400': connected && !forwarding,
              'bg-yellow-400 animate-pulse': status === 'Connecting' || status === 'Reconnecting',
              'bg-red-400': status === 'Failed',
              'bg-gray-400': status === 'Disconnected' || status === 'Not started',
            })}
          />
          {tunnel?.public_url && connected ? (
            <>
              <a
                href={tunnel.public_url}
                target="_blank"
                rel="noreferrer"
                className="font-mono text-green-300 truncate"
              >
                {tunnel.public_url}
              </a>
              <CopyButton text={tunnel.public_url} />
              {!forwarding && <span className="text-white/40">waiting for the program</span>}
            </>
          ) : (
            <span className="text-white/60 truncate">
              {status.toLowerCase()}
              {tunnel?.server ? ` via ${tunnel.server}` : ''}
              {tunnel?.error ? `: ${tunnel.error}` : ''}
            </span>
          )}
        </div>
      )}
    </div>
  )
}
