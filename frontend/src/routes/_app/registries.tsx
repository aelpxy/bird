import { useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { PackageOpen, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { EmptyState } from '@/components/ui/empty-state'
import { IconButton } from '@/components/ui/icon-button'
import { ErrorState } from '@/components/ui/error-state'
import { PageHeader } from '@/components/ui/page-header'
import { Skeleton } from '@/components/ui/skeleton'
import { notifySuccess } from '@/lib/toast'
import { logOutOfRegistry, registriesQuery } from '@/features/registries/api'
import { AddRegistryDialog } from '@/features/registries/components/add-registry-dialog'

export const Route = createFileRoute('/_app/registries')({
  head: () => ({ meta: [{ title: 'Registries · bird' }] }),
  component: Registries,
})

function Registries() {
  const { session } = Route.useRouteContext()
  const queryClient = useQueryClient()
  const registries = useQuery(registriesQuery)
  const isAdmin = session.role === 'admin'

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        eyebrow="Server"
        title="Registries"
        description="Credentials for pulling private images. They apply to every project on this server."
        actions={isAdmin ? <AddRegistryDialog /> : undefined}
      />
      {registries.isPending ? (
        <Skeleton className="h-32 w-full" />
      ) : registries.isError ? (
        <ErrorState error={registries.error} onRetry={() => void registries.refetch()} />
      ) : registries.data.length === 0 ? (
        <EmptyState
          icon={PackageOpen}
          title="No private registries"
          description={isAdmin ? 'Public images need nothing here.' : 'Public images need nothing here. A server admin can add credentials.'}
          action={isAdmin ? <AddRegistryDialog /> : undefined}
        />
      ) : (
        <ul className="divide-y divide-line border border-line bg-surface">
          {registries.data.map((registry) => (
            <li key={registry.host} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-sm text-fg">{registry.host}</span>
                  {registry.insecure && <Badge tone="warn">No TLS check</Badge>}
                </div>
                <p className="mt-0.5 font-mono text-2xs text-fg-faint">as {registry.username}</p>
              </div>
              {isAdmin && (
                <ConfirmDialog
                  trigger={
                    <IconButton label={`Remove ${registry.host}`} tone="danger">
                      <Trash2 aria-hidden className="size-4" />
                    </IconButton>
                  }
                  title={`Remove ${registry.host}?`}
                  description="Running machines keep their images, but deploys from this host fail until credentials are added again."
                  confirmLabel="Remove"
                  tone="danger"
                  onConfirm={async () => {
                    await logOutOfRegistry(registry.host)
                    notifySuccess(`Removed ${registry.host}`)
                    await queryClient.invalidateQueries({ queryKey: registriesQuery.queryKey })
                  }}
                />
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
