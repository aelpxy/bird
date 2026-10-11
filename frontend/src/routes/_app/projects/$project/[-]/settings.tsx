import { Link, Outlet, createFileRoute, notFound } from '@tanstack/react-router'
import { SettingsLayout, settingsLinkClass } from '@/components/layout/settings-layout'
import { BackLink } from '@/components/ui/back-link'
import { PageHeader } from '@/components/ui/page-header'
import { projectsQuery } from '@/features/projects/api'

const sections = [
  { to: '/projects/$project/-/settings', label: 'Environments', exact: true },
  { to: '/projects/$project/-/settings/backups', label: 'Backups' },
  { to: '/projects/$project/-/settings/danger', label: 'Danger zone' },
] as const

export const Route = createFileRoute('/_app/projects/$project/-/settings')({
  beforeLoad: async ({ context, params }) => {
    const projects = await context.queryClient.query(projectsQuery)
    const project = projects.find((candidate) => candidate.name === params.project)
    if (!project) throw notFound()
    return { project }
  },
  head: ({ params }) => ({ meta: [{ title: `Settings · ${params.project} · bird` }] }),
  component: ProjectSettings,
})

function ProjectSettings() {
  const { project } = Route.useRouteContext()
  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-3">
        <BackLink to="/projects/$project" params={{ project: project.name }}>
          {project.name}
        </BackLink>
        <PageHeader eyebrow={project.name} title="Project settings" description={`Owned by ${project.org}.`} />
      </div>
      <SettingsLayout
        label="Settings"
        nav={sections.map((section) => (
          <li key={section.to}>
            <Link to={section.to} params={{ project: project.name }} activeOptions={{ exact: 'exact' in section }} className={settingsLinkClass}>
              {section.label}
            </Link>
          </li>
        ))}
      >
        <Outlet />
      </SettingsLayout>
    </div>
  )
}
