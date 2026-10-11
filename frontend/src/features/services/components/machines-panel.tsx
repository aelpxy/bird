import { Server } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { eyebrowClass } from '@/components/ui/control-styles'
import { EmptyState } from '@/components/ui/empty-state'
import { bytes, timeAgo } from '@/lib/format'
import type { Machine, Service } from '@/features/services/api'
import { machineTones } from '@/features/services/status'

const COLUMNS = ['Machine', 'State', 'CPU', 'Memory', 'Network']

export function MachinesPanel({ service }: { service: Service }) {
  const machines = service.deployment?.machines ?? []
  if (machines.length === 0) {
    return (
      <EmptyState
        icon={Server}
        title="No machines"
        description={service.state === 'stopped' ? 'The service is stopped.' : 'Deploy the service to start machines.'}
      />
    )
  }
  return (
    <div className="overflow-x-auto border border-line bg-surface">
      <table className="w-full text-left">
        <thead>
          <tr className="border-b border-line">
            {COLUMNS.map((column) => (
              <th key={column} scope="col" className={`px-3.5 py-2 font-medium whitespace-nowrap ${eyebrowClass}`}>
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {machines.map((machine) => (
            <MachineRow key={machine.id} machine={machine} service={service} />
          ))}
        </tbody>
      </table>
    </div>
  )
}

function MachineRow({ machine, service }: { machine: Machine; service: Service }) {
  const stats = machine.stats
  const memoryLimit = service.memory * 1024 * 1024
  return (
    <tr className="font-mono text-xs whitespace-nowrap">
      <td className="px-3.5 py-2.5 text-fg-muted" title={machine.id}>
        {machine.id.slice(0, 13)}
      </td>
      <td className="px-3.5 py-2.5">
        <Badge tone={machineTones[machine.state]} dot pulse={machine.state === 'starting'}>
          {machine.state}
        </Badge>
      </td>
      {stats ? (
        <>
          <Meter
            label="CPU"
            value={(stats.cpu_millicores / 1000).toFixed(2)}
            limit={`${Number((service.cpus / 1000).toFixed(2))} ${service.cpus === 1000 ? 'core' : 'cores'}`}
            ratio={stats.cpu_millicores / service.cpus}
          />
          <Meter label="Memory" value={bytes(stats.memory_bytes)} limit={bytes(memoryLimit)} ratio={stats.memory_bytes / memoryLimit} />
          <td className="tabular px-3.5 py-2.5 text-fg">
            ↓{bytes(stats.net_rx_bytes)} <span className="text-fg-faint">↑{bytes(stats.net_tx_bytes)}</span>
          </td>
        </>
      ) : (
        <td colSpan={3} className="px-3.5 py-2.5 text-fg-faint">
          {machine.state === 'running' ? 'Measuring…' : `Updated ${timeAgo(machine.updated_at)}`}
        </td>
      )}
    </tr>
  )
}

function Meter({ label, value, limit, ratio }: { label: string; value: string; limit: string; ratio: number }) {
  const percent = Math.min(100, Math.max(0, Math.round(ratio * 100)))
  const color = percent >= 90 ? 'bg-bad' : percent >= 70 ? 'bg-warn' : 'bg-fg'
  return (
    <td className="tabular px-3.5 py-2.5">
      <div className="flex items-center gap-3">
        <span className="text-fg">
          {value}
          <span className="text-fg-faint"> / {limit}</span>
        </span>
        <div
          role="meter"
          aria-label={`${label} use`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          aria-valuetext={`${percent}% of the limit`}
          title={`${percent}%`}
          className="h-1 w-12 shrink-0 bg-raised"
        >
          <div className={`h-full ${color} transition-[width] duration-500`} style={{ width: `${percent}%` }} />
        </div>
      </div>
    </td>
  )
}
