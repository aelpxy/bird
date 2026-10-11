import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { focusRing } from '@/components/ui/control-styles'
import { Skeleton } from '@/components/ui/skeleton'
import type { Scope } from '@/lib/api/paths'
import { bytes } from '@/lib/format'
import { deploymentsQuery } from '@/features/deployments/api'
import type { Service } from '@/features/services/api'
import { METRICS_RANGES, metricsQuery, type MetricsRange, type ServiceMetrics } from '../api'
import { cores } from '../format'
import { MetricChart, type Marker } from './metric-chart'

const RANGE_SECONDS: Record<MetricsRange, number> = { '1h': 3600, '6h': 21_600, '7d': 604_800, '30d': 2_592_000 }

// the service's machines added up, kept by birdd across deploys and restarts for 30 days
export function UsagePanel({ scope, service }: { scope: Scope; service: Service }) {
  const [range, setRange] = useState<MetricsRange>('1h')
  const metrics = useQuery(metricsQuery(scope, service.name, range))
  const deployments = useQuery(deploymentsQuery(scope, service.name))
  const markers: Marker[] = (deployments.data ?? []).map((deployment) => ({ at: deployment.created_at, label: `Deploy ${deployment.status}` }))
  const machines = Math.max(1, service.replicas)

  return (
    <div className="flex flex-col gap-4 border border-line bg-surface p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-xs text-fg-faint">All machines added up. Dotted lines are deploys.</p>
        <div role="radiogroup" aria-label="Range" className="flex border border-line">
          {METRICS_RANGES.map((option) => (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={range === option.value}
              onClick={() => setRange(option.value)}
              className={`px-2 py-1 font-mono text-2xs tracking-[0.06em] uppercase transition-colors ${range === option.value ? 'bg-raised text-fg' : 'text-fg-faint hover:text-fg'} ${focusRing}`}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>
      {metrics.isPending ? (
        <Skeleton className="h-40 w-full" />
      ) : metrics.isError ? (
        <p className="text-sm text-fg-faint">Couldn&apos;t load usage.</p>
      ) : (
        <UsageCharts data={metrics.data} now={metrics.dataUpdatedAt} range={range} markers={markers} cpuLimit={service.cpus * machines} memoryLimit={service.memory * 1024 * 1024 * machines} />
      )}
    </div>
  )
}

// `now` is when the data was fetched, in unix milliseconds, so the axis ends where the data does
type ChartsProps = { data: ServiceMetrics; now: number; range: MetricsRange; markers?: Marker[]; cpuLimit: number; memoryLimit: number; traffic?: boolean }

export function UsageCharts({ data, now, range, markers, cpuLimit, memoryLimit, traffic = true }: ChartsProps) {
  const to = Math.floor(now / 1000)
  const from = to - RANGE_SECONDS[range]
  const step = data.step_secs
  const perSecond = (total: number) => `${bytes(total / step)}/s`
  const shared = { from, to, step, markers }
  return (
    <div className="grid gap-6 md:grid-cols-2">
      <MetricChart {...shared} title="CPU" limit={cpuLimit} format={cores} points={data.points.map((point) => ({ at: point.at, value: point.cpu_millicores, peak: point.cpu_peak_millicores }))} />
      <MetricChart {...shared} title="Memory" limit={memoryLimit} format={bytes} points={data.points.map((point) => ({ at: point.at, value: point.memory_bytes, peak: point.memory_peak_bytes }))} />
      {traffic && (
        <>
          <MetricChart {...shared} title="Traffic in" format={(value) => perSecond(value)} points={data.points.map((point) => ({ at: point.at, value: point.net_rx_bytes }))} />
          <MetricChart {...shared} title="Traffic out" format={(value) => perSecond(value)} points={data.points.map((point) => ({ at: point.at, value: point.net_tx_bytes }))} />
        </>
      )}
    </div>
  )
}
