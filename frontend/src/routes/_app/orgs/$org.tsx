import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { FolderKanban, Trash2 } from 'lucide-react'
import { BackLink } from '@/components/ui/back-link'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { DangerZone } from '@/components/ui/danger-zone'
import { EmptyState } from '@/components/ui/empty-state'
import { ErrorState } from '@/components/ui/error-state'
import { PageHeader } from '@/components/ui/page-header'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { notifySuccess } from '@/lib/toast'
import { removeOrg } from '@/features/orgs/api'
import { MembersPanel } from '@/features/orgs/components/members-panel'
import { useOverview } from '@/features/overview/use-overview'
import { orgsQuery } from '@/features/projects/api'
import { NewProjectDialog } from '@/features/projects/components/new-project-dialog'
import { ProjectGrid } from '@/features/projects/components/projects-by-org'

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
      <Section title="Projects" description="Everything this org owns. Its members work in these." actions={<NewProjectDialog org={org} />}>
        <OrgProjects org={org} />
      </Section>
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

function OrgProjects({ org }: { org: string }) {
  const overview = useOverview()
  const projects = overview.projects.filter((project) => project.org === org)
  if (overview.loading && overview.projects.length === 0) return <Skeleton className="h-36 w-full" />
  if (overview.error && overview.projects.length === 0) return <ErrorState error={overview.error} />
  if (projects.length === 0) return <EmptyState icon={FolderKanban} title="No projects yet" description={`Projects created in ${org} show up here.`} />
  return <ProjectGrid projects={projects} services={overview.services} />
}
