import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import { ErrorState } from '@/components/ui/error-state'
import { PageHeader } from '@/components/ui/page-header'
import { SecretBox } from '@/components/ui/secret-box'
import { Skeleton } from '@/components/ui/skeleton'
import { NewUserDialog } from '@/features/users/components/new-user-dialog'
import { UsersTable } from '@/features/users/components/users-table'
import { usersQuery } from '@/features/users/api'

export const Route = createFileRoute('/_app/admin/users/')({
  head: () => ({ meta: [{ title: 'Users · bird' }] }),
  component: Users,
})

function Users() {
  const { session } = Route.useRouteContext()
  const users = useQuery(usersQuery)
  const [created, setCreated] = useState<{ name: string; token: string } | null>(null)

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        eyebrow="Server"
        title="Users"
        description="Everyone who can use this server."
        actions={<NewUserDialog onCreated={(name, token) => setCreated({ name, token })} />}
      />
      {created && (
        <SecretBox value={created.token} title={`${created.name}'s API token`} onDismiss={() => setCreated(null)}>
          Shown only this once. Send it to them privately, or set a password for them below.
        </SecretBox>
      )}
      {users.isPending ? (
        <Skeleton className="h-48 w-full" />
      ) : users.isError ? (
        <ErrorState error={users.error} onRetry={() => void users.refetch()} />
      ) : (
        <UsersTable users={users.data} currentUser={session.name} />
      )}
    </div>
  )
}
