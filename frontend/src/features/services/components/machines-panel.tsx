import { Server } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { EmptyState } from '@/components/ui/empty-state'
import { Sparkline } from '@/components/ui/sparkline'
import { Table, TableCell, TableRow } from '@/components/ui/table'
import { bytes, timeAgo } from '@/lib/format'
import type { Machine, Service } from '@/features/services/api'
import { machineTones } from '@/features/services/status'

const LAYOUT = 'sm:grid-cols-[8.5rem_6.5rem_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)]'
const COLUMNS = [{ label: 'Machine' }, { label: 'State' }, { label: 'CPU' }, { label: 'Memory' }, { label: 'Network' }]

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
    <Table label="Machines" columns={COLUMNS} layout={LAYOUT}>
      {machines.map((machine) => (
        <MachineRow key={machine.id} machine={machine} service={service} />
      ))}
    </Table>
  )
}

function MachineRow({ machine, service }: { machine: Machine; service: Service }) {
  const stats = machine.stats
  const memoryLimit = service.memory * 1024 * 1024
  return (
    <TableRow layout={LAYOUT} className="font-mono text-xs">
      <TableCell className="truncate text-fg-muted">
        <span title={machine.id}>{machine.id.slice(0, 13)}</span>
      </TableCell>
      <TableCell>
        <Badge tone={machineTones[machine.state]} dot pulse={machine.state === 'starting'}>
          {machine.state}
        </Badge>
      </TableCell>
      {stats ? (
        <>
          <Meter
            label="CPU"
            value={(stats.cpu_millicores / 1000).toFixed(2)}
            limit={`${Number((service.cpus / 1000).toFixed(2))} ${service.cpus === 1000 ? 'core' : 'cores'}`}
            ratio={stats.cpu_millicores / service.cpus}
            history={machine.history?.map((point) => point.cpu_millicores / service.cpus)}
          />
          <Meter
            label="Memory"
            value={bytes(stats.memory_bytes)}
            limit={bytes(memoryLimit)}
            ratio={stats.memory_bytes / memoryLimit}
            history={machine.history?.map((point) => point.memory_bytes / memoryLimit)}
          />
          <TableCell label="Network" className="tabular text-fg">
            <span>
              ↓{bytes(stats.net_rx_bytes)} <span className="text-fg-faint">↑{bytes(stats.net_tx_bytes)}</span>
            </span>
          </TableCell>
        </>
      ) : (
        <TableCell className="text-fg-faint max-sm:col-span-2 sm:col-span-3">
          {machine.state === 'running' ? 'Measuring…' : `Updated ${timeAgo(machine.updated_at)}`}
        </TableCell>
      )}
    </TableRow>
  )
}

type MeterProps = { label: string; value: string; limit: string; ratio: number; history?: number[] }

function Meter({ label, value, limit, ratio, history = [] }: MeterProps) {
  const percent = Math.min(100, Math.max(0, Math.round(ratio * 100)))
  const color = percent >= 90 ? 'bg-bad' : percent >= 70 ? 'bg-warn' : 'bg-fg'
  const line = percent >= 90 ? 'text-bad' : percent >= 70 ? 'text-warn' : 'text-fg-muted'
  return (
    <TableCell label={label} className="tabular">
      <div className="flex items-center gap-3">
        <span className="whitespace-nowrap text-fg">
          {value}
          <span className="text-fg-faint"> / {limit}</span>
        </span>
        {history.length >= 2 ? (
          <Sparkline values={history} label={`${label} over the last ${Math.round((history.length * 30) / 60)} minutes, now ${percent}% of the limit`} className={line} />
        ) : (
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
        )}
      </div>
    </TableCell>
  )
}
