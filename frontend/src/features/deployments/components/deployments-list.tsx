import { useMutation, useQueryClient } from '@tanstack/react-query'
import { History } from 'lucide-react'
import { Badge, type Tone } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import type { Scope } from '@/lib/api/paths'
import { dateTime, plural, timeAgo } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { rollback, type Deployment } from '@/features/deployments/api'

const statuses: Record<Deployment['status'], { label: string; tone: Tone }> = {
  pending: { label: 'Pending', tone: 'neutral' },
  deploying: { label: 'Deploying', tone: 'info' },
  active: { label: 'Active', tone: 'ok' },
  superseded: { label: 'Superseded', tone: 'neutral' },
  failed: { label: 'Failed', tone: 'bad' },
}

type DeploymentsListProps = { scope: Scope; service: string; deployments: Deployment[] }

export function DeploymentsList({ scope, service, deployments }: DeploymentsListProps) {
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (id: string) => rollback(scope, service, id),
    onSuccess: () => notifySuccess(`Rolled back ${service}`),
    onSettled: () =>
      queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] }),
  })

  return (
    <ol className="divide-y divide-line border border-line bg-surface">
      {deployments.map((deployment) => {
        const status = statuses[deployment.status]
        return (
          <li key={deployment.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone={status.tone} dot pulse={deployment.status === 'deploying'}>
                  {status.label}
                </Badge>
                <span className="truncate font-mono text-xs text-fg" title={deployment.image}>
                  {deployment.image}
                </span>
              </div>
              <p className="mt-1 font-mono text-2xs text-fg-faint">
                <time dateTime={new Date(deployment.created_at * 1000).toISOString()} title={dateTime(deployment.created_at)}>
                  {timeAgo(deployment.created_at)}
                </time>
                {' · '}port {deployment.port} · {plural(deployment.variables, 'variable')}
              </p>
            </div>
            {deployment.status === 'superseded' && (
              <ConfirmDialog
                trigger={
                  <Button variant="secondary" size="sm" disabled={mutation.isPending}>
                    <History aria-hidden className="size-3.5" />
                    Roll back
                  </Button>
                }
                title="Roll back to this deployment?"
                description={
                  <>
                    {service} goes back to <span className="font-mono text-fg">{deployment.image}</span>{' '}
                    with the variables and command it had then. Traffic moves once the new machines are healthy.
                  </>
                }
                confirmLabel="Roll back"
                onConfirm={() => mutation.mutateAsync(deployment.id)}
              />
            )}
          </li>
        )
      })}
    </ol>
  )
}
