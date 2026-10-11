import { ShieldOff } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { Panel } from '@/components/ui/panel'
import { Section } from '@/components/ui/section'
import { notifySuccess } from '@/lib/toast'
import { disableTwoFactor } from '@/features/account/api'
import { ResetPasswordDialog } from '@/features/users/components/reset-password-dialog'
import { Link, createFileRoute } from '@tanstack/react-router'
import { useUserDetail } from '@/features/users/use-user-detail'

export const Route = createFileRoute('/_app/admin/users/$user/')({ component: SignIn })

function SignIn() {
  const { user, isSelf, refresh } = useUserDetail()
  if (!user) return null
  return (
      <Section title="Sign-in">
        <Panel className="flex flex-wrap items-center justify-between gap-4">
          <div className="text-sm">
            <p className="text-fg">{user.has_password ? 'Has a password' : 'No password; API tokens only'}</p>
            <p className="mt-0.5 text-fg-muted">Two-factor is {user.two_factor ? 'on' : 'off'}.</p>
          </div>
          <div className="flex flex-wrap gap-2">
            {!isSelf && <ResetPasswordDialog user={user} onDone={refresh} />}
            {user.two_factor && !isSelf && (
              <ConfirmDialog
                trigger={
                  <Button variant="secondary" size="sm">
                    <ShieldOff aria-hidden className="size-4" />
                    Turn off two-factor
                  </Button>
                }
                title={`Turn off two-factor for ${user.name}?`}
                description="For someone who lost their phone and their recovery codes. Their password alone will sign them in."
                confirmLabel="Turn off"
                tone="danger"
                onConfirm={async () => {
                  await disableTwoFactor(user.name)
                  notifySuccess(`Two-factor is off for ${user.name}`)
                  await refresh()
                }}
              />
            )}
            {isSelf && (
              <Link to="/account" className="text-sm text-fg-muted underline decoration-line-strong underline-offset-4 hover:text-fg">
                Manage in your account
              </Link>
            )}
          </div>
        </Panel>
      </Section>
  )
}
