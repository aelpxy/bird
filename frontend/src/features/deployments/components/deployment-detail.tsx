import { useMutation, useQueryClient } from '@tanstack/react-query'
import { History } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { CopyButton } from '@/components/ui/copy-button'
import { ErrorAlert } from '@/components/ui/error-alert'
import { Eyebrow } from '@/components/ui/eyebrow'
import type { Scope } from '@/lib/api/paths'
import { dateTime, plural } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { rollback, type Deployment, type DeployLogLine } from '../api'
import { deploymentStatuses } from '../status'
import { useDeploymentLog } from '../use-deployment-log'
import { DeployLog } from './deploy-log'

type Kind = DeployLogLine['kind'] | 'all'
const kinds: { value: Kind; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'bird', label: 'Steps' },
  { value: 'build', label: 'Build' },
  { value: 'release', label: 'Release' },
  { value: 'app', label: 'App' },
]

function duration(seconds: number): string {
  return seconds < 60 ? `${seconds.toFixed(1)}s` : `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`
}

type DeploymentDetailProps = { scope: Scope; service: string; deployment: Deployment }

export function DeploymentDetail({ scope, service, deployment }: DeploymentDetailProps) {
  const queryClient = useQueryClient()
  const log = useDeploymentLog(scope, service, deployment)
  const [kind, setKind] = useState<Kind>('all')
  const status = deploymentStatuses[deployment.status]
  const back = useMutation({
    mutationFn: () => rollback(scope, service, deployment.id),
    onSuccess: () => notifySuccess(`Rolled back ${service}`),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] }),
  })
  const present = new Set(log.lines.map((line) => line.kind))
  const shown = kind === 'all' ? log.lines : log.lines.filter((line) => line.kind === kind)
  const first = log.lines.at(0)
  const last = log.lines.at(-1)
  const took = first && last && !log.following ? (last.at - first.at) / 1000 : undefined
  // birdd's own last word on a failed deploy says why
  const reason = deployment.status === 'failed' ? log.lines.findLast((line) => line.kind === 'bird' && line.text.startsWith('The deploy failed'))?.text : undefined

  return (
    <div className="flex min-w-0 flex-col gap-5">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={status.tone} dot pulse={deployment.status === 'deploying'}>
              {status.label}
            </Badge>
            <span className="truncate font-mono text-sm text-fg" title={deployment.image}>
              {deployment.image}
            </span>
          </div>
          <p className="mt-2 font-mono text-2xs text-fg-faint">
            {[dateTime(deployment.created_at), took !== undefined && `took ${duration(took)}`, `port ${deployment.port}`, plural(deployment.variables, 'variable')]
              .filter(Boolean)
              .join(' · ')}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <CopyButton value={deployment.id} label="Copy id" />
          {deployment.status === 'superseded' && (
            <ConfirmDialog
              trigger={
                <Button variant="secondary" size="sm" disabled={back.isPending}>
                  <History aria-hidden className="size-3.5" />
                  Roll back
                </Button>
              }
              title="Roll back to this deployment?"
              description={
                <>
                  {service} goes back to <span className="font-mono text-fg">{deployment.image}</span> with the variables and command it had then. Traffic moves once
                  the new machines are healthy.
                </>
              }
              confirmLabel="Roll back"
              onConfirm={() => back.mutateAsync()}
            />
          )}
        </div>
      </div>

      {reason && <p className="border border-bad/40 bg-bad/5 px-3 py-2.5 text-ui text-bad">{reason}</p>}

      <div className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Eyebrow>Log</Eyebrow>
          <div role="radiogroup" aria-label="Show" className="flex border border-line">
            {kinds
              .filter((option) => option.value === 'all' || present.has(option.value))
              .map((option) => (
                <button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={kind === option.value}
                  onClick={() => setKind(option.value)}
                  className={`px-2 py-1 font-mono text-2xs tracking-[0.06em] uppercase transition-colors ${kind === option.value ? 'bg-raised text-fg' : 'text-fg-faint hover:text-fg'}`}
                >
                  {option.label}
                </button>
              ))}
          </div>
        </div>
        {log.error ? <ErrorAlert error={log.error} /> : <DeployLog lines={shown} live={log.following} tall empty={log.loading ? 'Loading the log…' : undefined} />}
      </div>
    </div>
  )
}
