import { createFileRoute } from '@tanstack/react-router'
import { Section } from '@/components/ui/section'
import { EnvironmentBackups } from '@/features/backups/components/environment-backups'

export const Route = createFileRoute('/_app/projects/$project/-/settings/backups')({ component: Backups })

function Backups() {
  const { project } = Route.useRouteContext()
  return (
    <div className="flex flex-col gap-10">
      {project.environments.map((environment) => (
        <Section
          key={environment}
          title={environment}
          description="Backups outlive their service, so a removed service's copies stay here until you delete them."
        >
          <EnvironmentBackups scope={{ project: project.name, environment }} />
        </Section>
      ))}
    </div>
  )
}
