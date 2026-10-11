import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { FormDialog } from '@/components/ui/form-dialog'
import { notifySuccess } from '@/lib/toast'
import { NAME_HINT, name } from '@/lib/validation'
import { createEnvironment, projectsQuery } from '../api'

const schema = z.object({ name: name('Enter an environment name.') })

type NewEnvironmentDialogProps = {
  project: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function NewEnvironmentDialog({ project, open, onOpenChange }: NewEnvironmentDialogProps) {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const dialog = useDialogForm({
    schema,
    defaultValues: { name: '' },
    open,
    onOpenChange,
    onSubmit: async (values) => {
      await createEnvironment(project, { name: values.name })
      await queryClient.invalidateQueries({ queryKey: projectsQuery.queryKey })
      notifySuccess(`Created ${values.name}`, `A new environment in ${project}, with its own network.`)
      await navigate({ to: '/projects/$project/$environment', params: { project, environment: values.name } })
    },
  })

  return (
    <FormDialog
      dialog={dialog}
      title="New environment"
      description="Services in an environment share a private network that no other environment can reach."
      submitLabel="Create environment"
    >
      <dialog.form.AppField name="name">
        {(field) => <field.TextField label="Environment name" placeholder="staging" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </dialog.form.AppField>
    </FormDialog>
  )
}
