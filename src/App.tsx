import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { ServiceSummary } from './types'
import { NamespaceSelector } from './components/NamespaceSelector'
import { ServiceList } from './components/ServiceList'
import './App.css'

export default function App() {
  const [namespaces, setNamespaces] = useState<string[]>([])
  const [selectedNs, setSelectedNs] = useState<string>('')
  const [services, setServices] = useState<ServiceSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const win = getCurrentWindow()
    let timer: ReturnType<typeof setTimeout> | null = null

    const unlistenPromise = win.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        timer = setTimeout(() => {
          if (!document.hasFocus()) {
            win.hide()
          }
        }, 150)
      } else {
        if (timer !== null) {
          clearTimeout(timer)
          timer = null
        }
      }
    })

    return () => {
      if (timer !== null) clearTimeout(timer)
      unlistenPromise.then(unlisten => unlisten())
    }
  }, [])

  useEffect(() => {
    invoke<string[]>('list_namespaces')
      .then((ns) => {
        setNamespaces(ns)
        if (ns.length > 0) setSelectedNs(ns[0])
        else setLoading(false)
      })
      .catch((e) => {
        setError(String(e))
        setLoading(false)
      })
  }, [])

  function loadServices(ns: string) {
    setLoading(true)
    setError(null)
    invoke<ServiceSummary[]>('list_services', { namespace: ns })
      .then(setServices)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    if (!selectedNs) return
    loadServices(selectedNs)
  }, [selectedNs])

  function handleRefresh() {
    if (!selectedNs) return
    loadServices(selectedNs)
  }

  return (
    <main className="app">
      <header className="app-header">
        <h1>Knative Explorer</h1>
        <div className="header-controls">
          <NamespaceSelector
            namespaces={namespaces}
            selected={selectedNs}
            onChange={setSelectedNs}
            disabled={loading}
          />
          <button onClick={handleRefresh} disabled={!selectedNs || loading}>
            Refresh
          </button>
        </div>
      </header>

      {error && <div className="error-banner">{error}</div>}

      <ServiceList services={services} loading={loading} />
    </main>
  )
}
