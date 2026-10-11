import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type VariableValue = Schemas['VariableValue']

// names only; values are fetched one at a time, when someone asks to see them
export const variablesQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'variables'],
    queryFn: ({ signal }) => api<string[]>(servicePath(scope, service, '/variables'), { signal }),
  })

export function revealVariable(scope: Scope, service: string, key: string) {
  return api<VariableValue>(servicePath(scope, service, `/variables/${encodeURIComponent(key)}`))
}

export function updateVariables(scope: Scope, service: string, body: Schemas['UpdateVariables']) {
  return api<Schemas['VariablesResponse']>(servicePath(scope, service, '/variables'), {
    method: 'PATCH',
    body,
  })
}
