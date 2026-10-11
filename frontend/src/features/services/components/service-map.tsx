import { Link } from '@tanstack/react-router'
import { Lock } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import { focusRing } from '@/components/ui/control-styles'
import type { Scope } from '@/lib/api/paths'
import type { Service } from '@/features/services/api'
import { mapColumns } from '@/features/services/graph'
import { ServiceStatusBadge } from './service-status-badge'

type Line = { id: string; path: string }

// services as boxes, with a line from each one to the services whose variables it reads
export function ServiceMap({ scope, services }: { scope: Scope; services: Service[] }) {
  const canvas = useRef<HTMLDivElement>(null)
  const boxes = useRef(new Map<string, HTMLElement>())
  const [lines, setLines] = useState<Line[]>([])
  const columns = mapColumns(services)

  useLayoutEffect(() => {
    const area = canvas.current
    if (!area) return
    function measure() {
      if (!area) return
      const origin = area.getBoundingClientRect()
      const next: Line[] = []
      for (const service of services) {
        for (const target of service.references ?? []) {
          const from = boxes.current.get(service.name)?.getBoundingClientRect()
          const to = boxes.current.get(target)?.getBoundingClientRect()
          if (!from || !to) continue
          const x1 = from.right - origin.left
          const y1 = from.top + from.height / 2 - origin.top
          const x2 = to.left - origin.left
          const y2 = to.top + to.height / 2 - origin.top
          const bend = (x1 + x2) / 2
          next.push({ id: `${service.name}-${target}`, path: `M${x1},${y1} C${bend},${y1} ${bend},${y2} ${x2},${y2}` })
        }
      }
      setLines(next)
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(area)
    return () => observer.disconnect()
  }, [services])

  return (
    <div className="overflow-x-auto border border-line bg-sunken">
      <div ref={canvas} className="relative flex w-max min-w-full items-center gap-20 p-8">
        <svg aria-hidden className="pointer-events-none absolute inset-0 size-full overflow-visible">
          <defs>
            <marker id="map-arrow" viewBox="0 0 6 6" refX="6" refY="3" markerWidth="6" markerHeight="6" orient="auto">
              <path d="M0,0 L6,3 L0,6 z" className="fill-line-strong" />
            </marker>
          </defs>
          {lines.map((line) => (
            <path key={line.id} d={line.path} fill="none" className="stroke-line-strong" strokeWidth={1} markerEnd="url(#map-arrow)" />
          ))}
        </svg>
        {columns.map((column, index) => (
          <ul key={index} className="relative flex flex-col gap-6">
            {column.map((service) => (
              <li
                key={service.name}
                ref={(element) => {
                  if (element) boxes.current.set(service.name, element)
                  else boxes.current.delete(service.name)
                }}
              >
                <ServiceBox scope={scope} service={service} />
              </li>
            ))}
          </ul>
        ))}
      </div>
    </div>
  )
}

function ServiceBox({ scope, service }: { scope: Scope; service: Service }) {
  return (
    <Link
      to="/projects/$project/$environment/services/$service"
      params={{ ...scope, service: service.name }}
      className={`flex w-56 flex-col gap-2 border border-line bg-surface p-3 transition-colors hover:border-line-strong ${focusRing}`}
    >
      <span className="flex items-center justify-between gap-2">
        <span className="truncate text-sm font-medium text-fg">{service.name}</span>
        <ServiceStatusBadge service={service} />
      </span>
      <span className="truncate font-mono text-2xs text-fg-faint" title={service.image}>
        {service.image}
      </span>
      <span className="flex items-center gap-1 truncate font-mono text-2xs text-fg-muted">
        {service.domains.length > 0 ? (
          service.domains[0]
        ) : (
          <>
            <Lock aria-hidden className="size-3" />
            {service.name}.internal:{service.port}
          </>
        )}
      </span>
    </Link>
  )
}
