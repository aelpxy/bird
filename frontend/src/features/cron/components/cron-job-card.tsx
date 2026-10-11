import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ChevronDown, Clock, Play } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Code } from '@/components/ui/code'
import { Skeleton } from '@/components/ui/skeleton'
import type { Scope } from '@/lib/api/paths'
import { dateTime, interval, timeAgo } from '@/lib/format'
import { notifyError, notifySuccess } from '@/lib/toast'
import { cronJobsQuery, cronRunsQuery, triggerRun, type CronJob } from '@/features/cron/api'
import { runStatuses } from '@/features/cron/run-status'
import { RunOutputModal } from './run-output-modal'

export function CronJobCard({ scope, service, job }: { scope: Scope; service: string; job: CronJob }) {
  const queryClient = useQueryClient()
  const [expanded, setExpanded] = useState(false)
  const [openRun, setOpenRun] = useState<string | null>(null)
  const runs = useQuery({ ...cronRunsQuery(scope, service, job.name), enabled: expanded })
  const trigger = useMutation({
    mutationFn: () => triggerRun(scope, service, job.name),
    onSuccess: (run) => {
      notifySuccess(`Started ${job.name}`)
      setExpanded(true)
      setOpenRun(run.id)
    },
    onError: (error) => notifyError(`Couldn't start ${job.name}`, error),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: cronJobsQuery(scope, service).queryKey }),
        queryClient.invalidateQueries({ queryKey: cronRunsQuery(scope, service, job.name).queryKey }),
      ]),
  })
  const last = job.last_run ? runStatuses[job.last_run.status] : undefined
  const runsId = `runs-${job.name}`

  return (
    <div className="border border-line bg-surface">
      <div className="flex flex-wrap items-start gap-x-4 gap-y-3 p-4">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium text-fg">{job.name}</span>
            <Code>{job.schedule}</Code>
            {last && job.last_run && (
              <Badge tone={last.tone} dot pulse={job.last_run.status === 'running'}>
                {last.label} {timeAgo(job.last_run.started_at)}
              </Badge>
            )}
          </div>
          <p className="mt-2 truncate font-mono text-xs text-fg-muted" title={job.command.join(' ')}>
            $ {job.command.join(' ')}
          </p>
          <p className="mt-1 flex items-center gap-1.5 text-xs text-fg-faint">
            <Clock aria-hidden className="size-3.5" />
            {job.next_run_at ? (
              <span title={dateTime(job.next_run_at)}>Next {timeAgo(job.next_run_at)}</span>
            ) : (
              'Not scheduled while the service is stopped'
            )}
            {' · '}stopped after {interval(job.timeout)}
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" loading={trigger.isPending} onClick={() => trigger.mutate()}>
            {!trigger.isPending && <Play aria-hidden className="size-4" />}
            Run now
          </Button>
          <Button
            variant="ghost"
            size="sm"
            aria-expanded={expanded}
            aria-controls={runsId}
            onClick={() => setExpanded(!expanded)}
          >
            History
            <ChevronDown aria-hidden className={`size-4 transition-transform ${expanded ? 'rotate-180' : ''}`} />
          </Button>
        </div>
      </div>
      {expanded && (
        <div id={runsId} className="border-t border-line">
          {runs.isPending ? (
            <div className="p-4">
              <Skeleton className="h-16 w-full" />
            </div>
          ) : runs.isError || runs.data.length === 0 ? (
            <p className="p-4 text-sm text-fg-faint">{runs.isError ? "Couldn't load runs." : 'No runs yet.'}</p>
          ) : (
            <ul className="divide-y divide-line">
              {runs.data.map((run) => {
                const status = runStatuses[run.status]
                return (
                  <li key={run.id}>
                    <button
                      type="button"
                      onClick={() => setOpenRun(run.id)}
                      className="flex w-full items-center gap-3 px-4 py-2 text-left text-ui transition-colors hover:bg-raised focus-visible:bg-raised focus-visible:outline-none"
                    >
                      <Badge tone={status.tone} dot pulse={run.status === 'running'}>
                        {status.label}
                      </Badge>
                      <span className="flex-1 text-fg-muted" title={dateTime(run.started_at)}>
                        {timeAgo(run.started_at)}
                      </span>
                      <span className="text-xs text-fg-faint">{run.trigger === 'manual' ? 'By hand' : 'Scheduled'}</span>
                      {run.finished_at != null && (
                        <span className="tabular w-14 text-right font-mono text-2xs text-fg-faint">
                          {Math.max(0, run.finished_at - run.started_at)}s
                        </span>
                      )}
                    </button>
                  </li>
                )
              })}
            </ul>
          )}
        </div>
      )}
      <RunOutputModal scope={scope} service={service} job={job.name} runId={openRun} onClose={() => setOpenRun(null)} />
    </div>
  )
}
