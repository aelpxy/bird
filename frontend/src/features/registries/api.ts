import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'

export type Registry = Schemas['RegistrySummary']

export const registriesQuery = queryOptions({
  queryKey: ['registries'],
  queryFn: ({ signal }) => api<Registry[]>('/v1/registries', { signal }),
})

export function logInToRegistry(host: string, body: Schemas['RegistryLogin']) {
  return api<void>(`/v1/registries/${encodeURIComponent(host)}`, { method: 'PUT', body })
}

export function logOutOfRegistry(host: string) {
  return api<void>(`/v1/registries/${encodeURIComponent(host)}`, { method: 'DELETE' })
}
