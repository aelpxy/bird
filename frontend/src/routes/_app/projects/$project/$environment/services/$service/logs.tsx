import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { LogViewer } from '@/features/logs/components/log-viewer'
import { parseRange, type LogRange } from '@/features/logs/ranges'
import { useServiceScope } from '@/features/services/use-service-scope'

export const Route = createFileRoute('/_app/projects/$project/$environment/services/$service/logs')({
  // the query and range live in the address, so a view of the logs can be shared or reloaded
  validateSearch: (search: Record<string, unknown>): { q?: string; range?: LogRange } => ({
    q: typeof search.q === 'string' && search.q ? search.q : undefined,
    range: parseRange(search.range),
  }),
  component: Logs,
})

function Logs() {
  const { scope, service } = useServiceScope()
  const { q = '', range = 'live' } = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  return (
    <LogViewer
      key={range}
      scope={scope}
      service={service}
      query={q}
      range={range}
      onQuery={(next) => void navigate({ search: (current) => ({ ...current, q: next || undefined }), replace: true })}
      onRange={(next) => void navigate({ search: (current) => ({ ...current, range: next === 'live' ? undefined : next }), replace: true })}
    />
  )
}
