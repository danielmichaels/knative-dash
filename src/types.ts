export type ConditionStatus = 'True' | 'False' | 'Unknown'

export interface ConditionSummary {
  condition_type: string
  status: ConditionStatus
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
  instance_count: number
  conditions: ConditionSummary[]
  latest_revision: string | null
  image: string | null
  events: EventSummary[]
}

export interface PingResult {
  status_code: number
  latency_ms: number
}

export type LogEvent =
  | { kind: 'line'; text: string; isHistory: boolean }
  | { kind: 'streamStarted' }
  | { kind: 'paused' }
  | { kind: 'resumed' }
  | { kind: 'error'; message: string }
  | { kind: 'bufferOverflow'; droppedCount: number }
  | { kind: 'streamEnded' }

export interface PodInfo {
  name: string
  phase: string
}
