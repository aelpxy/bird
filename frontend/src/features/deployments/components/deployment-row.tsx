import { focusRing } from '@/components/ui/control-styles'
import { timeAgo, dateTime } from '@/lib/format'
import type { Deployment } from '../api'
import { deploymentStatuses, splitImage } from '../status'

type DeploymentRowProps = { deployment: Deployment; selected: boolean; onSelect: () => void }

export function DeploymentRow({ deployment, selected, onSelect }: DeploymentRowProps) {
  const status = deploymentStatuses[deployment.status]
  const { tag } = splitImage(deployment.image)
  return (
    <button
      type="button"
      aria-current={selected ? 'true' : undefined}
      onClick={onSelect}
      title={deployment.image}
      className={`relative flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors ${selected ? 'bg-raised' : 'hover:bg-raised/60'} ${focusRing}`}
    >
      {selected && <span aria-hidden className="absolute inset-y-0 left-0 w-0.5 bg-fg" />}
      <span aria-hidden className={`size-2 shrink-0 ${status.dot} ${deployment.status === 'deploying' ? 'animate-pulse' : ''}`} />
      <span className="flex min-w-0 flex-1 items-baseline gap-2">
        <span className="truncate font-mono text-ui font-medium text-fg">{tag}</span>
        {deployment.status === 'active' && <span className="font-mono text-2xs tracking-[0.08em] text-ok uppercase">Live</span>}
        {deployment.status === 'failed' && <span className="font-mono text-2xs tracking-[0.08em] text-bad uppercase">Failed</span>}
        {deployment.status === 'deploying' && <span className="font-mono text-2xs tracking-[0.08em] text-info uppercase">Deploying</span>}
      </span>
      <time className="shrink-0 font-mono text-2xs text-fg-faint" dateTime={new Date(deployment.created_at * 1000).toISOString()} title={dateTime(deployment.created_at)}>
        {timeAgo(deployment.created_at)}
      </time>
    </button>
  )
}
