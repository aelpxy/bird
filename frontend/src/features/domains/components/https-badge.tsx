import { Badge } from '@/components/ui/badge'
import { dateTime, timeAgo } from '@/lib/format'
import type { Https } from '../api'

// where a domain's certificate stands, with the reason on hover when it is failing
export function HttpsBadge({ https }: { https: Https }) {
  switch (https.state) {
    case 'issued':
      return (
        <span title={`Certificate valid until ${dateTime(https.expires_at)}; birdd renews it 30 days before`}>
          <Badge tone="ok" dot>
            HTTPS
          </Badge>
        </span>
      )
    case 'pending':
      return (
        <span title="birdd is asking Let's Encrypt for a certificate; this needs the domain's DNS to point at this server">
          <Badge tone="info" dot pulse>
            Issuing
          </Badge>
        </span>
      )
    case 'failing':
      return (
        <span title={`${https.error}\nRetrying ${timeAgo(https.retry_at)}`}>
          <Badge tone="bad" dot>
            Certificate failing
          </Badge>
        </span>
      )
    case 'off':
      return (
        <span title="birdd runs without --acme-email, so this domain is served over plain HTTP">
          <Badge>HTTP only</Badge>
        </span>
      )
    case 'local':
      return (
        <span title="*.localhost names never get certificates">
          <Badge>Local</Badge>
        </span>
      )
  }
}
