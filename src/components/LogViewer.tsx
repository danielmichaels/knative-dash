import { useEffect, useRef, useState, useCallback } from 'react'
import { invoke, Channel } from '@tauri-apps/api/core'
import type { LogEvent, PodInfo } from '../types'

interface Props {
  namespace: string
  serviceName: string
  instanceCount: number
  onClose: () => void
}

interface LogLine {
  text: string
  isHistory: boolean
}

const MAX_LINES = 5000

export function LogViewer({ namespace, serviceName, instanceCount, onClose }: Props) {
  const [pods, setPods] = useState<PodInfo[]>([])
  const [selectedPod, setSelectedPod] = useState<string | null>(null)
  const [lines, setLines] = useState<LogLine[]>([])
  const [streaming, setStreaming] = useState(false)
  const [paused, setPaused] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const logRef = useRef<HTMLPreElement>(null)
  const autoScrollRef = useRef(true)
  const hasHistoryRef = useRef(false)
  const hasLiveRef = useRef(false)
  const pendingLinesRef = useRef<LogLine[]>([])
  const rafRef = useRef<number>(0)

  const flushPendingLines = useCallback(() => {
    rafRef.current = 0
    const batch = pendingLinesRef.current
    if (batch.length === 0) return
    pendingLinesRef.current = []

    for (const line of batch) {
      if (line.isHistory) hasHistoryRef.current = true
      else hasLiveRef.current = true
    }

    setLines((prev) => {
      const next = prev.concat(batch)
      return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next
    })
  }, [])

  const resetHistoryTracking = useCallback(() => {
    hasHistoryRef.current = false
    hasLiveRef.current = false
  }, [])

  const fetchPods = useCallback(async (autoStart: boolean) => {
    try {
      const result = await invoke<PodInfo[]>('list_pods', { namespace, serviceName })
      setPods(result)
      const running = result.filter((p) => p.phase === 'Running')
      if (autoStart && running.length === 1) {
        setSelectedPod(running[0].name)
        startStream(running[0].name)
      }
    } catch (e) {
      setError(String(e))
    }
  }, [namespace, serviceName])

  useEffect(() => {
    fetchPods(true)
    return () => {
      invoke('stop_log_stream').catch(() => {})
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
    }
  }, [fetchPods])

  useEffect(() => {
    if (!streaming) {
      fetchPods(!selectedPod)
    }
  }, [instanceCount])

  const handleScroll = useCallback(() => {
    const el = logRef.current
    if (!el) return
    autoScrollRef.current = el.scrollTop + el.clientHeight >= el.scrollHeight - 20
  }, [])

  useEffect(() => {
    const el = logRef.current
    if (el && autoScrollRef.current) {
      el.scrollTop = el.scrollHeight
    }
  }, [lines])

  async function startStream(podName: string) {
    setLines([])
    resetHistoryTracking()
    setError(null)
    setPaused(false)

    const channel = new Channel<LogEvent>()
    channel.onmessage = (event: LogEvent) => {
      switch (event.kind) {
        case 'line':
          pendingLinesRef.current.push({ text: event.text, isHistory: event.isHistory })
          if (!rafRef.current) {
            rafRef.current = requestAnimationFrame(flushPendingLines)
          }
          break
        case 'streamStarted':
          setStreaming(true)
          break
        case 'paused':
          setPaused(true)
          break
        case 'resumed':
          setPaused(false)
          break
        case 'error':
          setError(event.message)
          setStreaming(false)
          break
        case 'bufferOverflow':
          pendingLinesRef.current.push({
            text: `--- ${event.droppedCount} lines dropped while paused ---`,
            isHistory: false,
          })
          if (!rafRef.current) {
            rafRef.current = requestAnimationFrame(flushPendingLines)
          }
          break
        case 'streamEnded':
          setStreaming(false)
          break
      }
    }

    try {
      await invoke('stream_logs', {
        namespace,
        podName,
        tailLines: 100,
        channel,
      })
    } catch (e) {
      setError(String(e))
    }
  }

  async function handlePodChange(podName: string) {
    if (streaming) {
      await invoke('stop_log_stream')
    }
    setSelectedPod(podName)
    await startStream(podName)
  }

  async function handlePause() {
    await invoke('pause_log_stream')
  }

  async function handleResume() {
    await invoke('resume_log_stream')
  }

  async function handleClose() {
    if (streaming) {
      await invoke('stop_log_stream')
    }
    onClose()
  }

  const accentColor = streaming ? (paused ? '#ffb74d' : '#00e676') : '#666'
  const statusText = streaming ? (paused ? 'Paused' : 'Streaming') : 'Stopped'
  const hasHistorySeparator = hasHistoryRef.current && hasLiveRef.current

  return (
    <div className="log-panel">
      <div className="log-header">
        <span className="log-title">Logs</span>
        <div className="log-controls">
          {pods.length > 0 ? (
            <select
              className="log-pod-select"
              value={selectedPod ?? ''}
              onChange={(e) => handlePodChange(e.target.value)}
            >
              <option value="" disabled>
                Select pod…
              </option>
              {pods.map((p) => (
                <option key={p.name} value={p.name} disabled={p.phase !== 'Running'}>
                  {p.name} ({p.phase})
                </option>
              ))}
            </select>
          ) : (
            <span className="log-no-pods">No pods running</span>
          )}
          <span className="log-status" style={{ color: accentColor }}>
            ● {statusText}
          </span>
          {streaming && !paused && (
            <button className="log-btn" onClick={handlePause}>
              ⏸ Pause
            </button>
          )}
          {streaming && paused && (
            <button className="log-btn" onClick={handleResume}>
              ▶ Resume
            </button>
          )}
          <button className="log-btn log-close-btn" onClick={handleClose}>
            ✕
          </button>
        </div>
      </div>
      <div className="log-accent" style={{ backgroundColor: accentColor }} />
      {error && <div className="log-error">{error}</div>}
      <pre className="log-viewer" ref={logRef} onScroll={handleScroll}>
        {lines.length === 0 && !streaming && !error && 'Select a pod to view logs…'}
        {lines.map((line, i) => {
          const showSeparator =
            hasHistorySeparator && line.isHistory && i + 1 < lines.length && !lines[i + 1].isHistory
          return (
            <span key={i}>
              {line.text}
              {'\n'}
              {showSeparator && (
                <span className="log-separator">{'── live ──\n'}</span>
              )}
            </span>
          )
        })}
        {streaming && !paused && <span className="log-cursor">▌</span>}
      </pre>
    </div>
  )
}
