import { Link } from '@tanstack/react-router'
import { ArrowUpRight, Lock } from 'lucide-react'
import { Skeleton } from '@/components/ui/skeleton'
import { Table } from '@/components/ui/table'
import { tableRowClass } from '@/components/ui/table-styles'
import type { Scope } from '@/lib/api/paths'
import { timeAgo } from '@/lib/format'
import type { Service } from '@/features/services/api'
import { runningMachines } from '@/features/services/status'
import { ServiceStatusBadge } from './service-status-badge'

const LAYOUT = 'sm:grid-cols-[minmax(0,1.3fr)_7.5rem_minmax(0,1.5fr)_4.5rem_6.5rem]'
const COLUMNS = [
  { label: 'Service' },
  { label: 'Status' },
  { label: 'Image' },
  { label: 'Machines' },
  { label: 'Deployed', className: 'text-right' },
]

export function ServicesList({ scope, services }: { scope: Scope; services: Service[] }) {
  return (
    <Table label="Services" columns={COLUMNS} layout={LAYOUT}>
      {services.map((service) => (
        <ServiceRow key={service.name} scope={scope} service={service} />
      ))}
    </Table>
  )
}

function ServiceRow({ scope, service }: { scope: Scope; service: Service }) {
  const machines = service.deployment?.machines ?? []
  const deployedAt = service.deployment?.created_at
  return (
    <Link
      to="/projects/$project/$environment/services/$service"
      params={{ ...scope, service: service.name }}
      role="row"
      className={`${tableRowClass(LAYOUT)} group py-3 transition-colors hover:bg-raised focus-visible:bg-raised focus-visible:outline-none`}
    >
      <div role="cell" className="min-w-0">
        <p className="flex items-center gap-1.5 truncate text-sm font-medium text-fg">
          {service.name}
          <ArrowUpRight aria-hidden className="size-3.5 text-fg-faint opacity-0 transition-opacity group-hover:opacity-100" />
        </p>
        <p className="mt-0.5 flex items-center gap-1 truncate font-mono text-2xs text-fg-faint">
          {service.domains.length > 0 ? (
            <>
              <span className="truncate">{service.domains[0]}</span>
              {service.domains.length > 1 && <span>+{service.domains.length - 1}</span>}
            </>
          ) : (
            <>
              <Lock aria-hidden className="size-3" />
              private · :{service.port}
            </>
          )}
        </p>
      </div>
      <div role="cell">
        <ServiceStatusBadge service={service} />
      </div>
      <p role="cell" className="hidden truncate font-mono text-xs text-fg-muted sm:block" title={service.image}>
        {service.image}
      </p>
      <p role="cell" className="tabular hidden font-mono text-xs text-fg-muted sm:block">
        {runningMachines(machines)}
        <span className="text-fg-faint">/{service.replicas}</span>
      </p>
      <p role="cell" className="hidden text-right text-xs text-fg-faint sm:block">
        {deployedAt ? timeAgo(deployedAt) : '—'}
      </p>
    </Link>
  )
}

export function ServicesListSkeleton() {
  return (
    <div className="border border-line bg-surface" aria-busy aria-label="Loading services">
      <div className="h-8 border-b border-line" />
      {Array.from({ length: 3 }, (_, index) => (
        <div key={index} className="flex items-center gap-6 border-b border-line px-4 py-4 last:border-b-0">
          <Skeleton className="h-3.5 w-32" />
          <Skeleton className="h-4 w-20" />
          <Skeleton className="hidden h-3 w-48 sm:block" />
        </div>
      ))}
    </div>
  )
}
