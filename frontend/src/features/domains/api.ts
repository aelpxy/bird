import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type DomainStatus = Schemas['DomainStatus']
export type Https = Schemas['Https']

const STILL_ISSUING_POLL_MS = 5000

export const domainsQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['domains', scope.project, scope.environment, service],
    queryFn: () => api<DomainStatus[]>(servicePath(scope, service, '/domains')),
    // a certificate usually arrives within a minute of DNS resolving, so keep looking until it does
    refetchInterval: (query) => (query.state.data?.some((domain) => domain.https.state === 'pending' || domain.https.state === 'failing') ? STILL_ISSUING_POLL_MS : false),
  })

export function addDomain(scope: Scope, service: string, hostname: string) {
  const body: Schemas['AddDomain'] = { hostname }
  return api<string[]>(servicePath(scope, service, '/domains'), { method: 'POST', body })
}

export function setRedirect(scope: Scope, service: string, hostname: string, target: string | null) {
  const body: Schemas['UpdateDomain'] = { redirect_to: target }
  return api<void>(servicePath(scope, service, `/domains/${encodeURIComponent(hostname)}`), { method: 'PUT', body })
}

export function removeDomain(scope: Scope, service: string, hostname: string) {
  return api<void>(servicePath(scope, service, `/domains/${encodeURIComponent(hostname)}`), {
    method: 'DELETE',
  })
}


// birdd only gets certificates for public names
export function domainUrl(hostname: string): string {
  return hostname.endsWith('.localhost') ? `http://${hostname}` : `https://${hostname}`
}
