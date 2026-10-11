import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type ServiceMetrics = Schemas['ServiceMetrics']
export type MetricPoint = Schemas['MetricPoint']
export type MetricsRange = ServiceMetrics['range']

export const METRICS_RANGES: { value: MetricsRange; label: string }[] = [
  { value: '1h', label: '1h' },
  { value: '6h', label: '6h' },
  { value: '7d', label: '7d' },
  { value: '30d', label: '30d' },
]

// birdd samples every 30 seconds, so refreshing faster would show nothing new
const REFRESH_MS = 30_000

export const metricsQuery = (scope: Scope, service: string, range: MetricsRange, machine?: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'metrics', range, machine ?? null],
    queryFn: ({ signal }) => {
      const query = new URLSearchParams({ range, ...(machine ? { machine } : {}) })
      return api<ServiceMetrics>(servicePath(scope, service, `/metrics?${query}`), { signal })
    },
    refetchInterval: REFRESH_MS,
    placeholderData: (previous) => previous,
  })
