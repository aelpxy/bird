import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { ErrorState } from '@/components/ui/error-state'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { proxyQuery } from '@/features/proxy/api'
import { MaintenancePanel } from '@/features/proxy/components/maintenance-panel'
import { ProxyForm } from '@/features/proxy/components/proxy-form'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/settings/proxy')({
  component: Page,
})

function Page() {
  const { scope, service } = useServiceScope()
  const settings = useQuery(proxyQuery(scope, service))
  if (settings.isPending) return <Skeleton className="h-96 w-full" />
  if (settings.isError) return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />
  return (
    <Section title="Proxy" description="How bird's proxy treats requests to this service. Changes apply to the next request, no redeploy needed.">
      <div className="flex flex-col gap-8">
        <MaintenancePanel scope={scope} service={service} settings={settings.data} />
        <ProxyForm key={JSON.stringify(settings.data)} scope={scope} service={service} settings={settings.data} />
      </div>
    </Section>
  )
}
