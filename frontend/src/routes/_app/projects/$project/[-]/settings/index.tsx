import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Layers, Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { focusRing } from '@/components/ui/control-styles'
import { IconButton } from '@/components/ui/icon-button'
import { Section } from '@/components/ui/section'
import { notifySuccess } from '@/lib/toast'
import { projectsQuery, removeEnvironment } from '@/features/projects/api'
import { NewEnvironmentDialog } from '@/features/projects/components/new-environment-dialog'

export const Route = createFileRoute('/_app/projects/$project/-/settings/')({ component: Environments })

function Environments() {
  const { project: name } = Route.useParams()
  const queryClient = useQueryClient()
  const { data: projects = [] } = useQuery(projectsQuery)
  const project = projects.find((candidate) => candidate.name === name)
  const [creating, setCreating] = useState(false)
  const refresh = () => queryClient.invalidateQueries({ queryKey: projectsQuery.queryKey })

  return (
      <Section title="Environments" description="Each has its own private network. An environment can only be deleted once it has no services or backups. Leftover backups are under Backups.">
        <ul className="divide-y divide-line border border-line bg-surface">
          {(project?.environments ?? []).map((environment) => (
            <li key={environment} className="flex items-center justify-between gap-3 px-4 py-3">
              <Link
                to="/projects/$project/$environment"
                params={{ project: name, environment }}
                className={`flex items-center gap-2 font-mono text-ui text-fg hover:underline underline-offset-4 ${focusRing}`}
              >
                <Layers aria-hidden className="size-4 text-fg-faint" />
                {environment}
              </Link>
              <ConfirmDialog
                trigger={
                  <IconButton label={`Delete ${environment}`} tone="danger">
                    <Trash2 aria-hidden className="size-4" />
                  </IconButton>
                }
                title={`Delete ${environment}?`}
                description="Its network is removed. birdd refuses while it still has services or backups."
                confirmLabel="Delete environment"
                tone="danger"
                confirmText={environment}
                onConfirm={async () => {
                  await removeEnvironment(name, environment)
                  notifySuccess(`Deleted ${environment}`)
                  await refresh()
                }}
              />
            </li>
          ))}
        </ul>
        <div>
          <Button variant="secondary" size="sm" onClick={() => setCreating(true)}>
            <Plus aria-hidden className="size-4" />
            New environment
          </Button>
        </div>
        <NewEnvironmentDialog project={name} open={creating} onOpenChange={setCreating} />
      </Section>
  )
}
