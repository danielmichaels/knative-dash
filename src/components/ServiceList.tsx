import type { ServiceSummary } from '../types'
import { ServiceCard } from './ServiceCard'

interface Props {
  services: ServiceSummary[]
  loading: boolean
}

export function ServiceList({ services, loading }: Props) {
  if (loading) return <p className="list-status">Loading services…</p>
  if (services.length === 0) return <p className="list-status">No services in this namespace</p>

  return (
    <div className="service-list">
      {services.map((svc) => (
        <ServiceCard key={`${svc.namespace}/${svc.name}`} service={svc} />
      ))}
    </div>
  )
}
