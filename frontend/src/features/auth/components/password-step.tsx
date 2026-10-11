import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { required } from '@/lib/validation'

const schema = z.object({
  username: z.string().trim().toLowerCase().min(1, 'Enter your username.'),
  password: required('Enter your password.'),
})

type PasswordStepProps = {
  defaultUsername: string
  onSubmit: (username: string, password: string) => Promise<void>
}

export function PasswordStep({ defaultUsername, onSubmit }: PasswordStepProps) {
  const form = useAppForm({
    defaultValues: { username: defaultUsername, password: '' },
    validators: { onSubmit: schema },
    onSubmit: ({ value }) => {
      const { username, password } = schema.parse(value)
      return onSubmit(username, password)
    },
  })
  return (
    <form.AppForm>
      <form.FormElement className="flex flex-col gap-4">
        <form.AppField name="username">
          {(field) => <field.TextField label="Username" autoComplete="username" autoCapitalize="none" autoCorrect="off" spellCheck={false} autoFocus={!defaultUsername} />}
        </form.AppField>
        <form.AppField name="password">
          {(field) => <field.PasswordField label="Password" autoComplete="current-password" autoFocus={Boolean(defaultUsername)} />}
        </form.AppField>
        <form.SubmitButton className="mt-2 w-full">Sign in</form.SubmitButton>
      </form.FormElement>
    </form.AppForm>
  )
}
