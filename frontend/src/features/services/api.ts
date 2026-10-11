import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { scopePath, servicePath, type Scope } from '@/lib/api/paths'

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

export function deploy(scope: Scope, body: Schemas['DeployRequest']) {
  return api<Schemas['DeployResponse']>(`${scopePath(scope)}/deploy`, { method: 'POST', body })
}
