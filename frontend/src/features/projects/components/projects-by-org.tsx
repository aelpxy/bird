import { Link } from '@tanstack/react-router'
import { Building2 } from 'lucide-react'
import { focusRing } from '@/components/ui/control-styles'
import { plural } from '@/lib/format'
import type { PlacedService } from '@/features/overview/use-overview'
import type { Project } from '../api'
import { ProjectCard } from './project-card'

// projects under the org that owns them, so it is clear whose each one is
export function ProjectsByOrg({ projects, services }: { projects: Project[]; services: PlacedService[] }) {
  const orgs = [...new Set(projects.map((project) => project.org))].sort()
  return (
    <div className="flex flex-col gap-6">
      {orgs.map((org) => {
        const owned = projects.filter((project) => project.org === org)
        return (
          <section key={org} aria-label={`Projects in ${org}`} className="flex flex-col gap-3">
            <div className="flex items-center gap-2">
              <Building2 aria-hidden className="size-3.5 text-fg-faint" />
              <Link to="/orgs/$org" params={{ org }} className={`text-sm font-semibold text-fg hover:underline ${focusRing}`}>
                {org}
              </Link>
              <span className="font-mono text-2xs text-fg-faint">{plural(owned.length, 'project')}</span>
            </div>
            <ProjectGrid projects={owned} services={services} />
          </section>
        )
      })}
    </div>
  )
}

export function ProjectGrid({ projects, services }: { projects: Project[]; services: PlacedService[] }) {
  return (
    <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {projects.map((project) => (
        <li key={project.name}>
          <ProjectCard project={project} services={services.filter(({ scope }) => scope.project === project.name)} />
        </li>
      ))}
    </ul>
  )
}
