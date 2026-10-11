import { useQueryClient } from '@tanstack/react-query'
import { ArchiveRestore, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CheckboxField } from '@/components/ui/checkbox-field'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import type { Scope } from '@/lib/api/paths'
import { bytes, dateTime, timeAgo } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { removeBackup, restoreBackup, type Backup } from '@/features/backups/api'

const triggers: Record<Backup['trigger'], string> = {
  manual: 'Manual',
  scheduled: 'Scheduled',
  restore: 'Before a restore',
}

type BackupsListProps = { scope: Scope; service: string; backups: Backup[] }

export function BackupsList({ scope, service, backups }: BackupsListProps) {
  const queryClient = useQueryClient()
  const refresh = () => queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] })
  return (
    <ul className="divide-y divide-line border border-line bg-surface">
      {backups.map((backup) => {
        const total = backup.volumes.reduce((sum, volume) => sum + volume.size_bytes, 0)
        return (
          <li key={backup.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-medium text-fg" title={dateTime(backup.created_at)}>
                  {timeAgo(backup.created_at)}
                </span>
                <Badge>{triggers[backup.trigger]}</Badge>
              </div>
              <p className="mt-1 font-mono text-2xs text-fg-faint">
                {backup.volumes.map((volume) => `${volume.name} ${bytes(volume.size_bytes)}`).join(' · ')} · {bytes(total)}{' '}
                total · stored in {backup.storage}
              </p>
            </div>
            <div className="flex gap-2">
              <RestoreButton scope={scope} service={service} backup={backup} onDone={refresh} />
              <ConfirmDialog
                trigger={
                  <Button variant="secondary" size="sm" aria-label={`Delete the backup from ${timeAgo(backup.created_at)}`}>
                    <Trash2 aria-hidden className="size-3.5" />
                  </Button>
                }
                title="Delete this backup?"
                description={`The copy from ${dateTime(backup.created_at)} is deleted from storage.`}
                confirmLabel="Delete backup"
                tone="danger"
                onConfirm={async () => {
                  await removeBackup(scope, service, backup.id)
                  notifySuccess('Backup deleted')
                  await refresh()
                }}
              />
            </div>
          </li>
        )
      })}
    </ul>
  )
}

type RestoreButtonProps = { scope: Scope; service: string; backup: Backup; onDone: () => Promise<unknown> }

function RestoreButton({ scope, service, backup, onDone }: RestoreButtonProps) {
  const [allowImageChange, setAllowImageChange] = useState(false)
  return (
    <ConfirmDialog
      trigger={
        <Button variant="secondary" size="sm">
          <ArchiveRestore aria-hidden className="size-3.5" />
          Restore
        </Button>
      }
      title={`Restore ${service} to ${timeAgo(backup.created_at)}?`}
      description={`Its volumes are replaced with this copy and the app restarts. The data it has now is saved as a backup first, so you can go back.`}
      confirmLabel="Restore"
      tone="danger"
      onConfirm={async () => {
        await restoreBackup(scope, service, backup.id, allowImageChange)
        notifySuccess(`Restored ${service}`, 'The data from before is kept as a backup.')
        await onDone()
      }}
    >
      <CheckboxField
        checked={allowImageChange}
        onCheckedChange={setAllowImageChange}
        tone="danger"
        label="Allow a different image"
        description="Only if you know the image running now can read data from the one that made this backup."
      />
    </ConfirmDialog>
  )
}
