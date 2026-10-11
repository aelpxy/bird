import { Link } from '@tanstack/react-router'
import { ArrowUpRight } from 'lucide-react'
import { timeAgo } from '@/lib/format'
import { ServiceStatusBadge } from '@/features/services/components/service-status-badge'
import type { PlacedService } from '../use-overview'

type ServiceRowsProps = { items: PlacedService[]; when: 'deployed' | 'status' }

// services from anywhere, each with where it lives; used for "needs attention" and "recent deploys"
export function ServiceRows({ items, when }: ServiceRowsProps) {
  return (
    <ul className="divide-y divide-line border border-line bg-surface">
      {items.map(({ scope, service }) => (
        <li key={`${scope.project}/${scope.environment}/${service.name}`}>
          <Link
            to="/projects/$project/$environment/services/$service"
            params={{ ...scope, service: service.name }}
            className="group flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-raised focus-visible:bg-raised focus-visible:outline-none"
          >
            <div className="min-w-0 flex-1">
              <p className="flex items-center gap-1.5 truncate text-ui font-medium text-fg">
                {service.name}
                <ArrowUpRight aria-hidden className="size-3 text-fg-faint opacity-0 transition-opacity group-hover:opacity-100" />
              </p>
              <p className="mt-0.5 truncate font-mono text-2xs text-fg-faint">
                {scope.project} / {scope.environment}
                {when === 'deployed' && ` · ${service.image}`}
              </p>
            </div>
            {when === 'deployed' && service.deployment ? (
              <span className="shrink-0 text-xs text-fg-faint">{timeAgo(service.deployment.created_at)}</span>
            ) : (
              <ServiceStatusBadge service={service} />
            )}
          </Link>
        </li>
      ))}
    </ul>
  )
}
