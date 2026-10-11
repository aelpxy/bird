import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Archive, Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { notifyError, notifySuccess } from '@/lib/toast'
import { backupsQuery, createBackup } from '@/features/backups/api'
import { BackupsList } from '@/features/backups/components/backups-list'
import { SchedulePanel } from '@/features/backups/components/schedule-panel'
import { serviceQuery } from '@/features/services/api'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/backups')({
  component: Backups,
})

function Backups() {
  const { scope, service: name } = useServiceScope()
  const queryClient = useQueryClient()
  const service = useQuery(serviceQuery(scope, name))
  const backups = useQuery(backupsQuery(scope, name))
  const create = useMutation({
    mutationFn: () => createBackup(scope, name),
    onSuccess: () => notifySuccess(`Backed up ${name}`),
    onError: (error) => notifyError(`Couldn't back up ${name}`, error),
    onSettled: () => queryClient.invalidateQueries({ queryKey: backupsQuery(scope, name).queryKey }),
  })

  if (!service.data) return <Skeleton className="h-48 w-full" />
  if (service.data.volumes.length === 0) {
    return <EmptyState icon={Archive} title="Nothing to back up" description={`${name} has no volumes, so it keeps no data of its own.`} />
  }
  const backUpNow = (
    <Button size="sm" loading={create.isPending} onClick={() => create.mutate()}>
      {!create.isPending && <Plus aria-hidden className="size-4" />}
      Back up now
    </Button>
  )
  return (
    <div className="flex max-w-3xl flex-col gap-10">
      <Section title="Schedule" description="Machines pause for a moment while their volumes are copied, so the copy is consistent.">
        <SchedulePanel key={JSON.stringify(service.data.backup_schedule)} scope={scope} service={service.data} />
      </Section>
      <Section title="Backups">
        {backups.isPending ? (
          <Skeleton className="h-32 w-full" />
        ) : backups.isError ? (
          <ErrorState error={backups.error} onRetry={() => void backups.refetch()} />
        ) : backups.data.length === 0 ? (
          <EmptyState icon={Archive} title="No backups yet" action={backUpNow} />
        ) : (
          <>
            <div>{backUpNow}</div>
            <BackupsList scope={scope} service={name} backups={backups.data} />
          </>
        )}
      </Section>
    </div>
  )
}
