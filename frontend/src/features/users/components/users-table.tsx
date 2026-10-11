import { useQueryClient } from '@tanstack/react-query'
import { Trash2 } from 'lucide-react'
import { Avatar } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { focusRing } from '@/components/ui/control-styles'
import { IconButton } from '@/components/ui/icon-button'
import { timeAgo } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { Link } from '@tanstack/react-router'
import { removeUser, usersQuery, type User } from '@/features/users/api'
import { ResetPasswordDialog } from './reset-password-dialog'

export function UsersTable({ users, currentUser }: { users: User[]; currentUser: string }) {
  const queryClient = useQueryClient()
  const refresh = () => queryClient.invalidateQueries({ queryKey: usersQuery.queryKey })

  return (
    <ul className="divide-y divide-line border border-line bg-surface">
      {users.map((user) => (
        <li key={user.name} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
          <Avatar name={user.name} />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Link
                to="/admin/users/$user"
                params={{ user: user.name }}
                className={`font-medium text-fg underline-offset-4 hover:underline ${focusRing}`}
              >
                {user.name}
              </Link>
              {user.name === currentUser && <span className="text-xs text-fg-faint">(you)</span>}
              <Badge tone={user.role === 'admin' ? 'info' : 'neutral'}>{user.role}</Badge>
              {user.two_factor && <Badge tone="ok">Two-factor</Badge>}
            </div>
            <p className="mt-1 font-mono text-2xs text-fg-faint">
              {user.has_password ? 'Signs in with a password' : 'API tokens only'} · added {timeAgo(user.created_at)}
            </p>
          </div>
          <div className="flex gap-2">
            {user.name !== currentUser && (
              <>
                <ResetPasswordDialog user={user} onDone={refresh} />
                <ConfirmDialog
                  trigger={
                    <IconButton label={`Remove ${user.name}`} tone="danger">
                      <Trash2 aria-hidden className="size-4" />
                    </IconButton>
                  }
                  title={`Remove ${user.name}?`}
                  description="Their sessions and API tokens stop working. birdd refuses while they are an org's last owner."
                  confirmLabel="Remove user"
                  tone="danger"
                  confirmText={user.name}
                  onConfirm={async () => {
                    await removeUser(user.name)
                    notifySuccess(`Removed ${user.name}`)
                    await refresh()
                  }}
                />
              </>
            )}
          </div>
        </li>
      ))}
    </ul>
  )
}
