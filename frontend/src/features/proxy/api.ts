import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type ProxySettings = Schemas['ProxySettings']

export const proxyQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['proxy', scope.project, scope.environment, service],
    queryFn: ({ signal }) => api<ProxySettings>(servicePath(scope, service, '/proxy'), { signal }),
  })

export function saveProxy(scope: Scope, service: string, settings: ProxySettings) {
  return api<ProxySettings>(servicePath(scope, service, '/proxy'), { method: 'PUT', body: settings })
}
