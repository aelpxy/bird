import { Link } from '@tanstack/react-router'
import { ArrowRight, Box, Building2, FolderKanban, Layers } from 'lucide-react'
import { buttonClass } from '@/components/ui/button-styles'
import { eyebrowClass } from '@/components/ui/control-styles'
import { type Project, preferredEnvironment } from '@/features/projects/api'

// shown until the first service exists, since "project" alone reads like one app to newcomers
export function StructureGuide({ project }: { project?: Project }) {
  const environment = project && preferredEnvironment(project)
  const steps = [
    { icon: Building2, label: 'Org', example: project?.org ?? 'acme', text: 'Who owns it: you, a team or a company.' },
    { icon: FolderKanban, label: 'Project', example: project?.name ?? 'shop', text: 'One product, with everything it needs to run.' },
    { icon: Layers, label: 'Environment', example: environment ?? 'production', text: 'A copy of the project, like production or staging, on its own private network.' },
    { icon: Box, label: 'Services', example: 'web · postgres · worker', text: 'One container each: your app, its database, a worker. They reach each other by name.' },
  ]
  return (
    <section aria-label="How bird is organised" className="border border-line bg-surface">
      <div className="flex flex-wrap items-end justify-between gap-4 border-b border-line px-5 py-4">
        <div>
          <p className="text-title font-semibold text-fg">How it fits together</p>
          <p className="mt-1 text-ui text-fg-muted">A project is not one app. It holds every service a product needs, side by side.</p>
        </div>
        {project && environment && (
          <Link to="/projects/$project/$environment" params={{ project: project.name, environment }} className={buttonClass('primary', 'sm')}>
            Add a service to {project.name}
            <ArrowRight aria-hidden className="size-3.5" />
          </Link>
        )}
      </div>
      <ol className="grid divide-line max-lg:divide-y lg:grid-cols-4 lg:divide-x">
        {steps.map(({ icon: Icon, label, example, text }) => (
          <li key={label} className="flex flex-col gap-2 px-5 py-4">
            <span className={`flex items-center gap-2 ${eyebrowClass}`}>
              <Icon aria-hidden className="size-3.5" />
              {label}
            </span>
            <span className="truncate font-mono text-xs text-fg">{example}</span>
            <span className="text-xs text-fg-faint">{text}</span>
          </li>
        ))}
      </ol>
    </section>
  )
}
