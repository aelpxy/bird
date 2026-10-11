import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { serviceQuery } from '@/features/services/api'
import { ServiceConfigForm } from '@/features/services/components/service-config-form'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/settings/')({
  component: Page,
})

function Page() {
  const { scope, service: name } = useServiceScope()
  const { data: service } = useQuery(serviceQuery(scope, name))
  if (!service) return <Skeleton className="h-48 w-full" />
  return (
    <Section title="Deployment" description="Saving redeploys the service with these settings. If the new machines never get healthy, the old ones keep serving and nothing changes.">
      <ServiceConfigForm key={`${service.image}-${service.deployment?.id ?? ''}`} scope={scope} service={service} />
    </Section>
  )
}
