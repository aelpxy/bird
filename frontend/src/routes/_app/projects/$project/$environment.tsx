import { Outlet, createFileRoute, notFound } from '@tanstack/react-router'
import { projectsQuery } from '@/features/projects/api'

// checks the project and environment exist before any page under them asks birdd about services
export const Route = createFileRoute('/_app/projects/$project/$environment')({
  beforeLoad: async ({ context, params }) => {
    const projects = await context.queryClient.query(projectsQuery)
    const project = projects.find((candidate) => candidate.name === params.project)
    if (!project?.environments.includes(params.environment)) throw notFound()
    return { scope: { project: params.project, environment: params.environment } }
  },
  component: EnvironmentLayout,
})

function EnvironmentLayout() {
  return <Outlet />
}
