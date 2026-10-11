import { useMutation, useQueryClient } from '@tanstack/react-query'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { Alert } from '@/components/ui/alert'
import { errorMessage, notifySuccess } from '@/lib/toast'
import { MIN_PASSWORD_CHARS, password } from '@/lib/validation'
import { sessionQuery, type Session } from '../../auth/api'
import { setPassword } from '../api'

const schema = z
  .object({ current: z.string(), next: password, confirm: z.string() })
  .refine((values) => values.confirm === values.next, { message: "The passwords don't match.", path: ['confirm'] })

// the current password is only asked when there is one
const withCurrent = schema.refine((values) => values.current !== '', { message: 'Enter your current password.', path: ['current'] })

export function PasswordForm({ session }: { session: Session }) {
  const queryClient = useQueryClient()
  const mutation = useMutation({
    mutationFn: (values: z.output<typeof schema>) => setPassword(session.name, { current: values.current || undefined, new: values.next }),
    onSuccess: () => {
      notifySuccess('Password changed', 'Your other sessions were signed out.')
      form.reset()
      return queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey })
    },
  })
  const rules = session.has_password ? withCurrent : schema
  const form = useAppForm({
    defaultValues: { current: '', next: '', confirm: '' },
    validators: { onChange: rules },
    onSubmit: ({ value }) => mutation.mutateAsync(rules.parse(value)).catch(() => undefined),
  })

  return (
    <form.AppForm>
      <form.FormElement className="flex max-w-md flex-col gap-4">
        {mutation.isError && <Alert>{errorMessage(mutation.error)}</Alert>}
        {session.has_password && (
          <form.AppField name="current">{(field) => <field.PasswordField label="Current password" autoComplete="current-password" />}</form.AppField>
        )}
        <form.AppField name="next">
          {(field) => (
            <field.PasswordField
              label="New password"
              autoComplete="new-password"
              description={`At least ${MIN_PASSWORD_CHARS} characters. A few unrelated words are easy to remember and hard to guess.`}
            />
          )}
        </form.AppField>
        <form.AppField name="confirm">{(field) => <field.PasswordField label="Confirm new password" autoComplete="new-password" />}</form.AppField>
        <div>
          <form.SubmitButton>{session.has_password ? 'Change password' : 'Set password'}</form.SubmitButton>
        </div>
      </form.FormElement>
    </form.AppForm>
  )
}
