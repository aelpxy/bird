import type { ReactNode } from 'react'
import { eyebrowClass } from '@/components/ui/control-styles'
import { cores, interval, mebibytes, plural } from '@/lib/format'
import type { Service } from '@/features/services/api'

function healthLabel(health: string): string {
  if (health === 'http') return 'any HTTP answer on /'
  if (health === 'tcp') return 'accepts TCP connections'
  return `2xx on ${health}`
}

const mono = (value: ReactNode) => <span className="font-mono text-xs break-all">{value}</span>

export function ServiceConfig({ service }: { service: Service }) {
  const rows: [string, ReactNode][] = [
    ['Image', mono(service.image)],
    ['Port', mono(service.port)],
    ['Command', service.command ? mono(service.command.join(' ')) : "The image's own"],
    ['Release', service.release ? mono(service.release.join(' ')) : 'None'],
    ['Health', `${healthLabel(service.health)}, within ${service.health_timeout}s`],
    ['Machines', service.replicas],
    ['Memory', `${mebibytes(service.memory)} each`],
    ['CPU', `${cores(service.cpus)} each`],
    [
      'Volumes',
      service.volumes.length > 0
        ? service.volumes.map((volume) => (
            <span key={volume.name} className="block font-mono text-xs">
              {volume.name} → {volume.path}
            </span>
          ))
        : 'None',
    ],
    ['Backups', service.backup_schedule ? `Every ${interval(service.backup_schedule.every)}, keeping ${plural(service.backup_schedule.keep, 'copy', 'copies')}` : 'Not scheduled'],
  ]
  return (
    <dl className="divide-y divide-line border border-line bg-surface">
      {rows.map(([label, value]) => (
        <div key={label} className="grid grid-cols-[6.5rem_1fr] gap-3 px-3.5 py-2.5 text-ui">
          <dt className={`${eyebrowClass} pt-0.5`}>{label}</dt>
          <dd className="min-w-0 text-fg">{value}</dd>
        </div>
      ))}
    </dl>
  )
}
