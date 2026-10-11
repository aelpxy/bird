import { useMutation } from '@tanstack/react-query'
import { QRCodeSVG } from 'qrcode.react'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { Alert } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { CopyButton } from '@/components/ui/copy-button'
import { Eyebrow } from '@/components/ui/eyebrow'
import { errorMessage } from '@/lib/toast'
import { confirmTwoFactor } from '../api'

const schema = z.object({ code: z.string().regex(/^\d{6}$/, 'Enter the 6 digits.') })

type TwoFactorSetupProps = {
  user: string
  secret: string
  uri: string
  onConfirmed: (codes: string[]) => void
  onCancel: () => void
}

// scanning and the first code on one screen, since the code proves the scan worked
export function TwoFactorSetup({ user, secret, uri, onConfirmed, onCancel }: TwoFactorSetupProps) {
  const confirm = useMutation({
    mutationFn: (code: string) => confirmTwoFactor(user, code),
    onSuccess: (result) => onConfirmed(result.codes),
    // a wrong code empties the boxes for the next try
    onError: () => form.reset(),
  })
  const form = useAppForm({
    defaultValues: { code: '' },
    validators: { onSubmit: schema },
    onSubmit: ({ value }) => confirm.mutateAsync(value.code).then(() => undefined, () => undefined),
  })
  const grouped = secret.match(/.{1,4}/g)?.join(' ') ?? secret

  return (
    <div className="grid gap-px border border-line bg-line md:grid-cols-[auto_minmax(0,1fr)]">
      <div className="flex flex-col items-center gap-4 bg-surface p-6">
        <Eyebrow>01 · Scan</Eyebrow>
        <div className="bg-fg p-3">
          <QRCodeSVG value={uri} size={176} role="img" aria-label="QR code for your authenticator app" />
        </div>
        <p className="max-w-52 text-center text-xs text-fg-faint">With 1Password, Google Authenticator, Authy or any TOTP app.</p>
      </div>
      <div className="flex flex-col gap-6 bg-surface p-6">
        <div className="flex flex-col gap-2">
          <Eyebrow>Can&apos;t scan it?</Eyebrow>
          <p className="text-sm text-fg-muted">Add an account by hand with this key, time-based, 6 digits.</p>
          <div className="flex flex-wrap items-center gap-2">
            <code className="border border-line bg-sunken px-3 py-1.5 font-mono text-sm tracking-[0.08em] text-fg select-all">{grouped}</code>
            <CopyButton value={secret} label="Copy key" />
          </div>
        </div>
        <form.AppForm>
          <form.FormElement className="flex flex-col gap-4 border-t border-line pt-6">
            <Eyebrow>02 · Verify</Eyebrow>
            {confirm.isError && <Alert>{errorMessage(confirm.error)}</Alert>}
            <div className="max-w-xs">
              <form.AppField name="code">
                {(field) => <field.CodeField label="Code from the app" description="It changes every 30 seconds." disabled={confirm.isPending} />}
              </form.AppField>
            </div>
            <div className="flex gap-2">
              <form.SubmitButton>Turn on two-factor</form.SubmitButton>
              <Button type="button" variant="ghost" onClick={onCancel}>
                Cancel
              </Button>
            </div>
          </form.FormElement>
        </form.AppForm>
      </div>
    </div>
  )
}
