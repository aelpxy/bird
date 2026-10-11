import { KeyRound } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import { notifySuccess } from '@/lib/toast'
import { MIN_PASSWORD_CHARS, password } from '@/lib/validation'
import { resetPassword, type User } from '../api'

const schema = z.object({ password })

export function ResetPasswordDialog({ user, onDone }: { user: User; onDone: () => Promise<unknown> }) {
  const dialog = useDialogForm({
    schema,
    defaultValues: { password: '' },
    onSubmit: async (values) => {
      await resetPassword(user.name, values.password)
      notifySuccess(`Changed ${user.name}'s password`)
      await onDone()
    },
  })
  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button variant="secondary" size="sm">
          <KeyRound aria-hidden className="size-3.5" />
          {user.has_password ? 'Reset password' : 'Set password'}
        </Button>
      }
      title={`${user.has_password ? 'Reset' : 'Set'} ${user.name}'s password`}
      description="Their sessions are signed out. Share the new password with them privately."
      submitLabel="Save password"
    >
      <dialog.form.AppField name="password">
        {(field) => <field.PasswordField label="New password" description={`At least ${MIN_PASSWORD_CHARS} characters.`} autoComplete="new-password" autoFocus />}
      </dialog.form.AppField>
    </FormDialog>
  )
}
