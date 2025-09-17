import { useState, useEffect } from 'preact/hooks'

interface VersionInfo {
  name: string
  version: string
}

export function useVersion() {
  const [version, setVersion] = useState<string>('')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        setLoading(true)
        const response = await fetch('/api/version')
        if (!response.ok) {
          throw new Error(`Failed to fetch version: ${response.statusText}`)
        }
        const data: VersionInfo = await response.json()
        setVersion(data.version)
        setError(null)
      } catch (err) {
        console.error('Error fetching version:', err)
        setError(err instanceof Error ? err.message : 'Unknown error')
        setVersion('')
      } finally {
        setLoading(false)
      }
    }

    fetchVersion()
  }, [])

  return { version, loading, error }
}
