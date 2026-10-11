import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export function addDomain(scope: Scope, service: string, hostname: string) {
  const body: Schemas['AddDomain'] = { hostname }
  return api<string[]>(servicePath(scope, service, '/domains'), { method: 'POST', body })
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
