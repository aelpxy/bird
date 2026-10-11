import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type Deployment = Schemas['DeploymentInfo']

export const deploymentsQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'deployments'],
    queryFn: ({ signal }) => api<Deployment[]>(servicePath(scope, service, '/deployments'), { signal }),
  })

export function rollback(scope: Scope, service: string, deploymentId: string) {
  const body: Schemas['RollbackRequest'] = { deployment_id: deploymentId }
  return api<Schemas['DeployResponse']>(servicePath(scope, service, '/rollback'), {
    method: 'POST',
    body,
  })
}
