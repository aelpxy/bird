import { createFileRoute } from '@tanstack/react-router'
import { LogViewer } from '@/features/logs/components/log-viewer'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/logs')({
  component: Logs,
})

function Logs() {
  const { scope, service } = useServiceScope()
  return <LogViewer scope={scope} service={service} />
}
