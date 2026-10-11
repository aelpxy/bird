import { useMutation, useQueryClient } from '@tanstack/react-query'
import { ShieldCheck, ShieldOff, ShieldQuestion } from 'lucide-react'
import { useState } from 'react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Alert } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import { errorMessage, notifySuccess } from '@/lib/toast'
import { required } from '@/lib/validation'
import { sessionQuery, type Session } from '@/features/auth/api'
import { disableTwoFactor, startTwoFactor } from '../api'
import { RecoveryCodes } from './recovery-codes'
import { SetupSteps } from './setup-steps'
import { TwoFactorSetup } from './two-factor-setup'

type Step = { kind: 'idle' } | { kind: 'scan'; secret: string; uri: string } | { kind: 'codes'; codes: string[] }

export function TwoFactorPanel({ session }: { session: Session }) {
  const queryClient = useQueryClient()
  const [step, setStep] = useState<Step>({ kind: 'idle' })
  const refresh = () => queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey })
  const start = useMutation({
    mutationFn: () => startTwoFactor(session.name),
    onSuccess: (setup) => setStep({ kind: 'scan', ...setup }),
  })
  const disable = useDialogForm({
    schema: z.object({ password: required('Enter your password.') }),
    defaultValues: { password: '' },
    onSubmit: async (values) => {
      await disableTwoFactor(session.name, values.password)
      notifySuccess('Two-factor authentication is off')
      await refresh()
    },
  })

  if (step.kind !== 'idle') {
    return (
      <div className="flex flex-col gap-6">
        <SetupSteps current={step.kind === 'scan' ? 0 : 2} />
        {step.kind === 'scan' ? (
          <TwoFactorSetup
            user={session.name}
            secret={step.secret}
            uri={step.uri}
            onConfirmed={(codes) => {
              setStep({ kind: 'codes', codes })
              void refresh()
            }}
            onCancel={() => setStep({ kind: 'idle' })}
          />
        ) : (
          <RecoveryCodes
            user={session.name}
            codes={step.codes}
            onDone={() => {
              setStep({ kind: 'idle' })
              notifySuccess('Two-factor authentication is on', 'Signing in now asks for a code from your app.')
            }}
          />
        )}
      </div>
    )
  }

  const on = session.two_factor === true
  const Icon = on ? ShieldCheck : session.has_password ? ShieldOff : ShieldQuestion
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-5 border border-line bg-surface p-5 sm:flex-row sm:items-start">
        <span className={`grid size-11 shrink-0 place-items-center border ${on ? 'border-ok/50 text-ok' : 'border-line-strong text-fg-muted'}`}>
          <Icon aria-hidden className="size-5" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex flex-wrap items-center gap-2.5">
            <h3 className="text-title font-semibold text-fg">Two-factor authentication</h3>
            <Badge tone={on ? 'ok' : 'neutral'} dot>
              {on ? 'On' : 'Off'}
            </Badge>
          </div>
          <p className="max-w-xl text-sm text-fg-muted">
            {on
              ? 'Signing in asks for your password and a code from your authenticator app. If you lose the phone, one of your recovery codes signs you in once.'
              : 'Adds a code from an app on your phone to signing in, so a leaked password alone cannot open your account or the services you run.'}
          </p>
          {start.isError && <Alert>{errorMessage(start.error)}</Alert>}
          {!session.has_password && <p className="text-sm text-warn">Set a password first: two-factor protects signing in with one.</p>}
        </div>
        <div className="shrink-0">
          {on ? (
            <FormDialog
              dialog={disable}
              trigger={
                <Button variant="secondary">
                  <ShieldOff aria-hidden className="size-3.5" />
                  Turn off
                </Button>
              }
              title="Turn off two-factor?"
              description="Your password alone will sign you in, and your recovery codes stop working."
              submitLabel="Turn off"
            >
              <disable.form.AppField name="password">{(field) => <field.PasswordField label="Password" autoComplete="current-password" autoFocus />}</disable.form.AppField>
            </FormDialog>
          ) : (
            <Button loading={start.isPending} disabled={!session.has_password} onClick={() => start.mutate()}>
              {!start.isPending && <ShieldCheck aria-hidden className="size-3.5" />}
              Set up
            </Button>
          )}
        </div>
      </div>
      {!on && session.has_password && (
        <ol className="grid gap-px border border-line bg-line sm:grid-cols-3">
          {[
            ['Scan', 'Point your authenticator app at a QR code.'],
            ['Verify', 'Type the 6-digit code it shows to prove it worked.'],
            ['Save codes', 'Keep ten recovery codes for a lost phone.'],
          ].map(([title, text], index) => (
            <li key={title} className="flex flex-col gap-1.5 bg-surface p-4">
              <span className="font-mono text-2xs tracking-[0.08em] text-fg-faint">0{index + 1}</span>
              <span className="text-ui font-medium text-fg">{title}</span>
              <span className="text-xs text-fg-faint">{text}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}
