import { useQueryClient } from '@tanstack/react-query'
import { UserPlus } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import { NAME_HINT, name } from '@/lib/validation'
import { createUser, usersQuery } from '../api'

const schema = z.object({ name: name('Enter a username.'), role: z.enum(['member', 'admin']) })

const ROLES = [
  { value: 'member', label: 'Member — works in the orgs they belong to' },
  { value: 'admin', label: 'Admin — manages the whole server' },
] as const

export function NewUserDialog({ onCreated }: { onCreated: (name: string, token: string) => void }) {
  const queryClient = useQueryClient()
  const dialog = useDialogForm({
    schema,
    defaultValues: { name: '', role: 'member' as 'member' | 'admin' },
    onSubmit: async (values) => {
      const created = await createUser(values)
      onCreated(created.user.name, created.token.token)
      await queryClient.invalidateQueries({ queryKey: usersQuery.queryKey })
    },
  })
  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button>
          <UserPlus aria-hidden className="size-3.5" />
          New user
        </Button>
      }
      title="New user"
      description="They get an API token to start with; set a password if they should use this dashboard."
      submitLabel="Create user"
    >
      <dialog.form.AppField name="name">
        {(field) => <field.TextField label="Username" placeholder="ada" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </dialog.form.AppField>
      <dialog.form.AppField name="role">{(field) => <field.SelectField label="Role" options={[...ROLES]} />}</dialog.form.AppField>
    </FormDialog>
  )
}
