import { useQuery } from '@tanstack/react-query'
import { Badge } from '@/components/ui/badge'
import { ErrorState } from '@/components/ui/error-state'
import { Modal } from '@/components/ui/modal'
import { Skeleton } from '@/components/ui/skeleton'
import type { Scope } from '@/lib/api/paths'
import { dateTime } from '@/lib/format'
import { cronRunQuery } from '@/features/cron/api'
import { runStatuses } from '@/features/cron/run-status'

type RunOutputModalProps = {
  scope: Scope
  service: string
  job: string
  runId: string | null
  onClose: () => void
}

export function RunOutputModal({ scope, service, job, runId, onClose }: RunOutputModalProps) {
  const run = useQuery({ ...cronRunQuery(scope, service, job, runId ?? ''), enabled: runId !== null })
  const status = run.data ? runStatuses[run.data.status] : undefined
  return (
    <Modal
      open={runId !== null}
      onOpenChange={(open) => !open && onClose()}
      title={`${job} run`}
      description={run.data ? `Started ${dateTime(run.data.started_at)} · ${run.data.trigger === 'manual' ? 'run by hand' : 'on schedule'}` : undefined}
    >
      {run.isPending ? (
        <Skeleton className="h-48 w-full" />
      ) : run.isError ? (
        <ErrorState error={run.error} onRetry={() => void run.refetch()} />
      ) : (
        <>
          <div className="flex items-center gap-2">
            {status && (
              <Badge tone={status.tone} dot pulse={run.data.status === 'running'}>
                {status.label}
              </Badge>
            )}
            {run.data.exit_code != null && (
              <span className="font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase">
                exit <span className="text-fg">{run.data.exit_code}</span>
              </span>
            )}
          </div>
          <pre
            aria-label="Output"
            tabIndex={0}
            className="min-h-32 overflow-auto border border-line bg-sunken p-3 font-mono text-xs leading-5 whitespace-pre-wrap text-fg focus-visible:outline focus-visible:outline-1 focus-visible:outline-fg"
          >
            {run.data.output || (run.data.status === 'running' ? 'Waiting for output…' : 'No output.')}
          </pre>
          <p className="text-xs text-fg-faint">The last 64 KB of output are kept.</p>
        </>
      )}
    </Modal>
  )
}
