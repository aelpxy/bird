import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Skeleton } from '@/components/ui/skeleton'
import { serviceQuery } from '@/features/services/api'
import { useServiceScope } from '@/features/services/use-service-scope'
import { TerminalPanel } from '@/features/terminal/components/terminal-panel'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/terminal')({
  component: TerminalTab,
})

function TerminalTab() {
  const { scope, service: name } = useServiceScope()
  const { data: service } = useQuery(serviceQuery(scope, name))
  if (!service) return <Skeleton className="h-48 w-full rounded-xl" />
  const machines =
    service.state === 'running'
      ? (service.deployment?.machines ?? []).filter((machine) => machine.state === 'running').map((machine) => machine.id)
      : []
  return <TerminalPanel scope={scope} service={name} machines={machines} />
}
