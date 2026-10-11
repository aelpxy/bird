import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Boxes } from 'lucide-react'
import { Code } from '@/components/ui/code'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { PageHeader } from '@/components/ui/page-header'
import { servicesQuery } from '@/features/services/api'
import { NewServiceDialog } from '@/features/services/components/new-service-dialog'
import { ServiceMap } from '@/features/services/components/service-map'
import { ServicesList, ServicesListSkeleton } from '@/features/services/components/services-list'
import { focusRing } from '@/components/ui/control-styles'

export const Route = createFileRoute('/_app/projects/$project/$environment/')({
  head: ({ params }) => ({ meta: [{ title: `${params.environment} · ${params.project} · bird` }] }),
  validateSearch: (search: Record<string, unknown>): { view?: 'map' } => (search.view === 'map' ? { view: 'map' } : {}),
  component: Services,
})

const viewClass = `px-2.5 py-1 font-mono text-2xs font-medium tracking-[0.08em] text-fg-faint uppercase transition-colors hover:text-fg aria-[current=page]:bg-raised aria-[current=page]:text-fg ${focusRing}`

function ViewSwitch({ map }: { map: boolean }) {
  return (
    <nav aria-label="View" className="flex self-start border border-line">
      <Link to="." search={{}} aria-current={map ? undefined : 'page'} className={viewClass}>
        List
      </Link>
      <Link to="." search={{ view: 'map' }} aria-current={map ? 'page' : undefined} className={`border-l border-line ${viewClass}`}>
        Map
      </Link>
    </nav>
  )
}

function Services() {
  const { scope } = Route.useRouteContext()
  const services = useQuery(servicesQuery(scope))
  const map = Route.useSearch().view === 'map'
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        eyebrow={scope.environment}
        title="Services"
        description={`Everything running in ${scope.environment}.`}
        actions={<NewServiceDialog scope={scope} />}
      />
      {services.isPending ? (
        <ServicesListSkeleton />
      ) : services.isError ? (
        <ErrorState error={services.error} onRetry={() => void services.refetch()} />
      ) : services.data.length === 0 ? (
        <EmptyState
          icon={Boxes}
          title="No services yet"
          description={
            <>
              Each part of {scope.project} is its own service: the app, its database, a worker. They reach each other by name on{' '}
              {scope.environment}&apos;s private network. Deploy an image, a database or a folder here, or from a folder with a <Code>bird.toml</Code> run{' '}
              <Code>
                bird deploy -p {scope.project} -E {scope.environment}
              </Code>
            </>
          }
          action={<NewServiceDialog scope={scope} />}
        />
      ) : (
        <>
          <ViewSwitch map={map} />
          {map ? <ServiceMap scope={scope} services={services.data} /> : <ServicesList scope={scope} services={services.data} />}
        </>
      )}
    </div>
  )
}
