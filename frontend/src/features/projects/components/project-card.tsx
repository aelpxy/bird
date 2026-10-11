import { Link } from '@tanstack/react-router'
import { ArrowUpRight } from 'lucide-react'
import { focusRing } from '@/components/ui/control-styles'
import { plural } from '@/lib/format'
import { serviceStatus } from '@/features/services/status'
import type { PlacedService } from '@/features/overview/use-overview'
import { type Project, preferredEnvironment } from '../api'

const dotTones = { ok: 'bg-ok', warn: 'bg-warn', bad: 'bg-bad', info: 'bg-info', neutral: 'bg-fg-faint' } as const

// a project with its environments, each showing one square per service colored by its state
export function ProjectCard({ project, services }: { project: Project; services: PlacedService[] }) {
  const environment = preferredEnvironment(project)
  const className = `pressable group relative flex h-full flex-col border border-line bg-surface p-4 hover:border-line-strong hover:bg-raised ${focusRing}`
  const content = (
    <>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-title font-semibold tracking-[-0.01em] text-fg">{project.name}</p>
          <p className="mt-0.5 text-xs text-fg-faint">
            {plural(services.length, 'service')} in {plural(project.environments.length, 'environment')}
          </p>
        </div>
        <ArrowUpRight aria-hidden className="size-4 shrink-0 text-fg-faint transition-colors group-hover:text-fg" />
      </div>
      <ul className="mt-5 flex flex-col gap-2">
        {project.environments.map((name) => {
          const here = services.filter(({ scope }) => scope.environment === name)
          return (
            <li key={name} className="flex items-center justify-between gap-3">
              <span className="font-mono text-2xs text-fg-muted">{name}</span>
              <span className="flex items-center gap-2">
                <span className="flex flex-wrap justify-end gap-0.5" aria-label={`${plural(here.length, 'service')} in ${name}`}>
                  {here.map(({ service }) => (
                    <span key={service.name} title={`${service.name}: ${serviceStatus(service).label}`} className={`size-2 ${dotTones[serviceStatus(service).tone]}`} />
                  ))}
                </span>
                <span className="tabular text-right font-mono text-2xs text-fg-faint">{here.length === 0 ? 'no services yet' : plural(here.length, 'service')}</span>
              </span>
            </li>
          )
        })}
        {project.environments.length === 0 && <li className="text-xs text-fg-faint">No environments</li>}
      </ul>
    </>
  )
  return environment ? (
    <Link to="/projects/$project/$environment" params={{ project: project.name, environment }} className={className}>
      {content}
    </Link>
  ) : (
    <Link to="/projects/$project" params={{ project: project.name }} className={className}>
      {content}
    </Link>
  )
}
