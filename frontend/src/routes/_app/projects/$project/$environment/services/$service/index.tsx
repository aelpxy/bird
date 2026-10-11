import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Alert } from '@/components/ui/alert'
import { Section } from '@/components/ui/section'
import { Skeleton } from '@/components/ui/skeleton'
import { dateTime } from '@/lib/format'
import { serviceQuery } from '@/features/services/api'
import { MachinesPanel } from '@/features/services/components/machines-panel'
import { ServiceConfig } from '@/features/services/components/service-config'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/')({
  component: Overview,
})

function Overview() {
  const { scope, service: name } = useServiceScope()
  // with stats, so this page alone pays for sampling the machines
  const service = useQuery({ ...serviceQuery(scope, name, true), placeholderData: (previous) => previous })
  const fallback = useQuery(serviceQuery(scope, name))
  const data = service.data ?? fallback.data
  if (!data) return <Skeleton className="h-48 w-full" />
  return (
    <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_22rem]">
      <Section title="Machines">
        {data.failed_deploy && (
          <Alert>
            The deploy on {dateTime(data.failed_deploy.created_at)} failed, so the previous deployment is still
            running. See Deployments and Logs for why.
          </Alert>
        )}
        <MachinesPanel service={data} />
      </Section>
      <Section title="Configuration">
        <ServiceConfig service={data} />
      </Section>
    </div>
  )
}
