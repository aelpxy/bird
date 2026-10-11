import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { BackLink } from '@/components/ui/back-link'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { DangerZone } from '@/components/ui/danger-zone'
import { PageHeader } from '@/components/ui/page-header'
import { Section } from '@/components/ui/section'
import { notifySuccess } from '@/lib/toast'
import { removeOrg } from '@/features/orgs/api'
import { MembersPanel } from '@/features/orgs/components/members-panel'
import { orgsQuery } from '@/features/projects/api'

export const Route = createFileRoute('/_app/orgs/$org')({
  head: ({ params }) => ({ meta: [{ title: `${params.org} · Orgs · bird` }] }),
  component: Org,
})

function Org() {
  const { org } = Route.useParams()
  const { session } = Route.useRouteContext()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  return (
    <div className="flex max-w-3xl flex-col gap-8">
      <div className="flex flex-col gap-2">
        <BackLink to="/orgs">Orgs</BackLink>
        <PageHeader eyebrow="Org" title={org} />
      </div>
      <Section title="Members" description="Owners and admins manage members; owners can delete the org.">
        <MembersPanel org={org} session={session} />
      </Section>
      <Section title="Danger zone">
        <DangerZone
          title="Delete this org"
          description={<>birdd refuses while the org still owns projects.</>}
          action={
          <ConfirmDialog
            trigger={
              <Button variant="danger" size="sm">
                <Trash2 aria-hidden className="size-4" />
                Delete org
              </Button>
            }
            title={`Delete ${org}?`}
            description="This cannot be undone."
            confirmLabel="Delete org"
            tone="danger"
            confirmText={org}
            onConfirm={async () => {
              await removeOrg(org)
              notifySuccess(`Deleted ${org}`)
              await queryClient.invalidateQueries({ queryKey: orgsQuery.queryKey })
              await navigate({ to: '/orgs' })
            }}
          />
          }
        />
      </Section>
    </div>
  )
}
