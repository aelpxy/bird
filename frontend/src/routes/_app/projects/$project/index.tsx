import { createFileRoute, notFound, redirect } from '@tanstack/react-router'
import { Layers } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { EmptyState } from '@/components/ui/empty-state'
import { preferredEnvironment, projectsQuery } from '@/features/projects/api'
import { NewEnvironmentDialog } from '@/features/projects/components/new-environment-dialog'

// a project has no page of its own; it opens on its main environment
export const Route = createFileRoute('/_app/projects/$project/')({
  beforeLoad: async ({ context, params }) => {
    const projects = await context.queryClient.query(projectsQuery)
    const project = projects.find((candidate) => candidate.name === params.project)
    if (!project) throw notFound()
    const environment = preferredEnvironment(project)
    if (environment) {
      throw redirect({
        to: '/projects/$project/$environment',
        params: { project: project.name, environment },
        replace: true,
      })
    }
  },
  component: NoEnvironments,
})

function NoEnvironments() {
  const { project } = Route.useParams()
  const [creating, setCreating] = useState(false)
  return (
    <>
      <EmptyState
        icon={Layers}
        title={`${project} has no environments`}
        description="Services run in an environment, so create one first."
        action={<Button onClick={() => setCreating(true)}>New environment</Button>}
      />
      <NewEnvironmentDialog project={project} open={creating} onOpenChange={setCreating} />
    </>
  )
}
