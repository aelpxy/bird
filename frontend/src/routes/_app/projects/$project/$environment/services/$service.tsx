import { useQuery } from '@tanstack/react-query'
import { Link, Outlet, createFileRoute } from '@tanstack/react-router'
import { PackageX } from 'lucide-react'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { Skeleton } from '@/components/ui/skeleton'
import { ApiError } from '@/lib/api/client'
import { serviceQuery } from '@/features/services/api'
import { ServiceHeader } from '@/features/services/components/service-header'
import { ServiceTabs } from '@/features/services/components/service-tabs'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service')({
  head: ({ params }) => ({ meta: [{ title: `${params.service} · ${params.project} · bird` }] }),
  component: ServiceLayout,
})

function ServiceLayout() {
  const { scope } = Route.useRouteContext()
  const { service: name } = Route.useParams()
  const service = useQuery(serviceQuery(scope, name))

  if (service.isPending) {
    return (
      <div className="flex flex-col gap-4" aria-busy aria-label="Loading service">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-4 w-72" />
        <Skeleton className="mt-4 h-9 w-full" />
      </div>
    )
  }
  if (service.isError) {
    if (service.error instanceof ApiError && service.error.status === 404) {
      return (
        <EmptyState
          icon={PackageX}
          title={`There is no ${name} in ${scope.environment}`}
          description="It may have been removed."
          action={
            <Link
              to="/projects/$project/$environment"
              params={scope}
              className="text-sm text-fg-muted underline underline-offset-4 hover:text-fg"
            >
              Back to services
            </Link>
          }
        />
      )
    }
    return <ErrorState error={service.error} onRetry={() => void service.refetch()} />
  }
  return (
    <div className="flex flex-col gap-6">
      <ServiceHeader scope={scope} service={service.data} />
      <ServiceTabs scope={scope} service={name} hasVolumes={service.data.volumes.length > 0} />
      <Outlet />
    </div>
  )
}
