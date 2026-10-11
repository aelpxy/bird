import { useNavigate } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { DangerZone } from '@/components/ui/danger-zone'
import { Section } from '@/components/ui/section'
import { notifySuccess } from '@/lib/toast'
import { removeUser } from '@/features/users/api'
import { createFileRoute } from '@tanstack/react-router'
import { useUserDetail } from '@/features/users/use-user-detail'

export const Route = createFileRoute('/_app/admin/users/$user/danger')({ component: Danger })

function Danger() {
  const { user, refresh } = useUserDetail()
  const navigate = useNavigate()
  if (!user) return null
  return (
      <Section title="Danger zone">
        <DangerZone
          title={`Remove ${user.name}`}
          description={<>Their sessions and tokens stop working. birdd refuses while they are an org&apos;s last owner.</>}
          action={
          <ConfirmDialog
            trigger={
              <Button variant="danger" size="sm">
                <Trash2 aria-hidden className="size-4" />
                Remove user
              </Button>
            }
            title={`Remove ${user.name}?`}
            description="This cannot be undone."
            confirmLabel="Remove user"
            tone="danger"
            confirmText={user.name}
            onConfirm={async () => {
              await removeUser(user.name)
              notifySuccess(`Removed ${user.name}`)
              await refresh()
              await navigate({ to: '/admin/users' })
            }}
          />
          }
        />
      </Section>
  )
}
