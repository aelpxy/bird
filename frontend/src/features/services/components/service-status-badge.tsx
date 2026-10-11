import { Badge } from '@/components/ui/badge'
import type { Service } from '@/features/services/api'
import { serviceStatus } from '@/features/services/status'

export function ServiceStatusBadge({ service }: { service: Service }) {
  const status = serviceStatus(service)
  return (
    <Badge tone={status.tone} dot pulse={status.busy}>
      {status.label}
    </Badge>
  )
}
