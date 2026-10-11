import { ArrowUpRight } from 'lucide-react'
import { focusRing } from '@/components/ui/control-styles'
import { Eyebrow } from '@/components/ui/eyebrow'
import type { Scope } from '@/lib/api/paths'
import { domainUrl } from '@/features/domains/api'
import type { Service } from '@/features/services/api'
import { ServiceActions } from './service-actions'
import { ServiceStatusBadge } from './service-status-badge'

export function ServiceHeader({ scope, service }: { scope: Scope; service: Service }) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        <Eyebrow>Service</Eyebrow>
        <div className="mt-2 flex flex-wrap items-center gap-3">
          <h1 className="truncate text-2xl font-semibold tracking-tight text-fg">{service.name}</h1>
          <ServiceStatusBadge service={service} />
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 font-mono text-xs text-fg-faint">
          <span className="truncate" title={service.image}>
            {service.image}
          </span>
          {service.domains.map((domain) => (
            <a
              key={domain}
              href={domainUrl(domain)}
              target="_blank"
              rel="noreferrer"
              className={`inline-flex items-center gap-0.5 text-fg-muted underline decoration-line-strong underline-offset-4 hover:text-fg hover:decoration-fg ${focusRing}`}
            >
              {domain}
              <ArrowUpRight aria-hidden className="size-3" />
              <span className="sr-only">(opens in a new tab)</span>
            </a>
          ))}
        </div>
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <ServiceActions scope={scope} service={service} />
      </div>
    </div>
  )
}
