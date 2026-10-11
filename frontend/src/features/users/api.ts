import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'

export type User = Schemas['UserSummary']
export type Role = Schemas['UserRole']

export const usersQuery = queryOptions({
  queryKey: ['users'],
  queryFn: ({ signal }) => api<User[]>('/v1/users', { signal }),
})

export function createUser(body: Schemas['CreateUser']) {
  return api<Schemas['CreatedUser']>('/v1/users', { method: 'POST', body })
}

export function removeUser(name: string) {
  return api<void>(`/v1/users/${encodeURIComponent(name)}`, { method: 'DELETE' })
}

// admins set someone else's password without knowing the old one
export function resetPassword(name: string, password: string) {
  return api<void>(`/v1/users/${encodeURIComponent(name)}/password`, {
    method: 'PUT',
    body: { new: password },
  })
}
