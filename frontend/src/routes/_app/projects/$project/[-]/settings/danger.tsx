import { useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute, useNavigate } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { DangerZone } from '@/components/ui/danger-zone'
import { Section } from '@/components/ui/section'
import { notifySuccess } from '@/lib/toast'
import { projectsQuery, removeProject } from '@/features/projects/api'

export const Route = createFileRoute('/_app/projects/$project/-/settings/danger')({ component: Danger })

function Danger() {
  const { project: name } = Route.useParams()
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const refresh = () => queryClient.invalidateQueries({ queryKey: projectsQuery.queryKey })
  return (
      <Section title="Danger zone">
        <DangerZone
          title="Delete this project"
          description={
            <>
              birdd refuses while any environment still has services or backups. Clear leftover backups under{' '}
              <Link to="/projects/$project/-/settings/backups" params={{ project: name }} className="text-fg underline underline-offset-4">
                Backups
              </Link>
              .
            </>
          }
          action={
          <ConfirmDialog
            trigger={
              <Button variant="danger" size="sm">
                <Trash2 aria-hidden className="size-4" />
                Delete project
              </Button>
            }
            title={`Delete ${name}?`}
            description="Its environments and their networks are removed. This cannot be undone."
            confirmLabel="Delete project"
            tone="danger"
            confirmText={name}
            onConfirm={async () => {
              await removeProject(name)
              notifySuccess(`Deleted ${name}`)
              queryClient.removeQueries({ queryKey: ['services', name] })
              await refresh()
              await navigate({ to: '/' })
            }}
          />
          }
        />
      </Section>
  )
}
