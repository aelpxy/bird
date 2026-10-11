import { queryOptions } from '@tanstack/react-query'
import { ApiError, api, streamJsonLines, type Schemas } from '@/lib/api/client'
import { scopePath, servicePath, type Scope } from '@/lib/api/paths'
import { endLive, liveKey, pushLive, startLive } from '@/features/deployments/live'

export type Service = Schemas['ServiceSummary']
export type Machine = Schemas['MachineSummary']

// birdd has no event stream, so pages showing live state poll it
export const LIVE_REFRESH_MS = 5_000

export const servicesQuery = (scope: Scope) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment],
    queryFn: ({ signal }) => api<Service[]>(`${scopePath(scope)}/services`, { signal }),
    refetchInterval: LIVE_REFRESH_MS,
  })

// with stats birdd samples each machine twice, half a second apart
export const serviceQuery = (scope: Scope, name: string, stats = false) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, name, { stats }],
    queryFn: ({ signal }) =>
      api<Service>(servicePath(scope, name, stats ? '?stats=true' : ''), { signal }),
    refetchInterval: LIVE_REFRESH_MS,
  })

export type PowerAction = 'start' | 'stop' | 'restart'

export function power(scope: Scope, name: string, action: PowerAction) {
  return api<Service>(servicePath(scope, name, `/${action}`), { method: 'POST' })
}

export function scale(scope: Scope, name: string, replicas: number) {
  return api<void>(servicePath(scope, name, '/scale'), { method: 'PUT', body: { replicas } })
}

export function removeService(scope: Scope, name: string, purge: boolean) {
  return api<void>(servicePath(scope, name, purge ? '?purge=true' : ''), { method: 'DELETE' })
}

// streams the deploy's log into the live store, so the service's pages can show it as it runs
export async function deploy(scope: Scope, body: Schemas['DeployRequest']): Promise<Schemas['DeployResponse']> {
  const key = liveKey(scope, body.name)
  startLive(key)
  let outcome: { deployed: Schemas['DeployResponse'] } | { failed: Schemas['ErrorBody'] } | undefined
  try {
    await streamJsonLines<Schemas['DeployEvent']>(
      `${scopePath(scope)}/deploy`,
      (event) => {
        if (event.type === 'log') pushLive(key, event)
        else if (event.type === 'deployed') outcome = { deployed: event }
        else outcome = { failed: event }
      },
      new AbortController().signal,
      { method: 'POST', body: JSON.stringify(body), contentType: 'application/json', accept: 'application/x-ndjson' },
    )
  } catch (error) {
    endLive(key, error instanceof Error ? error.message : 'The deploy failed')
    throw error
  }
  if (outcome && 'deployed' in outcome) {
    endLive(key)
    return outcome.deployed
  }
  const failed = outcome?.failed ?? { error: 'birdd ended the deploy without a result' }
  endLive(key, failed.error)
  throw new ApiError(502, failed.error, failed.logs ?? [])
}
