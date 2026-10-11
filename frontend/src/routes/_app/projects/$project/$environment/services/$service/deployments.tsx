import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { History } from 'lucide-react'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { Skeleton } from '@/components/ui/skeleton'
import { SourceDeployDialog } from '@/features/builds/components/source-deploy-dialog'
import { deploymentsQuery } from '@/features/deployments/api'
import { DeploymentsView } from '@/features/deployments/components/deployments-view'
import { serviceQuery } from '@/features/services/api'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/deployments')({
  component: Deployments,
})

function Deployments() {
  const { scope, service } = useServiceScope()
  const deployments = useQuery(deploymentsQuery(scope, service))
  const { data: summary } = useQuery(serviceQuery(scope, service))
  const header = summary && (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p className="text-sm text-fg-muted">Every deploy with its log, newest first. Earlier working ones can be rolled back to.</p>
      <SourceDeployDialog scope={scope} service={summary} />
    </div>
  )
  if (deployments.isPending) return <Skeleton className="h-96 w-full" />
  if (deployments.isError) {
    return <ErrorState error={deployments.error} onRetry={() => void deployments.refetch()} />
  }
  if (deployments.data.length === 0) {
    return <EmptyState icon={History} title="No deployments yet" />
  }
  return (
    <div className="flex flex-col gap-4">
      {header}
      <DeploymentsView scope={scope} service={service} deployments={deployments.data} />
    </div>
  )
}
