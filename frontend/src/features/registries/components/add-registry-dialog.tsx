import { useQueryClient } from '@tanstack/react-query'
import { Plus } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import { notifySuccess } from '@/lib/toast'
import { registryHost, required } from '@/lib/validation'
import { logInToRegistry, registriesQuery } from '../api'

const schema = z.object({
  host: registryHost,
  username: required('Enter the username.'),
  password: required('Enter the password or token.'),
  insecure: z.boolean(),
})

export function AddRegistryDialog() {
  const queryClient = useQueryClient()
  const dialog = useDialogForm({
    schema,
    defaultValues: { host: '', username: '', password: '', insecure: false },
    onSubmit: async ({ host, ...login }) => {
      await logInToRegistry(host, login)
      notifySuccess(`Saved credentials for ${host}`)
      await queryClient.invalidateQueries({ queryKey: registriesQuery.queryKey })
    },
  })
  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button>
          <Plus aria-hidden className="size-3.5" />
          Add registry
        </Button>
      }
      title="Add a registry"
      description="birdd uses these credentials whenever it pulls an image from this host. Prefer a read-only token over a password."
      submitLabel="Save credentials"
    >
      <dialog.form.AppField name="host">
        {(field) => <field.TextField label="Host" placeholder="ghcr.io" mono autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </dialog.form.AppField>
      <dialog.form.AppField name="username">
        {(field) => <field.TextField label="Username" autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </dialog.form.AppField>
      <dialog.form.AppField name="password">{(field) => <field.PasswordField label="Password or token" autoComplete="new-password" />}</dialog.form.AppField>
      <dialog.form.AppField name="insecure">
        {(field) => <field.CheckboxField label="Skip TLS verification" description="Only for a registry on your own network with a self-signed certificate." />}
      </dialog.form.AppField>
    </FormDialog>
  )
}
