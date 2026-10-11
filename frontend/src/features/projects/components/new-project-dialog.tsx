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

// with an org it creates the project there, otherwise it asks when there are several
type NewProjectDialogProps = { org?: string; open?: boolean; onOpenChange?: (open: boolean) => void }

export function NewProjectDialog({ org, open, onOpenChange }: NewProjectDialogProps) {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const { data: orgs = [] } = useQuery(orgsQuery)
  const dialog = useDialogForm({
    schema,
    // empty means the first org, which the select shows first
    defaultValues: { name: '', org: org ?? '' },
    open,
    onOpenChange,
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
        open === undefined ? (
          <Button>
            <Plus aria-hidden className="size-3.5" />
            New project
          </Button>
        ) : undefined
      }
      title="New project"
      description={`One product, like a shop: its app, database and workers are each added to it as a service. It starts with a production environment${org ? ` and belongs to ${org}.` : '.'}`}
      submitLabel="Create project"
    >
      <dialog.form.AppField name="name">
        {(field) => <field.TextField label="Project name" placeholder="shop" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
      </dialog.form.AppField>
      {!org && orgs.length > 1 && (
        <dialog.form.AppField name="org">
          {(field) => <field.SelectField label="Org" options={orgs.map((org) => ({ value: org.name, label: org.name }))} />}
        </dialog.form.AppField>
      )}
    </FormDialog>
  )
}
