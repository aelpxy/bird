import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { CheckboxField } from '@/components/ui/checkbox-field'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { DangerZone } from '@/components/ui/danger-zone'
import type { Scope } from '@/lib/api/paths'
import { notifySuccess } from '@/lib/toast'
import { removeService, type Service } from '@/features/services/api'

export function DeleteService({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const [purge, setPurge] = useState(false)
  const hasVolumes = service.volumes.length > 0

  async function remove() {
    await removeService(scope, service.name, purge)
    queryClient.removeQueries({ queryKey: ['services', scope.project, scope.environment, service.name] })
    await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment] })
    notifySuccess(`Deleted ${service.name}`)
    await navigate({ to: '/projects/$project/$environment', params: scope })
  }

  return (
    <DangerZone
      title="Delete this service"
      description="Stops its machines and removes its domains, variables and history. Backups are kept."
      action={
        <ConfirmDialog
          trigger={
            <Button variant="danger" size="sm">
              <Trash2 aria-hidden className="size-3.5" />
              Delete service
            </Button>
          }
          title={`Delete ${service.name}?`}
          description="This cannot be undone."
          confirmLabel="Delete service"
          tone="danger"
          confirmText={service.name}
          onConfirm={remove}
        >
          {hasVolumes && (
            <CheckboxField
              checked={purge}
              onCheckedChange={setPurge}
              tone="danger"
              label="Also delete its volumes"
              description="Without this, birdd refuses to delete a service that still has data."
            />
          )}
        </ConfirmDialog>
      }
    />
  )
}
