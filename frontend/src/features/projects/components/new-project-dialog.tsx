import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import { notifySuccess } from '@/lib/toast'
import { NAME_HINT, name } from '@/lib/validation'
import { createProject, orgsQuery, projectsQuery } from '../api'

const schema = z.object({ name: name('Enter a project name.'), org: z.string() })

export function NewProjectDialog() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const { data: orgs = [] } = useQuery(orgsQuery)
  const dialog = useDialogForm({
    schema,
    // empty means the first org, which the select shows first
    defaultValues: { name: '', org: '' },
    onSubmit: async (values) => {
      const project = await createProject({ name: values.name, org: values.org || orgs[0]?.name })
      await queryClient.invalidateQueries({ queryKey: projectsQuery.queryKey })
      notifySuccess(`Created ${project.name}`)
      await navigate({ to: '/projects/$project', params: { project: project.name } })
    },
  })

  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button>
          <Plus aria-hidden className="size-3.5" />
          New project
        </Button>
      }
      title="New project"
      description="A project groups services, with a production environment to start."
      submitLabel="Create project"
    >
      <dialog.form.AppField name="name">
        {(field) => <field.TextField label="Project name" placeholder="shop" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </dialog.form.AppField>
      {orgs.length > 1 && (
        <dialog.form.AppField name="org">
          {(field) => <field.SelectField label="Org" options={orgs.map((org) => ({ value: org.name, label: org.name }))} />}
        </dialog.form.AppField>
      )}
    </FormDialog>
  )
}
