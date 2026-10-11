import { queryOptions } from '@tanstack/react-query'
import { api, streamJsonLines, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type Deployment = Schemas['DeploymentInfo']
export type DeployLogLine = Schemas['DeployLogLine']
export type DeploymentLog = Schemas['DeploymentLog']

const NDJSON = 'application/x-ndjson'
const DEPLOYING_POLL_MS = 2_000
const IDLE_POLL_MS = 15_000

function logPath(scope: Scope, service: string, id: string, follow: boolean) {
  return servicePath(scope, service, `/deployments/${id}/log${follow ? '?follow=true' : ''}`)
}

export function deploymentLog(scope: Scope, service: string, id: string, signal: AbortSignal) {
  return api<DeploymentLog>(logPath(scope, service, id, false), { signal })
}

// every line so far, then new ones until the deploy ends
export function followDeploymentLog(scope: Scope, service: string, id: string, onLine: (line: DeployLogLine) => void, signal: AbortSignal) {
  return streamJsonLines<DeployLogLine>(logPath(scope, service, id, true), onLine, signal, { accept: NDJSON })
}

export const deploymentsQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'deployments'],
    queryFn: ({ signal }) => api<Deployment[]>(servicePath(scope, service, '/deployments'), { signal }),
    // a deploy started anywhere, the CLI included, should show up and settle without a reload
    refetchInterval: (query) => (query.state.data?.some((deployment) => deployment.status === 'deploying') ? DEPLOYING_POLL_MS : IDLE_POLL_MS),
  })

export function rollback(scope: Scope, service: string, deploymentId: string) {
  const body: Schemas['RollbackRequest'] = { deployment_id: deploymentId }
  return api<Schemas['DeployResponse']>(servicePath(scope, service, '/rollback'), {
    method: 'POST',
    body,
  })
}
