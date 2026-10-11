import { createFileRoute } from '@tanstack/react-router'
import { CircleCheck, FolderKanban, History } from 'lucide-react'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { PageHeader } from '@/components/ui/page-header'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { ServiceRows } from '@/features/overview/components/service-rows'
import { StatTiles } from '@/features/overview/components/stat-tiles'
import { useOverview } from '@/features/overview/use-overview'
import { NewProjectDialog } from '@/features/projects/components/new-project-dialog'
import { ProjectCard } from '@/features/projects/components/project-card'

export const Route = createFileRoute('/_app/')({
  head: () => ({ meta: [{ title: 'Overview · bird' }] }),
  component: Overview,
})

function greeting(): string {
  const hour = new Date().getHours()
  return hour < 5 ? 'Up late' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening'
}

function Overview() {
  const { session } = Route.useRouteContext()
  const overview = useOverview()

  if (overview.loading && overview.projects.length === 0) {
    return (
      <div className="flex flex-col gap-8" aria-busy aria-label="Loading the overview">
        <Skeleton className="h-16 w-72" />
        <Skeleton className="h-28 w-full" />
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 3 }, (_, index) => (
            <Skeleton key={index} className="h-36" />
          ))}
        </div>
      </div>
    )
  }
  if (overview.error && overview.projects.length === 0) return <ErrorState error={overview.error} />

  return (
    <div className="flex flex-col gap-10">
      <PageHeader eyebrow="Overview" title={`${greeting()}, ${session.name}`} description="Everything this server runs, at a glance." actions={<NewProjectDialog />} />

      {overview.projects.length === 0 ? (
        <EmptyState icon={FolderKanban} title="No projects yet" description="Create one to start deploying services." action={<NewProjectDialog />} />
      ) : (
        <>
          <StatTiles overview={overview} />

          <div className="grid gap-10 lg:grid-cols-2">
            <Section title="Needs attention">
              {overview.attention.length > 0 ? (
                <ServiceRows items={overview.attention} when="status" />
              ) : (
                <div className="flex items-center gap-3 border border-line bg-surface px-4 py-4">
                  <CircleCheck aria-hidden className="size-4 text-ok" />
                  <p className="text-ui text-fg-muted">Every service is running as it should.</p>
                </div>
              )}
            </Section>
            <Section title="Recent deploys">
              {overview.recent.length > 0 ? (
                <ServiceRows items={overview.recent} when="deployed" />
              ) : (
                <div className="flex items-center gap-3 border border-line bg-surface px-4 py-4">
                  <History aria-hidden className="size-4 text-fg-faint" />
                  <p className="text-ui text-fg-muted">Nothing deployed yet.</p>
                </div>
              )}
            </Section>
          </div>

          <Section title="Projects">
            <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {overview.projects.map((project) => (
                <li key={project.name}>
                  <ProjectCard project={project} services={overview.services.filter(({ scope }) => scope.project === project.name)} />
                </li>
              ))}
            </ul>
          </Section>
        </>
      )}
    </div>
  )
}
