import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { ServiceSummary, PingResult as PingResultType, ConditionSummary, EventSummary, ConditionStatus } from '../types'
import { PingResult } from './PingResult'

interface Props {
  service: ServiceSummary
}

const conditionClass: Record<ConditionStatus, string> = {
  True: 'cond-true',
  Unknown: 'cond-unknown',
  False: 'cond-false',
}

function ConditionRow({ c }: { c: ConditionSummary }) {
  const cls = conditionClass[c.status] ?? 'cond-false'
  return (
    <div className={`condition-row ${cls}`}>
      <span className="cond-type">{c.condition_type}</span>
      <div className="cond-body">
        <span className="cond-status">{c.status}</span>
        {c.reason && <span className="cond-reason">{c.reason}</span>}
        {c.message && <span className="cond-message">{c.message}</span>}
      </div>
    </div>
  )
}

function EventRow({ e }: { e: EventSummary }) {
  const isWarning = e.event_type === 'Warning'
  return (
    <div className={`event-row ${isWarning ? 'event-warning' : 'event-normal'}`}>
      <span className="event-reason">{e.reason}</span>
      {e.count > 1 && <span className="event-count">×{e.count}</span>}
      <span className="event-message">{e.message}</span>
    </div>
  )
}

export function ServiceCard({ service }: Props) {
  const [pingResult, setPingResult] = useState<PingResultType | null>(null)
  const [pingError, setPingError] = useState<string | null>(null)
  const [pinging, setPinging] = useState(false)
  const [openError, setOpenError] = useState<string | null>(null)
  const [expanded, setExpanded] = useState(false)
  const [logs, setLogs] = useState<string | null>(null)
  const [logsError, setLogsError] = useState<string | null>(null)
  const [loadingLogs, setLoadingLogs] = useState(false)

  async function handleLogs() {
    if (logs !== null) {
      setLogs(null)
      return
    }
    setLoadingLogs(true)
    setLogsError(null)
    try {
      const result = await invoke<string>('get_logs', {
        namespace: service.namespace,
        serviceName: service.name,
      })
      setLogs(result)
    } catch (e) {
      setLogsError(String(e))
    } finally {
      setLoadingLogs(false)
    }
  }

  async function handlePing() {
    if (!service.url) {
      setPingError('No URL')
      return
    }
    setPinging(true)
    setPingResult(null)
    setPingError(null)
    try {
      const result = await invoke<PingResultType>('ping_service', { url: service.url })
      setPingResult(result)
    } catch (e) {
      setPingError(String(e))
    } finally {
      setPinging(false)
    }
  }

  async function handleOpen() {
    if (!service.url) return
    try {
      await invoke('open_url', { url: service.url })
    } catch (e) {
      setOpenError(String(e))
    }
  }

  const hasDetails =
    service.conditions.length > 0 || service.image || service.latest_revision || service.events.length > 0

  return (
    <div className={`service-card ${service.ready ? 'card-ready' : 'card-not-ready'}`}>
      <div className="card-header">
        <span className={`ready-badge ${service.ready ? 'badge-ready' : 'badge-not-ready'}`}>
          {service.ready ? 'Ready' : 'Not Ready'}
        </span>
        <span className={`instance-badge ${service.instance_count === 0 ? 'scaled-to-zero-badge' : 'instance-count-badge'}`}>
          {service.instance_count === 0 ? 'Scaled to zero' : `${service.instance_count} instance${service.instance_count !== 1 ? 's' : ''}`}
        </span>
        <span className="service-name">{service.name}</span>
        {hasDetails && (
          <button className="expand-btn" onClick={() => setExpanded(e => !e)}>
            {expanded ? '▲' : '▼'}
          </button>
        )}
      </div>

      {service.url && <div className="card-url">{service.url}</div>}

      <div className="card-actions">
        <button onClick={handlePing} disabled={!service.url || pinging}>
          Ping
        </button>
        <button onClick={handleOpen} disabled={!service.url}>
          Open
        </button>
        <button onClick={handleLogs} disabled={loadingLogs}>
          {loadingLogs ? 'Loading…' : logs !== null ? 'Hide Logs' : 'Logs'}
        </button>
        <PingResult result={pingResult} error={pingError} loading={pinging} />
      </div>

      {openError && <div className="log-error">{openError}</div>}
      {logsError && <div className="log-error">{logsError}</div>}
      {logs !== null && (
        <pre className="log-viewer">{logs || '(no output)'}</pre>
      )}

      {expanded && (
        <div className="card-details">
          {(service.latest_revision || service.image) && (
            <div className="detail-section">
              {service.latest_revision && (
                <div className="detail-row">
                  <span className="detail-label">Revision</span>
                  <span className="detail-value mono">{service.latest_revision}</span>
                </div>
              )}
              {service.image && (
                <div className="detail-row">
                  <span className="detail-label">Image</span>
                  <span className="detail-value mono">{service.image}</span>
                </div>
              )}
            </div>
          )}

          {service.conditions.length > 0 && (
            <div className="detail-section">
              <div className="detail-heading">Conditions</div>
              {service.conditions.map(c => (
                <ConditionRow key={c.condition_type} c={c} />
              ))}
            </div>
          )}

          {service.events.length > 0 && (
            <div className="detail-section">
              <div className="detail-heading">Recent Events</div>
              {service.events.map((e, i) => (
                <EventRow key={i} e={e} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
