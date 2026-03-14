import type { PingResult as PingResultType } from '../types'

interface Props {
  result: PingResultType | null
  error: string | null
  loading: boolean
}

export function PingResult({ result, error, loading }: Props) {
  if (loading) return <span className="ping-status ping-loading">Pinging…</span>
  if (error) return <span className="ping-status ping-error" title={error}>Error</span>
  if (!result) return null

  const ok = result.status_code >= 200 && result.status_code < 300
  return (
    <span className={`ping-status ${ok ? 'ping-ok' : 'ping-warn'}`}>
      {result.status_code} · {result.latency_ms}ms
    </span>
  )
}
