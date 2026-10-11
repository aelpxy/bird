import { useQuery, useQueryClient } from '@tanstack/react-query'
import { KeyRound, Plus } from 'lucide-react'
import { useState } from 'react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Code } from '@/components/ui/code'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { FormDialog } from '@/components/ui/form-dialog'
import { SecretBox } from '@/components/ui/secret-box'
import { Skeleton } from '@/components/ui/skeleton'
import { dateTime, timeAgo } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { NAME_HINT, name } from '@/lib/validation'
import { createToken, revokeToken, tokensQuery, type IssuedToken } from '@/features/account/api'

const EXPIRY_OPTIONS = [
  { days: 30, label: '30 days' },
  { days: 90, label: '90 days' },
  { days: 365, label: '1 year' },
  { days: 0, label: 'Never' },
]

export function TokensPanel({ user }: { user: string }) {
  const queryClient = useQueryClient()
  const tokens = useQuery(tokensQuery(user))
  const [issued, setIssued] = useState<IssuedToken | null>(null)
  const tokenDialog = useDialogForm({
    schema: z.object({ name: name('Name the token.'), expiresInDays: z.number() }),
    defaultValues: { name: '', expiresInDays: 90 },
    onSubmit: async (values) => {
      const token = await createToken(user, { name: values.name, expires_in_days: values.expiresInDays || null })
      setIssued(token)
      await refresh()
    },
  })
  const refresh = () => queryClient.invalidateQueries({ queryKey: tokensQuery(user).queryKey })

  const newToken = (
    <FormDialog
      dialog={tokenDialog}
      trigger={
        <Button size="sm">
          <Plus aria-hidden className="size-3.5" />
          New token
        </Button>
      }
      title="New API token"
      description="For scripts and CI. A token acts as you, with your access."
      submitLabel="Create token"
    >
      <tokenDialog.form.AppField name="name">
        {(field) => <field.TextField label="Name" placeholder="ci" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </tokenDialog.form.AppField>
      <tokenDialog.form.AppField name="expiresInDays">
        {(field) => <field.SelectField label="Expires after" options={EXPIRY_OPTIONS.map((option) => ({ value: option.days, label: option.label }))} />}
      </tokenDialog.form.AppField>
    </FormDialog>
  )

  return (
    <div className="flex flex-col gap-4">
      {issued && (
        <SecretBox value={issued.token} title={`Copy your new token ${issued.name}`} onDismiss={() => setIssued(null)}>
          It is shown only this once. Use it with <Code>bird login {window.location.host}</Code> or as a bearer token.
        </SecretBox>
      )}
      {tokens.isPending ? (
        <Skeleton className="h-32 w-full" />
      ) : tokens.isError ? (
        <ErrorState error={tokens.error} onRetry={() => void tokens.refetch()} />
      ) : tokens.data.length === 0 ? (
        <EmptyState icon={KeyRound} title="No API tokens" description="Create one for the CLI, scripts or CI." action={newToken} />
      ) : (
        <>
          <div>{newToken}</div>
          <ul className="divide-y divide-line border border-line bg-surface">
            {tokens.data.map((token) => (
              <li key={token.name} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
                <KeyRound aria-hidden className="size-4 shrink-0 text-fg-faint" />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-fg">
                    {token.name} <span className="ml-1 font-mono text-xs text-fg-faint">{token.prefix}…</span>
                  </p>
                  <p className="mt-1 font-mono text-2xs text-fg-faint">
                    Created {timeAgo(token.created_at)} ·{' '}
                    {token.last_used_at ? `last used ${timeAgo(token.last_used_at)}` : 'never used'} ·{' '}
                    {token.expires_at ? (
                      <span title={dateTime(token.expires_at)}>expires {timeAgo(token.expires_at)}</span>
                    ) : (
                      'never expires'
                    )}
                  </p>
                </div>
                <ConfirmDialog
                  trigger={
                    <Button variant="secondary" size="sm">
                      Revoke
                    </Button>
                  }
                  title={`Revoke ${token.name}?`}
                  description="Anything using it stops working right away."
                  confirmLabel="Revoke token"
                  tone="danger"
                  onConfirm={async () => {
                    await revokeToken(user, token.name)
                    notifySuccess(`Revoked ${token.name}`)
                    await refresh()
                  }}
                />
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  )
}
