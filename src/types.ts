export interface ConditionSummary {
  condition_type: string
  status: string
  reason: string | null
  message: string | null
}

export interface EventSummary {
  reason: string
  message: string
  count: number
  event_type: string
}

export interface ServiceSummary {
  name: string
  namespace: string
  url: string | null
  ready: boolean
  conditions: ConditionSummary[]
  latest_revision: string | null
  image: string | null
  events: EventSummary[]
}

export interface PingResult {
  status_code: number
  latency_ms: number
}
