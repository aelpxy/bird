import { ChevronRight, Server } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { focusRing } from '@/components/ui/control-styles'
import { EmptyState } from '@/components/ui/empty-state'
import { Table, TableCell } from '@/components/ui/table'
import { tableRowClass } from '@/components/ui/table-styles'
import { bytes, timeAgo } from '@/lib/format'
import type { Machine, Service } from '@/features/services/api'
import { machineTones } from '@/features/services/status'
import { UsageChart } from './usage-chart'

const LAYOUT = 'sm:grid-cols-[9.5rem_6.5rem_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)]'
const COLUMNS = [{ label: 'Machine' }, { label: 'State' }, { label: 'CPU' }, { label: 'Memory' }, { label: 'Network' }]
const SAMPLE_SECONDS = 30

export function MachinesPanel({ service }: { service: Service }) {
  const [open, setOpen] = useState<string | null>(null)
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
        <MachineRow key={machine.id} machine={machine} service={service} open={open === machine.id} onToggle={() => setOpen(open === machine.id ? null : machine.id)} />
      ))}
    </Table>
  )
}

type RowProps = { machine: Machine; service: Service; open: boolean; onToggle: () => void }

function MachineRow({ machine, service, open, onToggle }: RowProps) {
  const stats = machine.stats
  const memoryLimit = service.memory * 1024 * 1024
  const chartsId = `charts-${machine.id}`
  return (
    <>
      <div
        role="row"
        tabIndex={0}
        aria-expanded={open}
        aria-controls={chartsId}
        onClick={onToggle}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            onToggle()
          }
        }}
        className={`${tableRowClass(LAYOUT)} cursor-pointer py-3 font-mono text-xs transition-colors hover:bg-raised ${open ? 'bg-raised' : ''} ${focusRing}`}
      >
        <TableCell className="flex items-center gap-1.5 text-fg-muted">
          <ChevronRight aria-hidden className={`size-3.5 shrink-0 text-fg-faint transition-transform ${open ? 'rotate-90' : ''}`} />
          <span className="truncate" title={machine.id}>
            {machine.id.slice(0, 13)}
          </span>
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
            />
            <Meter label="Memory" value={bytes(stats.memory_bytes)} limit={bytes(memoryLimit)} ratio={stats.memory_bytes / memoryLimit} />
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
      </div>
      {open && (
        <div id={chartsId} role="row" className="border-t border-line bg-sunken">
          <div role="cell" className="px-4 py-4">
            <MachineCharts machine={machine} service={service} />
          </div>
        </div>
      )}
    </>
  )
}

// birdd samples every running machine twice a minute and keeps an hour, in memory only
function MachineCharts({ machine, service }: { machine: Machine; service: Service }) {
  const history = machine.history ?? []
  if (history.length < 2) {
    return (
      <p className="text-xs text-fg-faint">
        {machine.state === 'running'
          ? `Collecting usage; the charts fill in every ${SAMPLE_SECONDS} seconds and keep the last hour.`
          : 'Usage is only kept while the machine runs.'}
      </p>
    )
  }
  const memoryLimit = service.memory * 1024 * 1024
  return (
    <div className="grid gap-6 md:grid-cols-2">
      <UsageChart
        title="CPU"
        points={history.map((point) => ({ at: point.at, value: point.cpu_millicores }))}
        limit={service.cpus}
        format={(millicores) => `${(millicores / 1000).toFixed(2)} cores`}
      />
      <UsageChart title="Memory" points={history.map((point) => ({ at: point.at, value: point.memory_bytes }))} limit={memoryLimit} format={bytes} />
    </div>
  )
}

function Meter({ label, value, limit, ratio }: { label: string; value: string; limit: string; ratio: number }) {
  const percent = Math.min(100, Math.max(0, Math.round(ratio * 100)))
  const color = percent >= 90 ? 'bg-bad' : percent >= 70 ? 'bg-warn' : 'bg-fg'
  return (
    <TableCell label={label} className="tabular">
      <div className="flex items-center gap-3">
        <span className="whitespace-nowrap text-fg">
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
    </TableCell>
  )
}
