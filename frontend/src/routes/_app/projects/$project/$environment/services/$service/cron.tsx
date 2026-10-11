import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { CalendarClock } from 'lucide-react'
import { Code } from '@/components/ui/code'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { Skeleton } from '@/components/ui/skeleton'
import { cronJobsQuery } from '@/features/cron/api'
import { CronJobsTable } from '@/features/cron/components/cron-jobs-table'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/cron')({
  component: Cron,
})

const EXAMPLE = `[[cron]]
name = "report"
schedule = "0 6 * * MON"
command = ["bin/report"]`

function Cron() {
  const { scope, service } = useServiceScope()
  const jobs = useQuery(cronJobsQuery(scope, service))
  if (jobs.isPending) return <Skeleton className="h-40 w-full" />
  if (jobs.isError) return <ErrorState error={jobs.error} onRetry={() => void jobs.refetch()} />
  if (jobs.data.length === 0) {
    return (
      <EmptyState
        icon={CalendarClock}
        title="No cron jobs"
        description={
          <>
            Add them to the service&apos;s <Code>bird.toml</Code> and deploy from it:
            <pre className="mt-3 border border-line bg-sunken p-3 text-left font-mono text-xs text-fg">{EXAMPLE}</pre>
          </>
        }
      />
    )
  }
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-fg-muted">Schedules are in UTC. Jobs come from bird.toml, and each run is a fresh container.</p>
      <CronJobsTable scope={scope} service={service} jobs={jobs.data} />
    </div>
  )
}
