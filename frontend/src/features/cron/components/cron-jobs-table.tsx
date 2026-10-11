import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ChevronDown, Play } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Code } from '@/components/ui/code'
import { IconButton } from '@/components/ui/icon-button'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableCell, TableRow } from '@/components/ui/table'
import type { Scope } from '@/lib/api/paths'
import { dateTime, interval, timeAgo } from '@/lib/format'
import { notifyError, notifySuccess } from '@/lib/toast'
import { cronJobsQuery, cronRunsQuery, triggerRun, type CronJob } from '@/features/cron/api'
import { runStatuses } from '@/features/cron/run-status'
import { RunOutputModal } from './run-output-modal'

const LAYOUT = 'sm:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)_6.5rem_9rem_auto]'
const COLUMNS = [{ label: 'Job' }, { label: 'Schedule' }, { label: 'Next run' }, { label: 'Last run' }, { label: '' }]

export function CronJobsTable({ scope, service, jobs }: { scope: Scope; service: string; jobs: CronJob[] }) {
  return (
    <Table label="Cron jobs" columns={COLUMNS} layout={LAYOUT}>
      {jobs.map((job) => (
        <CronJobRow key={job.name} scope={scope} service={service} job={job} />
      ))}
    </Table>
  )
}

function CronJobRow({ scope, service, job }: { scope: Scope; service: string; job: CronJob }) {
  const queryClient = useQueryClient()
  const [expanded, setExpanded] = useState(false)
  const [openRun, setOpenRun] = useState<string | null>(null)
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
    <>
      <TableRow layout={LAYOUT}>
        <TableCell>
          <p className="truncate text-sm font-medium text-fg">{job.name}</p>
          <p className="mt-0.5 truncate font-mono text-2xs text-fg-faint" title={job.command.join(' ')}>
            $ {job.command.join(' ')}
          </p>
        </TableCell>
        <TableCell className="flex items-center justify-end gap-1 sm:order-last">
          <Button variant="secondary" size="sm" loading={trigger.isPending} onClick={() => trigger.mutate()}>
            {!trigger.isPending && <Play aria-hidden className="size-3.5" />}
            Run
          </Button>
          <IconButton
            label={expanded ? `Hide the runs of ${job.name}` : `Show the runs of ${job.name}`}
            aria-expanded={expanded}
            aria-controls={runsId}
            onClick={() => setExpanded(!expanded)}
          >
            <ChevronDown aria-hidden className={`size-4 transition-transform ${expanded ? 'rotate-180' : ''}`} />
          </IconButton>
        </TableCell>
        <TableCell label="Schedule">
          <span className="flex flex-col items-start gap-0.5">
            <Code>{job.schedule}</Code>
            <span className="text-2xs text-fg-faint">stops after {interval(job.timeout)}</span>
          </span>
        </TableCell>
        <TableCell label="Next run" className="text-xs text-fg-muted">
          {job.next_run_at ? <span title={dateTime(job.next_run_at)}>{timeAgo(job.next_run_at)}</span> : <span title="Not scheduled while the service is stopped">Paused</span>}
        </TableCell>
        <TableCell label="Last run">
          {last && job.last_run ? (
            <Badge tone={last.tone} dot pulse={job.last_run.status === 'running'}>
              {last.label} {timeAgo(job.last_run.started_at)}
            </Badge>
          ) : (
            <span className="text-xs text-fg-faint">Never</span>
          )}
        </TableCell>
      </TableRow>
      {expanded && <RunHistory id={runsId} scope={scope} service={service} job={job.name} onOpen={setOpenRun} />}
      <RunOutputModal scope={scope} service={service} job={job.name} runId={openRun} onClose={() => setOpenRun(null)} />
    </>
  )
}

type RunHistoryProps = { id: string; scope: Scope; service: string; job: string; onOpen: (run: string) => void }

function RunHistory({ id, scope, service, job, onOpen }: RunHistoryProps) {
  const runs = useQuery(cronRunsQuery(scope, service, job))
  return (
    <div id={id} role="row" className="bg-sunken">
      <div role="cell">
        {runs.isPending ? (
          <div className="p-4">
            <Skeleton className="h-16 w-full" />
          </div>
        ) : runs.isError || runs.data.length === 0 ? (
          <p className="px-4 py-3 text-sm text-fg-faint">{runs.isError ? "Couldn't load runs." : 'No runs yet.'}</p>
        ) : (
          <ul aria-label={`Runs of ${job}`} className="divide-y divide-line">
            {runs.data.map((run) => {
              const status = runStatuses[run.status]
              return (
                <li key={run.id}>
                  <button
                    type="button"
                    onClick={() => onOpen(run.id)}
                    className="flex w-full items-center gap-3 px-4 py-2 pl-8 text-left text-ui transition-colors hover:bg-raised focus-visible:bg-raised focus-visible:outline-none"
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
    </div>
  )
}
