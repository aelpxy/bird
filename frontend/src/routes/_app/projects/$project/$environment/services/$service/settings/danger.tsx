import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { serviceQuery } from '@/features/services/api'
import { DeleteService } from '@/features/services/components/delete-service'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/settings/danger')({
  component: Page,
})

function Page() {
  const { scope, service: name } = useServiceScope()
  const { data: service } = useQuery(serviceQuery(scope, name))
  if (!service) return <Skeleton className="h-48 w-full" />
  return (
    <Section title="Danger zone" description="Things that cannot be undone.">
      <DeleteService scope={scope} service={service} />
    </Section>
  )
}
