import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Archive, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { focusRing } from '@/components/ui/control-styles'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { Eyebrow } from '@/components/ui/eyebrow'
import { IconButton } from '@/components/ui/icon-button'
import { Skeleton } from '@/components/ui/skeleton'
import type { Scope } from '@/lib/api/paths'
import { bytes, dateTime, plural, timeAgo } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { servicesQuery } from '@/features/services/api'
import { environmentBackupsQuery, removeBackup, type Backup } from '../api'

const triggers: Record<Backup['trigger'], string> = { manual: 'Manual', scheduled: 'Scheduled', restore: 'Before a restore' }

// one environment's backups grouped by service; a removed service's can only be deleted from here
export function EnvironmentBackups({ scope }: { scope: Scope }) {
  const queryClient = useQueryClient()
  const backups = useQuery(environmentBackupsQuery(scope))
  const services = useQuery(servicesQuery(scope))
  const refresh = () => queryClient.invalidateQueries({ queryKey: ['backups', scope.project, scope.environment] })

  if (backups.isPending || services.isPending) return <Skeleton className="h-32 w-full" />
  if (backups.isError) return <ErrorState error={backups.error} onRetry={() => void backups.refetch()} />
  if (backups.data.length === 0) return <EmptyState icon={Archive} title={`No backups in ${scope.environment}`} />

  const live = new Set((services.data ?? []).map((service) => service.name))
  const groups = new Map<string, Backup[]>()
  for (const backup of backups.data) groups.set(backup.service, [...(groups.get(backup.service) ?? []), backup])

  async function removeAll(service: string, items: Backup[]) {
    for (const backup of items) await removeBackup(scope, service, backup.id)
    notifySuccess(`Deleted ${plural(items.length, 'backup')} of ${service}`)
    await refresh()
  }

  return (
    <div className="flex flex-col gap-4">
      {[...groups].map(([service, items]) => {
        const removed = !live.has(service)
        const total = items.reduce((sum, backup) => sum + backup.volumes.reduce((size, volume) => size + volume.size_bytes, 0), 0)
        return (
          <div key={service} className="border border-line bg-surface">
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-4 py-2.5">
              <div className="flex flex-wrap items-center gap-2">
                {removed ? (
                  <span className="font-mono text-ui font-medium text-fg">{service}</span>
                ) : (
                  <Link
                    to="/projects/$project/$environment/services/$service/backups"
                    params={{ ...scope, service }}
                    className={`font-mono text-ui font-medium text-fg underline-offset-4 hover:underline ${focusRing}`}
                  >
                    {service}
                  </Link>
                )}
                {removed && <Badge tone="warn">Service removed</Badge>}
                <Eyebrow as="span">
                  {plural(items.length, 'backup')} · {bytes(total)}
                </Eyebrow>
              </div>
              {removed && (
                <ConfirmDialog
                  trigger={
                    <Button variant="danger" size="sm">
                      <Trash2 aria-hidden className="size-3.5" />
                      Delete all
                    </Button>
                  }
                  title={`Delete every backup of ${service}?`}
                  description={`${plural(items.length, 'backup')} of a service that no longer exists are deleted from storage. Nothing can restore them afterwards.`}
                  confirmLabel="Delete all"
                  tone="danger"
                  confirmText={service}
                  onConfirm={() => removeAll(service, items)}
                />
              )}
            </div>
            <ul className="divide-y divide-line">
              {items.map((backup) => (
                <li key={backup.id} className="flex items-center gap-4 px-4 py-2.5">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-ui text-fg" title={dateTime(backup.created_at)}>
                        {timeAgo(backup.created_at)}
                      </span>
                      <Badge>{triggers[backup.trigger]}</Badge>
                    </div>
                    <p className="mt-0.5 font-mono text-2xs text-fg-faint">
                      {backup.volumes.map((volume) => `${volume.name} ${bytes(volume.size_bytes)}`).join(' · ')} · {backup.storage}
                    </p>
                  </div>
                  <ConfirmDialog
                    trigger={
                      <IconButton label={`Delete the backup from ${timeAgo(backup.created_at)}`} tone="danger">
                        <Trash2 aria-hidden className="size-4" />
                      </IconButton>
                    }
                    title="Delete this backup?"
                    description={`The copy of ${service} from ${dateTime(backup.created_at)} is deleted from storage.`}
                    confirmLabel="Delete backup"
                    tone="danger"
                    onConfirm={async () => {
                      await removeBackup(scope, service, backup.id)
                      notifySuccess('Backup deleted')
                      await refresh()
                    }}
                  />
                </li>
              ))}
            </ul>
          </div>
        )
      })}
    </div>
  )
}
