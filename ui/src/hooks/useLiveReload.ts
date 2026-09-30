import { useEffect } from 'preact/hooks'

/** Reload the page when a watched build lands. The server marks watch mode with a meta tag */
export function useLiveReload() {
  useEffect(() => {
    if (!document.querySelector('meta[name="wasmrun-watch"]')) return

    let generation: number | null = null
    let timer: ReturnType<typeof setTimeout>

    const poll = async () => {
      try {
        const response = await fetch('/api/dev', { cache: 'no-store' })
        const { build } = await response.json()
        if (generation !== null && build.generation !== generation) {
          location.reload()
          return
        }
        generation = build.generation
        timer = setTimeout(poll, 1000)
      } catch {
        timer = setTimeout(poll, 2000)
      }
    }

    poll()
    return () => clearTimeout(timer)
  }, [])
}
