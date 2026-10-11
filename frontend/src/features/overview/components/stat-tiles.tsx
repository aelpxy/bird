import { eyebrowClass } from '@/components/ui/control-styles'
import type { Overview } from '../use-overview'

type Tile = { label: string; value: string; detail: string; tone?: 'bad' }

export function StatTiles({ overview }: { overview: Overview }) {
  const total = overview.services.length
  const up = overview.services.filter(({ service }) => service.state === 'running' && service.deployment).length
  const environments = overview.projects.reduce((sum, project) => sum + project.environments.length, 0)
  const tiles: Tile[] = [
    { label: 'Projects', value: String(overview.projects.length), detail: `${environments} ${environments === 1 ? 'environment' : 'environments'}` },
    { label: 'Services', value: `${up}`, detail: `running of ${total}` },
    { label: 'Machines', value: `${overview.machines.running}`, detail: `of ${overview.machines.wanted} wanted` },
    {
      label: 'Attention',
      value: String(overview.attention.length),
      detail: overview.attention.length === 0 ? 'all healthy' : overview.attention.length === 1 ? 'service needs a look' : 'services need a look',
      tone: overview.attention.length > 0 ? 'bad' : undefined,
    },
  ]
  return (
    <dl className="grid grid-cols-2 border border-line bg-surface lg:grid-cols-4">
      {tiles.map((tile, index) => (
        <div
          key={tile.label}
          className={`flex flex-col gap-3 p-4 ${index % 2 === 1 ? 'border-l border-line' : ''} ${index >= 2 ? 'border-t border-line lg:border-t-0' : ''} ${index === 2 ? 'lg:border-l' : ''}`}
        >
          <dt className={`flex items-center gap-2 ${eyebrowClass}`}>
            {tile.tone === 'bad' && <span aria-hidden className="size-1.5 animate-pulse bg-bad" />}
            {tile.label}
          </dt>
          <dd>
            <span className={`tabular block font-mono text-3xl font-medium tracking-[-0.03em] ${tile.tone === 'bad' ? 'text-bad' : 'text-fg'}`}>{tile.value}</span>
            <span className="mt-1 block text-xs text-fg-faint">{tile.detail}</span>
          </dd>
        </div>
      ))}
    </dl>
  )
}
