import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { serviceQuery } from '@/features/services/api'
import { VolumesPanel } from '@/features/services/components/volumes-panel'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/settings/volumes')({
  component: Page,
})

function Page() {
  const { scope, service: name } = useServiceScope()
  const { data: service } = useQuery(serviceQuery(scope, name))
  if (!service) return <Skeleton className="h-48 w-full" />
  return (
    <Section title="Volumes" description="Data kept across deploys and machine replacements, and copied by backups.">
      <VolumesPanel scope={scope} service={service} />
    </Section>
  )
}
