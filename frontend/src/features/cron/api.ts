import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type CronJob = Schemas['CronJobSummary']
export type CronRun = Schemas['CronRunSummary']
export type CronRunDetail = Schemas['CronRunDetail']

const RUNNING_REFRESH_MS = 2_000

const job = (scope: Scope, service: string, name: string, rest = '') =>
  servicePath(scope, service, `/cron/${encodeURIComponent(name)}${rest}`)

export const cronJobsQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'cron'],
    queryFn: ({ signal }) => api<CronJob[]>(servicePath(scope, service, '/cron'), { signal }),
    refetchInterval: 10_000,
  })

export const cronRunsQuery = (scope: Scope, service: string, name: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'cron', name, 'runs'],
    queryFn: ({ signal }) => api<CronRun[]>(job(scope, service, name, '/runs'), { signal }),
    refetchInterval: (query) => (query.state.data?.some((run) => run.status === 'running') ? RUNNING_REFRESH_MS : false),
  })

// polled while the run is going, so its output fills in
export const cronRunQuery = (scope: Scope, service: string, name: string, id: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'cron', name, 'runs', id],
    queryFn: ({ signal }) => api<CronRunDetail>(job(scope, service, name, `/runs/${encodeURIComponent(id)}`), { signal }),
    refetchInterval: (query) => (query.state.data?.status === 'running' ? RUNNING_REFRESH_MS : false),
  })

export function triggerRun(scope: Scope, service: string, name: string) {
  return api<CronRun>(job(scope, service, name, '/runs'), { method: 'POST' })
}
