import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Building2, ChevronRight, Plus } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ErrorState } from '@/components/ui/error-state'
import { FormDialog } from '@/components/ui/form-dialog'
import { PageHeader } from '@/components/ui/page-header'
import { Skeleton } from '@/components/ui/skeleton'
import { notifySuccess } from '@/lib/toast'
import { NAME_HINT, name } from '@/lib/validation'
import { createOrg } from '@/features/orgs/api'
import { orgsQuery } from '@/features/projects/api'

export const Route = createFileRoute('/_app/orgs/')({
  head: () => ({ meta: [{ title: 'Orgs · bird' }] }),
  component: Orgs,
})

function Orgs() {
  const queryClient = useQueryClient()
  const orgs = useQuery(orgsQuery)
  const newOrg = useDialogForm({
    schema: z.object({ name: name('Enter an org name.') }),
    defaultValues: { name: '' },
    onSubmit: async (values) => {
      await createOrg(values.name)
      notifySuccess(`Created ${values.name}`)
      await queryClient.invalidateQueries({ queryKey: orgsQuery.queryKey })
    },
  })
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        eyebrow="Ownership"
        title="Orgs"
        description="Orgs own projects; their members work in them."
        actions={
          <FormDialog
            dialog={newOrg}
            trigger={
              <Button>
                <Plus aria-hidden className="size-3.5" />
                New org
              </Button>
            }
            title="New org"
            description="You become its owner."
            submitLabel="Create org"
          >
            <newOrg.form.AppField name="name">
              {(field) => <field.TextField label="Org name" placeholder="acme" description={NAME_HINT} autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />}
            </newOrg.form.AppField>
          </FormDialog>
        }
      />
      {orgs.isPending ? (
        <Skeleton className="h-32 w-full" />
      ) : orgs.isError ? (
        <ErrorState error={orgs.error} onRetry={() => void orgs.refetch()} />
      ) : (
        <ul className="divide-y divide-line border border-line bg-surface">
          {orgs.data.map((org) => (
            <li key={org.name}>
              <Link
                to="/orgs/$org"
                params={{ org: org.name }}
                className="group flex items-center gap-3 px-4 py-3 transition-colors hover:bg-surface focus-visible:bg-surface focus-visible:outline-none"
              >
                <Building2 aria-hidden className="size-4 text-fg-faint" />
                <span className="flex-1 text-sm font-medium text-fg">{org.name}</span>
                {org.role && <Badge>{org.role}</Badge>}
                <ChevronRight aria-hidden className="size-4 text-fg-faint group-hover:text-fg-muted" />
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
