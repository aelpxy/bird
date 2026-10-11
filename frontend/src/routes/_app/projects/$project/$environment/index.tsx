import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Boxes } from 'lucide-react'
import { Code } from '@/components/ui/code'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { PageHeader } from '@/components/ui/page-header'
import { servicesQuery } from '@/features/services/api'
import { NewServiceDialog } from '@/features/services/components/new-service-dialog'
import { ServicesList, ServicesListSkeleton } from '@/features/services/components/services-list'

export const Route = createFileRoute('/_app/projects/$project/$environment/')({
  head: ({ params }) => ({ meta: [{ title: `${params.environment} · ${params.project} · bird` }] }),
  component: Services,
})

function Services() {
  const { scope } = Route.useRouteContext()
  const services = useQuery(servicesQuery(scope))
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
              Deploy an image, a database or a folder here, or from a folder with a <Code>bird.toml</Code> run{' '}
              <Code>
                bird deploy -p {scope.project} -E {scope.environment}
              </Code>
            </>
          }
          action={<NewServiceDialog scope={scope} />}
        />
      ) : (
        <ServicesList scope={scope} services={services.data} />
      )}
    </div>
  )
}
