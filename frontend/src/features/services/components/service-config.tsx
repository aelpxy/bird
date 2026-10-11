import { Link } from '@tanstack/react-router'
import { HardDrive, Settings2 } from 'lucide-react'
import { Fragment, type ReactNode } from 'react'
import { focusRing } from '@/components/ui/control-styles'
import { CopyButton } from '@/components/ui/copy-button'
import { Eyebrow } from '@/components/ui/eyebrow'
import type { Scope } from '@/lib/api/paths'
import { interval, mebibytes, plural } from '@/lib/format'
import type { Service } from '../api'

// long names break only after separators, never in the middle of a word
function breakable(text: string): ReactNode {
  return text.split(/(?<=[/:@.-])/).map((part, index) => (
    <Fragment key={index}>
      {part}
      <wbr />
    </Fragment>
  ))
}

function quote(args: string[]): string {
  return args.map((arg) => (/[\s'"\\]/.test(arg) || arg === '' ? JSON.stringify(arg) : arg)).join(' ')
}

function healthLabel(health: string, timeout: number): string {
  const check = health === 'http' ? 'HTTP' : health === 'tcp' ? 'TCP' : `HTTP ${health}`
  return `${check} · ${timeout}s`
}

function Block({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2 px-4 py-3.5">
      <Eyebrow>{label}</Eyebrow>
      {children}
    </div>
  )
}

function Chip({ children }: { children: ReactNode }) {
  return <span className="border border-line bg-sunken px-1.5 py-0.5 font-mono text-2xs text-fg-muted">{children}</span>
}

function CommandLine({ args }: { args: string[] }) {
  return <code className="block border border-line bg-sunken px-3 py-2 font-mono text-xs leading-5 break-words whitespace-pre-wrap text-fg">{quote(args)}</code>
}

export function ServiceConfig({ scope, service }: { scope: Scope; service: Service }) {
  const tiles = [
    { label: 'Machines', value: String(service.replicas) },
    { label: 'Memory', value: mebibytes(service.memory) },
    { label: 'CPU', value: `${Number((service.cpus / 1000).toFixed(2))}`, unit: service.cpus === 1000 ? 'core' : 'cores' },
  ]
  // birdd's defaults, for a summary from before stop settings existed
  const stop = service.stop ?? { drain: 5, kill_signal: 'SIGTERM', kill_timeout: 10 }
  return (
    <div className="divide-y divide-line border border-line bg-surface">
      <Block label="Image">
        <div className="flex items-start justify-between gap-2">
          <p className="min-w-0 font-mono text-ui leading-5 text-fg">{breakable(service.image)}</p>
          <CopyButton value={service.image} label="Copy" />
        </div>
        <div className="flex flex-wrap gap-1.5">
          <Chip>:{service.port}</Chip>
          <Chip>{healthLabel(service.health, service.health_timeout)}</Chip>
        </div>
      </Block>

      <Block label="Command">
        {service.command ? <CommandLine args={service.command} /> : <p className="text-ui text-fg-faint">The image&apos;s own</p>}
      </Block>
      {service.release && (
        <Block label="Release, before each deploy">
          <CommandLine args={service.release} />
        </Block>
      )}

      <Block label="Stopping old machines">
        <p className="text-ui text-fg-muted">
          {stop.drain > 0 ? `They finish requests for ${stop.drain}s, then get ` : 'They get '}
          <span className="font-mono text-fg">{stop.kill_signal}</span> and are killed if still running {stop.kill_timeout}s later.
        </p>
      </Block>

      <dl className="grid grid-cols-3 divide-x divide-line">
        {tiles.map((tile) => (
          <div key={tile.label} className="px-4 py-3.5">
            <dt>
              <Eyebrow as="span">{tile.label}</Eyebrow>
            </dt>
            <dd className="tabular mt-1.5 font-mono text-lg text-fg">
              {tile.value}
              {tile.unit && <span className="ml-1 text-2xs text-fg-faint">{tile.unit}</span>}
            </dd>
          </div>
        ))}
      </dl>
      <p className="px-4 py-2 text-2xs text-fg-faint">Memory and CPU are per machine.</p>

      <Block label={service.volumes.length === 1 ? 'Volume' : 'Volumes'}>
        {service.volumes.length > 0 ? (
          <ul className="flex flex-col gap-1.5">
            {service.volumes.map((volume) => (
              <li key={volume.name} className="flex items-center gap-2 font-mono text-ui text-fg">
                <HardDrive aria-hidden className="size-3.5 shrink-0 text-fg-faint" />
                {volume.name}
                <span className="text-fg-faint">→</span>
                <span className="min-w-0 truncate">{volume.path}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-ui text-fg-faint">None, so nothing written survives a deploy</p>
        )}
        {service.volumes.length > 0 && (
          <Link
            to="/projects/$project/$environment/services/$service/backups"
            params={{ ...scope, service: service.name }}
            className={`self-start text-xs underline decoration-line-strong underline-offset-4 hover:text-fg ${service.backup_schedule ? 'text-fg-muted' : 'text-warn'} ${focusRing}`}
          >
            {service.backup_schedule
              ? `Backed up every ${interval(service.backup_schedule.every)}, keeping ${plural(service.backup_schedule.keep, 'copy', 'copies')}`
              : 'Not backed up on a schedule'}
          </Link>
        )}
      </Block>

      <Link
        to="/projects/$project/$environment/services/$service/settings"
        params={{ ...scope, service: service.name }}
        className={`flex items-center gap-2 px-4 py-2.5 font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase transition-colors hover:bg-raised hover:text-fg ${focusRing}`}
      >
        <Settings2 aria-hidden className="size-3.5" />
        Edit settings
      </Link>
    </div>
  )
}
