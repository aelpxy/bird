import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { serviceQuery } from '@/features/services/api'
import { ScalePanel } from '@/features/services/components/scale-panel'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/settings/scaling')({
  component: Page,
})

function Page() {
  const { scope, service: name } = useServiceScope()
  const { data: service } = useQuery(serviceQuery(scope, name))
  if (!service) return <Skeleton className="h-48 w-full" />
  return (
    <Section title="Scaling" description="How many machines run this service.">
      <ScalePanel scope={scope} service={service} />
    </Section>
  )
}
