import { Cpu, MemoryStick, Network, Server } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { eyebrowClass } from '@/components/ui/control-styles'
import { EmptyState } from '@/components/ui/empty-state'
import { bytes, timeAgo } from '@/lib/format'
import type { Machine, Service } from '@/features/services/api'
import { machineTones } from '@/features/services/status'

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
    <ul className="grid gap-3 lg:grid-cols-2">
      {machines.map((machine) => (
        <li key={machine.id}>
          <MachineCard machine={machine} service={service} />
        </li>
      ))}
    </ul>
  )
}

function MachineCard({ machine, service }: { machine: Machine; service: Service }) {
  const stats = machine.stats
  const memoryLimit = service.memory * 1024 * 1024
  return (
    <div className="border border-line bg-surface">
      <div className="flex items-center justify-between gap-3 border-b border-line px-3.5 py-2.5">
        <span className="flex min-w-0 items-center gap-2 font-mono text-xs text-fg-muted" title={machine.id}>
          <Server aria-hidden className="size-3.5 shrink-0 text-fg-faint" />
          <span className="truncate">{machine.id.slice(0, 13)}</span>
        </span>
        <Badge tone={machineTones[machine.state]} dot pulse={machine.state === 'starting'}>
          {machine.state}
        </Badge>
      </div>
      {stats ? (
        <dl className="grid grid-cols-2 divide-x divide-line sm:grid-cols-3">
          <Meter icon={Cpu} label="CPU" value={`${(stats.cpu_millicores / 1000).toFixed(2)}`} unit="cores" ratio={stats.cpu_millicores / service.cpus} />
          <Meter icon={MemoryStick} label="Memory" value={bytes(stats.memory_bytes)} ratio={stats.memory_bytes / memoryLimit} />
          <div className="col-span-2 border-t border-line px-3.5 py-3 sm:col-span-1 sm:border-t-0">
            <dt className={`flex items-center gap-1.5 ${eyebrowClass}`}>
              <Network aria-hidden className="size-3" />
              Network
            </dt>
            <dd className="tabular mt-1.5 font-mono text-ui text-fg">
              ↓{bytes(stats.net_rx_bytes)} <span className="text-fg-faint">↑{bytes(stats.net_tx_bytes)}</span>
            </dd>
          </div>
        </dl>
      ) : (
        <p className="px-3.5 py-3 text-xs text-fg-faint">
          {machine.state === 'running' ? 'Measuring…' : `Updated ${timeAgo(machine.updated_at)}`}
        </p>
      )}
    </div>
  )
}

type MeterProps = { icon: typeof Cpu; label: string; value: string; unit?: string; ratio: number }

function Meter({ icon: Icon, label, value, unit, ratio }: MeterProps) {
  const percent = Math.min(100, Math.max(0, Math.round(ratio * 100)))
  const color = percent >= 90 ? 'bg-bad' : percent >= 70 ? 'bg-warn' : 'bg-fg'
  return (
    <div className="px-3.5 py-3">
      <dt className={`flex items-center justify-between gap-2 ${eyebrowClass}`}>
        <span className="flex items-center gap-1.5">
          <Icon aria-hidden className="size-3" />
          {label}
        </span>
        <span className="tabular">{percent}%</span>
      </dt>
      <dd className="tabular mt-1.5 font-mono text-ui text-fg">
        {value}
        {unit && <span className="text-fg-faint"> {unit}</span>}
        <div
          role="meter"
          aria-label={`${label} use`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          aria-valuetext={`${percent}% of the limit`}
          className="mt-2 h-1 bg-raised"
        >
          <div className={`h-full ${color} transition-[width] duration-500`} style={{ width: `${percent}%` }} />
        </div>
      </dd>
    </div>
  )
}
